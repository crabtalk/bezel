//! The surface: pan, zoom, selection and the mindmap keys over a [`Canvas`].
//!
//! Zoom scales the layout rather than transforming paint — gpui has no
//! transform for arbitrary elements — so content re-lays out at each zoom and
//! stays sharp.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
};

use editor::{Chrome as EditorChrome, Editor, EditorEvent};
use gpui::{
    AnyElement, App, Bounds, Context, CursorStyle, DispatchPhase, ElementId, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, KeyContext, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point, Render, Rgba, ScrollWheelEvent, Size,
    Subscription, WeakEntity, Window, canvas as painter, div, point, prelude::*, px,
};
use motion::{AppExt as _, LAYOUT};
use theme::{TextStyle, Theme};
use web_time::Instant;

use crate::{
    change::{self, Change},
    drag::{self, Drag, DragHandler, Phase},
    kind::{self, Chrome, Sizing},
    mindmap::{self, Toward},
    model::{self, Canvas, Edge, End, Node, Side},
};

/// The key context the canvas binds in.
pub const CONTEXT: &str = "BezelCanvas";

/// Claims `tab`, which adds a child here, from `ui::focus` traversal.
fn key_context() -> KeyContext {
    let mut context = KeyContext::default();
    context.add(CONTEXT);
    context.add(ui::focus::CLAIMS_TAB);
    context
}

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
/// How far a press on a node travels before it is a drag, in screen pixels.
const DRAG_SLOP: f32 = 3.0;
/// How much of a connector a drop would cut still shows.
const CUT: f32 = 0.25;
/// The accent wash inside a node a drop would land on.
const TARGET_WASH: f32 = 0.12;

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

// A change is handed on and dropped, never kept in bulk; boxing its node would
// only put a `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    /// A change landed in the document.
    Changed(Change),
    Selected(Option<String>),
}

/// Who decides where nodes sit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrange {
    /// [`mindmap::layout`] after every change.
    #[default]
    Mindmap,
    /// Where the document, and the drag handler, put them.
    Free,
}

/// Decides each change before it lands: it, another, or `None` to refuse.
type Filter = Rc<dyn Fn(&Canvas, Change, &mut App) -> Option<Change>>;

/// Canvas positions, by node id.
type Positions = HashMap<String, (f32, f32)>;

/// What a held button is moving.
enum Grab {
    Pan(Point<Pixels>),
    Node {
        id: String,
        from: Point<Pixels>,
        origin: (i64, i64),
        /// Whether it was pinned before the press, to put back on a drop.
        pinned: bool,
        moved: bool,
    },
}

/// Nodes easing from where they were painted toward where the document puts
/// them.
struct Glide {
    from: Positions,
    to: Positions,
    since: Instant,
}

struct Session {
    id: String,
    editor: Entity<Editor>,
    _changes: Subscription,
}

pub struct CanvasView {
    canvas: Canvas,
    arrange: Arrange,
    drag: DragHandler,
    filter: Option<Filter>,
    focus: FocusHandle,
    selected: Option<String>,
    editing: Option<Session>,
    /// Screen position of the canvas origin, from the view's top left.
    pan: Point<f32>,
    zoom: f32,
    grab: Option<Grab>,
    /// What the drop would do, drawn while a node is held: the drag handler's
    /// answer to the last move, less its `Move`s.
    pending: Vec<Change>,
    /// Where each node was painted last frame.
    shown: Positions,
    glide: Option<Glide>,
    stale: bool,
    /// The view size the document was last centred in.
    framed: Option<Size<Pixels>>,
    /// The reader has panned or zoomed, so a resize no longer re-centres.
    touched: bool,
    viewport: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Growing node heights read at prepaint, in canvas units.
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
            drag: drag::pin,
            filter: None,
            focus: cx.focus_handle(),
            selected: None,
            editing: None,
            pan: point(0.0, 0.0),
            zoom: 1.0,
            grab: None,
            pending: Vec::new(),
            shown: HashMap::new(),
            glide: None,
            stale: true,
            framed: None,
            touched: false,
            viewport: Rc::default(),
            measured: Rc::default(),
        }
    }

    pub fn with_arrange(mut self, arrange: Arrange) -> Self {
        self.arrange = arrange;
        self
    }

    /// What dragging a node does. [`drag::pin`] unless an app says otherwise.
    pub fn with_drag(mut self, handler: DragHandler) -> Self {
        self.drag = handler;
        self
    }

    /// Sees every change the canvas is about to make, and answers what lands:
    /// the change, another, or `None`. Called inside this view's update, so
    /// reach the view itself through `cx.defer`.
    pub fn with_changes(
        mut self,
        filter: impl Fn(&Canvas, Change, &mut App) -> Option<Change> + 'static,
    ) -> Self {
        self.filter = Some(Rc::new(filter));
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

    /// Land a change as if the reader made it: through the filter first.
    /// `false` when it was refused.
    pub fn submit(&mut self, change: Change, cx: &mut Context<Self>) -> bool {
        let change = match self.filter.clone() {
            Some(filter) => match filter(&self.canvas, change, cx) {
                Some(change) => change,
                None => return false,
            },
            None => change,
        };
        self.apply(change, cx);
        true
    }

    /// Land a change of the app's own, past the filter.
    pub fn apply(&mut self, change: Change, cx: &mut Context<Self>) {
        change::apply(&mut self.canvas, &change);
        if self
            .editing
            .as_ref()
            .is_some_and(|s| self.canvas.node(&s.id).is_none())
        {
            self.editing = None;
        }
        if let Some(id) = &self.selected
            && self.canvas.node(id).is_none()
        {
            self.select(None, cx);
        }
        self.stale = true;
        cx.emit(CanvasEvent::Changed(change));
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

    /// Where the canvas origin sits, in pixels from the view's top left.
    pub fn pan(&self) -> Point<f32> {
        self.pan
    }

    /// Where the view painted last frame, in window coordinates.
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.viewport.get()
    }

    /// Zoom about the middle of the view.
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let middle = self.middle();
        self.zoom_about(zoom, middle, cx);
    }

    /// What `cmd-=` does.
    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom * ZOOM_STEP, cx);
    }

    /// What `cmd--` does.
    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.set_zoom(self.zoom / ZOOM_STEP, cx);
    }

    /// Centre the document again, as it opened.
    pub fn fit(&mut self, cx: &mut Context<Self>) {
        self.touched = false;
        self.framed = None;
        cx.notify();
    }

    /// The canvas point under the middle of the view — where something added
    /// "here" lands.
    pub fn center(&self) -> (i64, i64) {
        let middle = self.middle();
        (
            ((middle.x - self.pan.x) / self.zoom).round() as i64,
            ((middle.y - self.pan.y) / self.zoom).round() as i64,
        )
    }

    pub fn arrange(&self) -> Arrange {
        self.arrange
    }

    /// Turning auto layout on tidies the document: every pin is dropped and
    /// the trees are laid out again.
    pub fn set_arrange(&mut self, arrange: Arrange, cx: &mut Context<Self>) {
        let tidy = arrange == Arrange::Mindmap && self.arrange != Arrange::Mindmap;
        self.arrange = arrange;
        if tidy {
            let pinned: Vec<String> = self
                .canvas
                .nodes
                .iter()
                .filter(|node| mindmap::is_pinned(node))
                .map(|node| node.id.clone())
                .collect();
            for id in pinned {
                self.apply(Change::Unpin { id }, cx);
            }
        }
        self.stale = true;
        cx.notify();
    }

    /// Swap what dragging a node does, from the next press.
    pub fn set_drag(&mut self, handler: DragHandler) {
        self.drag = handler;
    }

    /// What `backspace` does: the selection and its branch, through the filter.
    pub fn remove_selected(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let next = mindmap::after_removal(&self.canvas, &id);
        if self.submit(Change::Remove { id }, cx) {
            self.select(next.filter(|next| self.canvas.node(next).is_some()), cx);
        }
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
        self.touched = true;
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

    /// The node a drag has in hand, once it has moved.
    fn held(&self) -> Option<&str> {
        match &self.grab {
            Some(Grab::Node {
                id, moved: true, ..
            }) => Some(id),
            _ => None,
        }
    }

    /// What `tab` makes under `parent`, from its kind.
    fn template(&self, parent: &str, cx: &App) -> Option<Node> {
        let parent = self.canvas.node(parent)?;
        Some((kind::kind(cx, &parent.kind).child)(parent))
    }

    fn added(&mut self, change: Option<Change>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(change) = change else { return };
        let id = change.id().to_owned();
        if self.submit(change, cx) && self.canvas.node(&id).is_some() {
            self.select(Some(id.clone()), cx);
            self.edit(id, window, cx);
        }
    }

    fn add_child(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Nothing selected: under the first root, or a root of its own.
        let parent = self.selected.clone().or_else(|| {
            mindmap::roots(&self.canvas)
                .next()
                .map(|root| root.id.clone())
        });
        let change = match parent {
            Some(parent) => self
                .template(&parent, cx)
                .and_then(|node| mindmap::child(&self.canvas, &parent, node)),
            None => {
                let node = (kind::kind(cx, model::TEXT).child)(&Node::default());
                Some(mindmap::root(&self.canvas, node, (0, 0)))
            }
        };
        self.added(change, window, cx);
    }

    /// A root has no siblings, so Enter on one adds a child.
    fn add_sibling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(of) = self.selected.clone() else {
            return;
        };
        let change = match mindmap::parent(&self.canvas, &of).map(str::to_owned) {
            Some(parent) => self
                .template(&parent, cx)
                .and_then(|node| mindmap::sibling(&self.canvas, &of, node)),
            None => self
                .template(&of, cx)
                .and_then(|node| mindmap::child(&self.canvas, &of, node)),
        };
        self.added(change, window, cx);
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
        let Some(node) = self.canvas.node(&id) else {
            return;
        };
        let Some(field) = kind::kind(cx, &node.kind).edit else {
            return;
        };
        let value = (field.read)(node);
        let size = TextStyle::Body.painted() * self.zoom;
        let editor = cx.new(|cx| {
            Editor::new(&value, cx)
                .with_chrome(EditorChrome {
                    handle: false,
                    slash: false,
                    language: false,
                    paste: false,
                })
                .with_text_size(size)
        });
        let changes = cx.subscribe(&editor, move |this, editor, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let Some(mut node) = this
                .editing
                .as_ref()
                .and_then(|s| this.canvas.node(&s.id))
                .cloned()
            else {
                return;
            };
            (field.write)(&mut node, editor.read(cx).source());
            this.submit(Change::Update { node }, cx);
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
        self.grab = Some(Grab::Pan(event.position));
        // The listeners that follow the grab are painted next frame.
        cx.notify();
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
        } else if let Some(node) = self.canvas.node(&id) {
            self.grab = Some(Grab::Node {
                origin: (node.x, node.y),
                pinned: mindmap::is_pinned(node),
                id,
                from: event.position,
                moved: false,
            });
            cx.notify();
        }
    }

    fn drag(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.release(event.position, cx);
            return;
        }
        match &mut self.grab {
            None => {}
            Some(Grab::Pan(last)) => {
                self.pan.x += (event.position.x - last.x).as_f32();
                self.pan.y += (event.position.y - last.y).as_f32();
                *last = event.position;
                self.touched = true;
                cx.notify();
            }
            Some(Grab::Node { from, moved, .. }) => {
                let dx = (event.position.x - from.x).as_f32();
                let dy = (event.position.y - from.y).as_f32();
                if !*moved && dx.abs().max(dy.abs()) < DRAG_SLOP {
                    return;
                }
                *moved = true;
                // Moves are a preview, applied as they come past the filter;
                // the rest is what the drop would do, and only drawn.
                let mut pending = Vec::new();
                for change in self.gesture(event.position, Phase::Move) {
                    match change {
                        Change::Move { .. } => change::apply(&mut self.canvas, &change),
                        other => pending.push(other),
                    }
                }
                self.pending = pending;
                self.stale = true;
                cx.notify();
            }
        }
    }

    /// The button came up: a node that moved is put back, and its drop is
    /// submitted.
    fn release(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.pending.clear();
        if let Some(Grab::Node { moved: true, .. }) = self.grab {
            let drop = self.gesture(position, Phase::Drop);
            if let Some(Grab::Node {
                id, origin, pinned, ..
            }) = &self.grab
            {
                let (id, to, pinned) = (id.clone(), *origin, *pinned);
                // Back where the press found it, pinned nodes below included.
                let back = Change::Move {
                    id: id.clone(),
                    to,
                    pin: false,
                };
                change::apply(&mut self.canvas, &back);
                if !pinned && let Some(node) = self.canvas.node_mut(&id) {
                    node.extra.remove(mindmap::PINNED);
                }
            }
            self.grab = None;
            for change in drop {
                self.submit(change, cx);
            }
            self.stale = true;
            cx.notify();
        }
        self.grab = None;
    }

    /// The drag handler's answer to the held node at `position`.
    fn gesture(&self, position: Point<Pixels>, phase: Phase) -> Vec<Change> {
        let Some(Grab::Node {
            id, from, origin, ..
        }) = &self.grab
        else {
            return Vec::new();
        };
        let zoom = self.zoom;
        let delta = (
            ((position.x - from.x).as_f32() / zoom).round() as i64,
            ((position.y - from.y).as_f32() / zoom).round() as i64,
        );
        let local = self.local(position);
        let pointer = (
            ((local.x - self.pan.x) / zoom).round() as i64,
            ((local.y - self.pan.y) / zoom).round() as i64,
        );
        let gesture = Drag {
            id,
            origin: *origin,
            delta,
            over: mindmap::node_at(&self.canvas, pointer, id),
            phase,
        };
        (self.drag)(&self.canvas, &gesture)
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        if event.modifiers.platform || event.modifiers.control {
            let zoom = self.zoom * (delta.y.as_f32() * WHEEL_ZOOM).exp();
            self.zoom_about(zoom, self.local(event.position), cx);
        } else {
            self.pan.x += delta.x.as_f32();
            self.pan.y += delta.y.as_f32();
            self.touched = true;
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
            let held = self.held().map(str::to_owned);
            mindmap::layout_holding(&mut self.canvas, held.as_deref());
        }
    }

    /// Where each node paints this frame. A node the document moved glides
    /// there from where it was painted; the held node follows the pointer, and
    /// a node never painted yet starts where it is.
    fn positions(&mut self, reduced: bool, window: &mut Window) -> Positions {
        let held = self.held().map(str::to_owned);
        let fixed =
            |id: &str, shown: &Positions| held.as_deref() == Some(id) || !shown.contains_key(id);
        let to: Positions = self
            .canvas
            .nodes
            .iter()
            .filter(|n| !fixed(&n.id, &self.shown))
            .map(|n| (n.id.clone(), (n.x as f32, n.y as f32)))
            .collect();
        let moved = to.iter().any(|(id, at)| self.shown.get(id) != Some(at));
        if reduced {
            self.glide = None;
        } else if moved && self.glide.as_ref().is_none_or(|glide| glide.to != to) {
            self.glide = Some(Glide {
                from: self.shown.clone(),
                to,
                since: Instant::now(),
            });
        }

        let raw = self.glide.as_ref().map_or(1.0, |glide| {
            let total = LAYOUT.total().mul_f32(motion::speed_scale());
            glide.since.elapsed().as_secs_f32() / total.as_secs_f32().max(f32::EPSILON)
        });
        let t = LAYOUT.progress(raw);
        let shown: Positions = self
            .canvas
            .nodes
            .iter()
            .map(|n| {
                let at = (n.x as f32, n.y as f32);
                let from = self
                    .glide
                    .as_ref()
                    .filter(|_| !fixed(&n.id, &self.shown))
                    .and_then(|glide| glide.from.get(&n.id));
                let at = from.map_or(at, |&from| {
                    (motion::lerp(from.0, at.0, t), motion::lerp(from.1, at.1, t))
                });
                (n.id.clone(), at)
            })
            .collect();
        if raw >= 1.0 {
            self.glide = None;
        } else {
            window.request_animation_frame();
        }
        self.shown = shown.clone();
        shown
    }

    /// Centre the document whenever the view's size changes, until the reader
    /// pans or zooms. An axis the document overflows starts at its edge
    /// instead, so a wide tree still shows its root.
    fn frame(&mut self) {
        let Some(viewport) = self
            .viewport
            .get()
            .filter(|v| !self.touched && self.framed != Some(v.size))
        else {
            return;
        };
        self.framed = Some(viewport.size);
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
            let fit = |view: f32, lo: i64, hi: i64| {
                let span = (hi - lo) as f32 * z;
                let start = if span <= view {
                    (view - span) / 2.0
                } else {
                    PAD * z
                };
                start - lo as f32 * z
            };
            point(fit(size.x, x0, x1), fit(size.y, y0, y1))
        };
    }

    fn paint_node(
        &self,
        ix: usize,
        at: (f32, f32),
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let node = &self.canvas.nodes[ix];
        let z = self.zoom;
        let (x, y) = (self.pan.x + at.0 * z, self.pan.y + at.1 * z);
        let (w, h) = (node.width as f32 * z, node.height as f32 * z);
        if let Some(viewport) = self.viewport.get() {
            let (vw, vh) = (viewport.size.width.as_f32(), viewport.size.height.as_f32());
            if x > vw || y > vh || x + w < 0.0 || y + h < 0.0 {
                return None;
            }
        }
        let kind = kind::kind(cx, &node.kind);
        let editing = self.editing.as_ref().filter(|s| s.id == node.id);
        // A node being typed in keeps the editor's own cursor.
        let draggable = editing.is_none();
        let content = match editing {
            Some(session) => session.editor.clone().into_any_element(),
            None => (kind.render)(node, z, window, cx),
        };
        let grows = kind.sizing == Sizing::Grows;
        let selected = self.selected.as_deref() == Some(node.id.as_str());
        // A drop would land here.
        let target = self
            .pending
            .iter()
            .any(|change| matches!(change, Change::Reparent { parent, .. } if *parent == node.id));
        let border = node
            .color
            .as_deref()
            .and_then(|c| color(theme, c))
            .unwrap_or(theme.border);
        let (id, measured_id) = (node.id.clone(), node.id.clone());
        let (measured, height) = (self.measured.clone(), node.height);
        let ring = |color: Hsla| {
            div()
                .absolute()
                .top(px(-RING))
                .left(px(-RING))
                .right(px(-RING))
                .bottom(px(-RING))
                .rounded(px(RADIUS * z + RING))
                .border_2()
                .border_color(color)
        };

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
            .rounded(px(RADIUS * z))
            .map(|d| match kind.chrome {
                Chrome::Card => d
                    .p(px(PAD * z))
                    .border_1()
                    .border_color(border)
                    .bg(theme.surface_card),
                Chrome::Outline => d.p(px(PAD * z)).border_1().border_color(border),
                Chrome::Bare => d,
            })
            .when(draggable, |d| d.cursor_grab())
            // A box of fixed size keeps whatever a renderer paints inside it.
            .child(if grows {
                content
            } else {
                div()
                    .size_full()
                    .overflow_hidden()
                    .child(content)
                    .into_any_element()
            })
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
            .when(target, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .rounded(px(RADIUS * z))
                        .bg(theme.accent.opacity(TARGET_WASH)),
                )
                .child(ring(theme.accent))
            })
            .when(selected && !target, |d| d.child(ring(theme.accent)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.press_node(id.clone(), event, window, cx);
                }),
            );
        Some(element.into_any_element())
    }

    fn edge_layer(
        &self,
        theme: &Theme,
        shown: &Positions,
        view: WeakEntity<Self>,
    ) -> impl IntoElement + use<> {
        let (z, pan) = (self.zoom, self.pan);
        let grabbing = self.grab.is_some();
        let holding = matches!(self.grab, Some(Grab::Node { .. }));
        // Connectors a drop would cut, and the one it would make.
        let cut: HashSet<&str> = self
            .pending
            .iter()
            .filter_map(|change| match change {
                Change::Reparent { id, .. } | Change::Detach { id } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let mut curves: Vec<Curve> = self
            .canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let mut curve = curve(&self.canvas, shown, edge, theme)?;
                if cut.contains(edge.to_node.as_str()) {
                    curve.color = curve.color.opacity(CUT);
                }
                Some(curve)
            })
            .collect();
        curves.extend(self.pending.iter().filter_map(|change| {
            let Change::Reparent { id, parent } = change else {
                return None;
            };
            let edge = Edge {
                to_end: Some(End::None),
                ..Edge::new("", parent.as_str(), id.as_str())
            };
            let mut preview = curve(&self.canvas, shown, &edge, theme)?;
            preview.color = theme.accent;
            Some(preview)
        }));
        let viewport = self.viewport.clone();
        painter(
            move |bounds, window, _| {
                if viewport.replace(Some(bounds)).map(|b| b.size) != Some(bounds.size) {
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
                // Window-wide, so the hand stays closed wherever the pointer
                // carries the node.
                if holding {
                    window.set_window_cursor_style(CursorStyle::ClosedHand);
                }
                // A held press follows the pointer past the view's edge, so it
                // listens to the window rather than to this element.
                if grabbing {
                    let moves = view.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            let _ = moves.update(cx, |this, cx| this.drag(event, window, cx));
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            let _ = view.update(cx, |this, cx| this.release(event.position, cx));
                        }
                    });
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
        let shown = self.positions(cx.reduced_motion(), window);
        let theme = Theme::of(cx).clone();
        let edges = self.edge_layer(&theme, &shown, cx.entity().downgrade());
        // The held node paints last, over whatever it is carried across.
        let held = self.held().and_then(|id| self.canvas.index_of(id));
        let order = (0..self.canvas.nodes.len())
            .filter(|ix| Some(*ix) != held)
            .chain(held);
        let nodes: Vec<AnyElement> = order
            .filter_map(|ix| {
                let at = shown[&self.canvas.nodes[ix].id];
                self.paint_node(ix, at, &theme, window, cx)
            })
            .collect();

        div()
            .id("bezel-canvas")
            .key_context(key_context())
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
            .on_action(cx.listener(|this, _: &keys::Remove, _, cx| this.remove_selected(cx)))
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
            .on_action(cx.listener(|this, _: &keys::ZoomIn, _, cx| this.zoom_in(cx)))
            .on_action(cx.listener(|this, _: &keys::ZoomOut, _, cx| this.zoom_out(cx)))
            .on_action(cx.listener(|this, _: &keys::ResetZoom, _, cx| this.set_zoom(1.0, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::press_background))
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_pinch(cx.listener(Self::pinch))
            .child(edges)
            .children(nodes)
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

/// A node's box where it paints, in canvas units.
#[derive(Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn of(node: &Node, shown: &Positions) -> Self {
        let (x, y) = shown
            .get(&node.id)
            .copied()
            .unwrap_or((node.x as f32, node.y as f32));
        Self {
            x,
            y,
            w: node.width as f32,
            h: node.height as f32,
        }
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

fn curve(canvas: &Canvas, shown: &Positions, edge: &Edge, theme: &Theme) -> Option<Curve> {
    let a = Rect::of(canvas.node(&edge.from_node)?, shown);
    let b = Rect::of(canvas.node(&edge.to_node)?, shown);
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
fn facing(a: Rect, b: Rect) -> (Side, Side) {
    if b.x >= a.x + a.w {
        (Side::Right, Side::Left)
    } else if b.x + b.w <= a.x {
        (Side::Left, Side::Right)
    } else if b.y >= a.y + a.h {
        (Side::Bottom, Side::Top)
    } else {
        (Side::Top, Side::Bottom)
    }
}

fn anchor(r: Rect, side: Side) -> (Point<f32>, Point<f32>) {
    match side {
        Side::Top => (point(r.x + r.w / 2.0, r.y), point(0.0, -1.0)),
        Side::Right => (point(r.x + r.w, r.y + r.h / 2.0), point(1.0, 0.0)),
        Side::Bottom => (point(r.x + r.w / 2.0, r.y + r.h), point(0.0, 1.0)),
        Side::Left => (point(r.x, r.y + r.h / 2.0), point(-1.0, 0.0)),
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
