//! The surface: pan, zoom, selection and the mindmap keys over a [`Canvas`].
//!
//! Zoom scales the layout rather than transforming paint — gpui has no
//! transform for arbitrary elements — so content re-lays out at each zoom and
//! stays sharp.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use editor::{Chrome, Editor, EditorEvent};
use gpui::{
    AnyElement, App, Bounds, Context, ElementId, Entity, EventEmitter, FocusHandle, Focusable,
    Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder, PinchEvent, Pixels, Point,
    Render, Rgba, ScrollWheelEvent, Subscription, Window, canvas as painter, div, point,
    prelude::*, px,
};
use markdown::{Editing, Marks, Typography};
use theme::{TextStyle, Theme};

use crate::{
    mindmap::{self, Toward},
    model::{Canvas, Edge, End, FILE, GROUP, LINK, Node, Side, TEXT},
    node,
};

/// The key context the canvas binds in.
pub const CONTEXT: &str = "BezelCanvas";

const MIN_ZOOM: f32 = 0.25;
const MAX_ZOOM: f32 = 4.0;
/// One chord's zoom.
const ZOOM_STEP: f32 = 1.25;
/// Zoom per pixel of a modified wheel.
const WHEEL_ZOOM: f32 = 0.01;
/// Inside a node, in canvas units.
const PAD: f32 = 12.0;
const RADIUS: f32 = 8.0;
/// Arrowhead length, in canvas units.
const ARROW: f32 = 8.0;
/// How far the selection ring sits outside a node, in screen pixels.
const RING: f32 = 3.0;

pub mod keys {
    //! Every action the canvas answers to, and the chords bound to it.

    use gpui::{KeyBinding, actions};

    actions!(
        bezel_canvas,
        [
            AddChild,
            AddSibling,
            Remove,
            Edit,
            StopEditing,
            SelectParent,
            SelectChild,
            SelectPrev,
            SelectNext,
            ZoomIn,
            ZoomOut,
            ResetZoom,
        ]
    );

    /// The default keymap, as data. Escape is bound over the editor's own so
    /// it leaves a node, which is why [`super::init`] runs after `editor::init`.
    pub fn bindings() -> Vec<KeyBinding> {
        let ctx = Some(super::CONTEXT);
        let editing = format!("{} > {}", super::CONTEXT, editor::CONTEXT);
        let mut bindings = vec![
            KeyBinding::new("tab", AddChild, ctx),
            KeyBinding::new("enter", AddSibling, ctx),
            KeyBinding::new("backspace", Remove, ctx),
            KeyBinding::new("delete", Remove, ctx),
            KeyBinding::new("f2", Edit, ctx),
            KeyBinding::new("left", SelectParent, ctx),
            KeyBinding::new("right", SelectChild, ctx),
            KeyBinding::new("up", SelectPrev, ctx),
            KeyBinding::new("down", SelectNext, ctx),
            KeyBinding::new("escape", StopEditing, Some(&editing)),
        ];
        #[cfg(target_os = "macos")]
        bindings.extend([
            KeyBinding::new("cmd-=", ZoomIn, ctx),
            KeyBinding::new("cmd--", ZoomOut, ctx),
            KeyBinding::new("cmd-0", ResetZoom, ctx),
        ]);
        #[cfg(not(target_os = "macos"))]
        bindings.extend([
            KeyBinding::new("ctrl-=", ZoomIn, ctx),
            KeyBinding::new("ctrl--", ZoomOut, ctx),
            KeyBinding::new("ctrl-0", ResetZoom, ctx),
        ]);
        bindings
    }
}

/// Install the canvas key bindings. Call after `editor::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys(keys::bindings());
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasEvent {
    /// The document is different: a node added, removed or retyped.
    Changed,
    Selected(Option<String>),
}

/// Who decides where nodes sit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrange {
    /// [`mindmap::layout`] after every change.
    #[default]
    Mindmap,
    /// Where the document says.
    Free,
}

struct Session {
    id: String,
    editor: Entity<Editor>,
    _changes: Subscription,
}

pub struct CanvasView {
    canvas: Canvas,
    arrange: Arrange,
    focus: FocusHandle,
    selected: Option<String>,
    editing: Option<Session>,
    /// Screen position of the canvas origin, from the view's top left.
    pan: Point<f32>,
    zoom: f32,
    grab: Option<Point<Pixels>>,
    stale: bool,
    framed: bool,
    viewport: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Text node heights read at prepaint, in canvas units.
    measured: Rc<RefCell<HashMap<String, i64>>>,
}

impl EventEmitter<CanvasEvent> for CanvasView {}

impl Focusable for CanvasView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl CanvasView {
    pub fn new(canvas: Canvas, cx: &mut Context<Self>) -> Self {
        Self {
            canvas,
            arrange: Arrange::default(),
            focus: cx.focus_handle(),
            selected: None,
            editing: None,
            pan: point(0.0, 0.0),
            zoom: 1.0,
            grab: None,
            stale: true,
            framed: false,
            viewport: Rc::default(),
            measured: Rc::default(),
        }
    }

    pub fn with_arrange(mut self, arrange: Arrange) -> Self {
        self.arrange = arrange;
        self
    }

    /// The document as a save would write it.
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    pub fn set_canvas(&mut self, canvas: Canvas, cx: &mut Context<Self>) {
        self.canvas = canvas;
        self.editing = None;
        if let Some(id) = &self.selected
            && self.canvas.node(id).is_none()
        {
            self.select(None, cx);
        }
        self.stale = true;
        cx.notify();
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn select(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if self.selected != id {
            self.selected = id.clone();
            cx.emit(CanvasEvent::Selected(id));
            cx.notify();
        }
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Zoom about the middle of the view.
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let middle = self.middle();
        self.zoom_about(zoom, middle, cx);
    }

    fn middle(&self) -> Point<f32> {
        self.viewport.get().map_or(point(0.0, 0.0), |b| {
            point(b.size.width.as_f32() / 2.0, b.size.height.as_f32() / 2.0)
        })
    }

    fn local(&self, position: Point<Pixels>) -> Point<f32> {
        let origin = self.viewport.get().map_or(Point::default(), |b| b.origin);
        point(
            (position.x - origin.x).as_f32(),
            (position.y - origin.y).as_f32(),
        )
    }

    fn zoom_about(&mut self, zoom: f32, anchor: Point<f32>, cx: &mut Context<Self>) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if zoom == self.zoom {
            return;
        }
        let world = point(
            (anchor.x - self.pan.x) / self.zoom,
            (anchor.y - self.pan.y) / self.zoom,
        );
        self.pan = point(anchor.x - world.x * zoom, anchor.y - world.y * zoom);
        self.zoom = zoom;
        if let Some(session) = &self.editing {
            let size = TextStyle::Body.painted() * zoom;
            session
                .editor
                .update(cx, |editor, cx| editor.set_text_size(size, cx));
        }
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.stale = true;
        cx.emit(CanvasEvent::Changed);
        cx.notify();
    }

    fn added(&mut self, id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = id else { return };
        self.changed(cx);
        self.select(Some(id.clone()), cx);
        self.edit(id, window, cx);
    }

    fn add_child(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = match &self.selected {
            Some(parent) => mindmap::add_child(&mut self.canvas, parent),
            None if self.canvas.nodes.is_empty() => Some(mindmap::add_root(&mut self.canvas, 0, 0)),
            None => None,
        };
        self.added(id, window, cx);
    }

    /// A root has no siblings, so Enter on one adds a child.
    fn add_sibling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(of) = self.selected.clone() else {
            return;
        };
        let id = mindmap::add_sibling(&mut self.canvas, &of)
            .or_else(|| mindmap::add_child(&mut self.canvas, &of));
        self.added(id, window, cx);
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let next = mindmap::remove(&mut self.canvas, &id);
        self.changed(cx);
        self.select(next, cx);
    }

    fn step(&mut self, toward: Toward, cx: &mut Context<Self>) {
        let next = match &self.selected {
            Some(id) => mindmap::step(&self.canvas, id, toward),
            None => mindmap::roots(&self.canvas).next().map(|n| n.id.clone()),
        };
        if next.is_some() {
            self.select(next, cx);
        }
    }

    fn edit(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_editing(window, cx);
        let Some(node) = self.canvas.node(&id).filter(|n| n.kind == TEXT) else {
            return;
        };
        let text = node.text.clone().unwrap_or_default();
        let size = TextStyle::Body.painted() * self.zoom;
        let editor = cx.new(|cx| {
            Editor::new(&text, cx)
                .with_chrome(Chrome {
                    handle: false,
                    slash: false,
                    language: false,
                    paste: false,
                })
                .with_text_size(size)
        });
        let changes = cx.subscribe(&editor, |this, editor, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let source = editor.read(cx).source();
            if let Some(session) = &this.editing
                && let Some(node) = this.canvas.node_mut(&session.id)
            {
                node.text = Some(source);
                cx.emit(CanvasEvent::Changed);
                cx.notify();
            }
        });
        window.focus(&editor.focus_handle(cx), cx);
        self.editing = Some(Session {
            id,
            editor,
            _changes: changes,
        });
        cx.notify();
    }

    fn stop_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn press_background(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        self.select(None, cx);
        self.grab = Some(event.position);
    }

    fn press_node(
        &mut self,
        id: String,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.as_ref().is_some_and(|s| s.id == id) {
            return;
        }
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        self.select(Some(id.clone()), cx);
        if event.click_count >= 2 {
            self.edit(id, window, cx);
        }
    }

    fn drag(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(last) = self.grab else { return };
        if !event.dragging() {
            self.grab = None;
            return;
        }
        self.pan.x += (event.position.x - last.x).as_f32();
        self.pan.y += (event.position.y - last.y).as_f32();
        self.grab = Some(event.position);
        cx.notify();
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        if event.modifiers.platform || event.modifiers.control {
            let zoom = self.zoom * (delta.y.as_f32() * WHEEL_ZOOM).exp();
            self.zoom_about(zoom, self.local(event.position), cx);
        } else {
            self.pan.x += delta.x.as_f32();
            self.pan.y += delta.y.as_f32();
            cx.notify();
        }
        cx.stop_propagation();
    }

    fn pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let zoom = self.zoom * (1.0 + event.delta);
        self.zoom_about(zoom, self.local(event.position), cx);
    }

    /// Take last frame's measurements, and lay out if anything moved.
    fn reflow(&mut self) {
        let measured = std::mem::take(&mut *self.measured.borrow_mut());
        for (id, height) in measured {
            if let Some(node) = self.canvas.node_mut(&id)
                && node.height != height
            {
                node.height = height;
                self.stale = true;
            }
        }
        if std::mem::take(&mut self.stale) && self.arrange == Arrange::Mindmap {
            mindmap::layout(&mut self.canvas);
        }
    }

    /// Centre the document in the view, once, when the view has a size.
    fn frame(&mut self) {
        let Some(viewport) = self.viewport.get().filter(|_| !self.framed) else {
            return;
        };
        self.framed = true;
        let size = point(viewport.size.width.as_f32(), viewport.size.height.as_f32());
        let nodes = &self.canvas.nodes;
        let (x0, y0, x1, y1) = nodes.iter().fold(
            (i64::MAX, i64::MAX, i64::MIN, i64::MIN),
            |(x0, y0, x1, y1), n| {
                (
                    x0.min(n.x),
                    y0.min(n.y),
                    x1.max(n.x + n.width),
                    y1.max(n.y + n.height),
                )
            },
        );
        self.pan = if nodes.is_empty() {
            point(size.x / 2.0, size.y / 2.0)
        } else {
            let z = self.zoom;
            point(
                (size.x - (x1 - x0) as f32 * z) / 2.0 - x0 as f32 * z,
                (size.y - (y1 - y0) as f32 * z) / 2.0 - y0 as f32 * z,
            )
        };
    }

    fn paint_node(
        &self,
        ix: usize,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let node = &self.canvas.nodes[ix];
        let z = self.zoom;
        let (x, y) = (
            self.pan.x + node.x as f32 * z,
            self.pan.y + node.y as f32 * z,
        );
        let (w, h) = (node.width as f32 * z, node.height as f32 * z);
        if let Some(viewport) = self.viewport.get() {
            let (vw, vh) = (viewport.size.width.as_f32(), viewport.size.height.as_f32());
            if x > vw || y > vh || x + w < 0.0 || y + h < 0.0 {
                return None;
            }
        }
        let content = match self.editing.as_ref().filter(|s| s.id == node.id) {
            Some(session) => session.editor.clone().into_any_element(),
            None => node::render(node, z, window, cx)
                .unwrap_or_else(|| builtin(node, z, theme, window, cx)),
        };
        let grows = node.kind == TEXT;
        let selected = self.selected.as_deref() == Some(node.id.as_str());
        let border = node
            .color
            .as_deref()
            .and_then(|c| color(theme, c))
            .unwrap_or(theme.border);
        let (id, measured_id) = (node.id.clone(), node.id.clone());
        let (measured, height) = (self.measured.clone(), node.height);

        let element = div()
            .id(ElementId::Name(node.id.clone().into()))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .map(|d| {
                if grows {
                    d.min_h(px(mindmap::NODE_HEIGHT as f32 * z))
                } else {
                    d.h(px(h))
                }
            })
            .p(px(PAD * z))
            .rounded(px(RADIUS * z))
            .border_1()
            .border_color(border)
            .when(node.kind != GROUP, |d| d.bg(theme.surface_card))
            .child(content)
            .when(grows, |d| {
                d.child(
                    painter(
                        move |bounds, window, _| {
                            let now = (bounds.size.height.as_f32() / z).round() as i64;
                            if now != height {
                                measured.borrow_mut().insert(measured_id, now);
                                window.refresh();
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                )
            })
            .when(selected, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(-RING))
                        .left(px(-RING))
                        .right(px(-RING))
                        .bottom(px(-RING))
                        .rounded(px(RADIUS * z + RING))
                        .border_2()
                        .border_color(theme.accent),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.press_node(id.clone(), event, window, cx);
                }),
            );
        Some(element.into_any_element())
    }

    fn edge_layer(&self, theme: &Theme) -> impl IntoElement + use<> {
        let (z, pan) = (self.zoom, self.pan);
        let curves: Vec<Curve> = self
            .canvas
            .edges
            .iter()
            .filter_map(|edge| curve(&self.canvas, edge, theme))
            .collect();
        let viewport = self.viewport.clone();
        painter(
            move |bounds, window, _| {
                if viewport.replace(Some(bounds)).is_none() {
                    window.refresh();
                }
            },
            move |bounds, _, window, _| {
                let origin = point(bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                let screen =
                    |p: Point<f32>| point(origin.x + pan.x + p.x * z, origin.y + pan.y + p.y * z);
                for curve in curves {
                    let (p0, p1) = (screen(curve.from), screen(curve.to));
                    let reach = (p1.x - p0.x).abs().max((p1.y - p0.y).abs()) / 2.0;
                    let c0 = point(
                        p0.x + curve.from_out.x * reach,
                        p0.y + curve.from_out.y * reach,
                    );
                    let c1 = point(p1.x + curve.to_out.x * reach, p1.y + curve.to_out.y * reach);
                    let mid = point((c0.x + c1.x) / 2.0, (c0.y + c1.y) / 2.0);
                    let mut path = PathBuilder::stroke(px(z.max(1.0)));
                    path.move_to(pt(p0));
                    path.curve_to(pt(mid), pt(c0));
                    path.curve_to(pt(p1), pt(c1));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, curve.color);
                    }
                    if curve.to_arrow {
                        arrow(window, p1, curve.to_out, ARROW * z, curve.color);
                    }
                    if curve.from_arrow {
                        arrow(window, p0, curve.from_out, ARROW * z, curve.color);
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.reflow();
        self.frame();
        let theme = Theme::of(cx).clone();
        let edges = self.edge_layer(&theme);
        let nodes: Vec<AnyElement> = (0..self.canvas.nodes.len())
            .filter_map(|ix| self.paint_node(ix, &theme, window, cx))
            .collect();

        div()
            .id("bezel-canvas")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .on_action(
                cx.listener(|this, _: &keys::AddChild, window, cx| this.add_child(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::AddSibling, window, cx| this.add_sibling(window, cx)),
            )
            .on_action(cx.listener(|this, _: &keys::Remove, _, cx| this.remove(cx)))
            .on_action(cx.listener(|this, _: &keys::Edit, window, cx| {
                if let Some(id) = this.selected.clone() {
                    this.edit(id, window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &keys::StopEditing, window, cx| {
                    this.stop_editing(window, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &keys::SelectParent, _, cx| this.step(Toward::Parent, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::SelectChild, _, cx| this.step(Toward::FirstChild, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::SelectPrev, _, cx| this.step(Toward::PrevSibling, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::SelectNext, _, cx| this.step(Toward::NextSibling, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::ZoomIn, _, cx| {
                    this.set_zoom(this.zoom * ZOOM_STEP, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &keys::ZoomOut, _, cx| {
                this.set_zoom(this.zoom / ZOOM_STEP, cx)
            }))
            .on_action(cx.listener(|this, _: &keys::ResetZoom, _, cx| this.set_zoom(1.0, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::press_background))
            .on_mouse_move(cx.listener(Self::drag))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.grab = None),
            )
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_pinch(cx.listener(Self::pinch))
            .child(edges)
            .children(nodes)
    }
}

/// What a node paints when no renderer claims it.
fn builtin(node: &Node, zoom: f32, theme: &Theme, window: &mut Window, cx: &mut App) -> AnyElement {
    let label = |text: String, color: Hsla| {
        div()
            .text_size(px(TextStyle::Callout.painted() * zoom))
            .line_height(px(TextStyle::Callout.painted_line_height() * zoom))
            .text_color(color)
            .child(text)
            .into_any_element()
    };
    match node.kind.as_str() {
        TEXT => {
            let doc =
                markdown::parse_with(node.text.as_deref().unwrap_or_default(), &Marks::of(cx));
            let editing = Editing {
                typography: Some(Typography::of(cx).scaled(zoom)),
                ..Editing::default()
            };
            markdown::render_with(&doc, editing, window, cx)
        }
        LINK => label(node.url.clone().unwrap_or_default(), theme.accent),
        FILE => label(
            format!(
                "{}{}",
                node.file.as_deref().unwrap_or_default(),
                node.subpath.as_deref().unwrap_or_default()
            ),
            theme.text,
        ),
        GROUP => label(node.label.clone().unwrap_or_default(), theme.text_muted),
        other => label(other.to_owned(), theme.text_faint),
    }
}

/// A JSON Canvas color: a preset, or hex.
fn color(theme: &Theme, color: &str) -> Option<Hsla> {
    match color {
        "1" => Some(theme.danger),
        "2" => Some(theme.warning),
        "4" => Some(theme.success),
        "6" => Some(theme.accent),
        // Yellow and cyan have no token to stand for them.
        "3" | "5" => None,
        hex => Rgba::try_from(hex).ok().map(Into::into),
    }
}

struct Curve {
    from: Point<f32>,
    /// Out of the node, at the anchor.
    from_out: Point<f32>,
    to: Point<f32>,
    to_out: Point<f32>,
    from_arrow: bool,
    to_arrow: bool,
    color: Hsla,
}

fn curve(canvas: &Canvas, edge: &Edge, theme: &Theme) -> Option<Curve> {
    let (a, b) = (canvas.node(&edge.from_node)?, canvas.node(&edge.to_node)?);
    let (from_side, to_side) = facing(a, b);
    let (from, from_out) = anchor(a, edge.from_side.unwrap_or(from_side));
    let (to, to_out) = anchor(b, edge.to_side.unwrap_or(to_side));
    Some(Curve {
        from,
        from_out,
        to,
        to_out,
        from_arrow: edge.from_end == Some(End::Arrow),
        to_arrow: edge.to_end.unwrap_or(End::Arrow) == End::Arrow,
        color: edge
            .color
            .as_deref()
            .and_then(|c| color(theme, c))
            .unwrap_or(theme.border_strong),
    })
}

/// The sides two nodes face each other on, for an edge that names none.
fn facing(a: &Node, b: &Node) -> (Side, Side) {
    if b.x >= a.x + a.width {
        (Side::Right, Side::Left)
    } else if b.x + b.width <= a.x {
        (Side::Left, Side::Right)
    } else if b.y >= a.y + a.height {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Top, Side::Bottom)
    }
}

fn anchor(node: &Node, side: Side) -> (Point<f32>, Point<f32>) {
    let (x, y) = (node.x as f32, node.y as f32);
    let (w, h) = (node.width as f32, node.height as f32);
    match side {
        Side::Top => (point(x + w / 2.0, y), point(0.0, -1.0)),
        Side::Right => (point(x + w, y + h / 2.0), point(1.0, 0.0)),
        Side::Bottom => (point(x + w / 2.0, y + h), point(0.0, 1.0)),
        Side::Left => (point(x, y + h / 2.0), point(-1.0, 0.0)),
    }
}

fn arrow(window: &mut Window, tip: Point<f32>, out: Point<f32>, size: f32, color: Hsla) {
    let base = point(tip.x + out.x * size, tip.y + out.y * size);
    let half = point(-out.y * size / 2.0, out.x * size / 2.0);
    let mut path = PathBuilder::fill();
    path.move_to(pt(tip));
    path.line_to(pt(point(base.x + half.x, base.y + half.y)));
    path.line_to(pt(point(base.x - half.x, base.y - half.y)));
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

fn pt(p: Point<f32>) -> Point<Pixels> {
    point(px(p.x), px(p.y))
}
