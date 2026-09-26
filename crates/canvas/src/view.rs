//! The surface: a [`CanvasEditor`] painted, and keys and the pointer turned
//! into its commands.
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
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, Hsla, KeyContext, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point, Render, ScrollWheelEvent,
    Subscription, WeakEntity, Window, canvas as painter, div, point, prelude::*, px, size,
};
use motion::{AppExt as _, LAYOUT};
use theme::{TextStyle, Theme};
use web_time::Instant;

use crate::{
    change::Change,
    drag::DragHandler,
    edge::{self, EdgeKinds},
    edit::{CanvasEditor, CanvasEvent},
    handle::{Handle, Role, Which},
    kind::{self, Capability, Kinds, Look, PAD, RADIUS, Sizing, color},
    layout::{Arrow, Layout},
    mindmap,
    model::{Canvas, Edge, End, Node},
    options::{Frame, Mark, Options, Overlays, Style},
    path::{Ends, Path, Rect},
    snap::Snap,
    tool::{self, Hand, Hit, Pointer, Sketch, Tool, Wish},
};

mod paint;
mod pointer;

use paint::*;

/// The key context the canvas binds in.
pub const CONTEXT: &str = "BezelCanvas";

/// Claims `tab`, which adds a child here, from `ui::focus` traversal.
fn key_context() -> KeyContext {
    let mut context = KeyContext::default();
    context.add(CONTEXT);
    context.add(ui::focus::CLAIMS_TAB);
    context
}

pub mod keys;

/// Install the canvas key bindings. Call after `editor::init`.
pub fn init(cx: &mut App) {
    cx.bind_keys(keys::bindings());
}

/// Canvas positions, by node id.
type Positions = HashMap<String, (f32, f32)>;

/// Nodes easing from where they were painted toward where the document puts
/// them.
struct Glide {
    from: Positions,
    to: Positions,
    since: Instant,
}

struct Session {
    id: String,
    /// An edge's label, not a node.
    edge: bool,
    input: Entity<Editor>,
    _changes: Subscription,
}

pub struct CanvasView {
    editor: CanvasEditor,
    focus: FocusHandle,
    editing: Option<Session>,
    /// Every gesture, in the order a press is offered to them.
    tools: Vec<Box<dyn Tool>>,
    /// The tool holding the pointer until it comes up.
    holding: Option<usize>,
    style: Style,
    overlays: Overlays,
    /// Where each node was painted last frame.
    shown: Positions,
    glide: Option<Glide>,
    viewport: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Growing node heights read at prepaint, in canvas units.
    measured: Rc<RefCell<HashMap<String, i64>>>,
    /// Where a held press last was, and when its drift last moved the view.
    aim: Option<Point<Pixels>>,
    drifted: Option<Instant>,
}

impl EventEmitter<CanvasEvent> for CanvasView {}

impl Focusable for CanvasView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// An action that runs one editor command.
fn command<A: 'static>(
    cx: &Context<CanvasView>,
    run: impl Fn(&mut CanvasEditor) + 'static,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    cx.listener(move |this, _: &A, _, cx| this.update_editor(cx, &run))
}

impl CanvasView {
    /// `layout` places the nodes; the kinds are what [`kind::set_kinds`]
    /// named, else the spec's.
    pub fn new(canvas: Canvas, layout: Layout, cx: &mut Context<Self>) -> Self {
        Self {
            editor: CanvasEditor::new(canvas, layout).with_kinds(kind::installed(cx)),
            focus: cx.focus_handle(),
            editing: None,
            tools: tool::defaults(),
            holding: None,
            style: Style::default(),
            overlays: Overlays::new(),
            shown: HashMap::new(),
            glide: None,
            viewport: Rc::default(),
            measured: Rc::default(),
            aim: None,
            drifted: None,
        }
    }

    pub fn with_kinds(mut self, kinds: Kinds) -> Self {
        self.editor.set_kinds(kinds);
        self
    }

    /// What the canvas is tuned by.
    pub fn with_options(mut self, options: Options) -> Self {
        self.editor.set_options(options);
        self
    }

    /// What its own paint measures.
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// What a node wears: its ring, a drop's wash, its handles, and the box it
    /// paints as when it is too far out to read.
    pub fn with_overlays(mut self, overlays: Overlays) -> Self {
        self.overlays = overlays;
        self
    }

    /// What an edge is, by its `type`. [`EdgeKinds::new`] unless an app says
    /// otherwise.
    pub fn with_edge_kinds(mut self, kinds: EdgeKinds) -> Self {
        self.editor.set_edge_kinds(kinds);
        self
    }

    /// Every gesture the canvas answers, in the order a press is offered to
    /// them. [`tool::defaults`] unless an app says otherwise.
    pub fn with_tools(mut self, tools: Vec<Box<dyn Tool>>) -> Self {
        self.tools = tools;
        self
    }

    /// What dragging a node does, in place of the layout's own.
    pub fn with_drag(mut self, handler: DragHandler) -> Self {
        self.editor.set_drag(handler);
        self
    }

    /// How dragged and resized boxes settle.
    pub fn with_snap(mut self, snap: Snap) -> Self {
        self.editor.set_snap(snap);
        self
    }

    /// See [`CanvasEditor::with_changes`].
    pub fn with_changes(
        mut self,
        filter: impl Fn(&Canvas, Change) -> Option<Change> + 'static,
    ) -> Self {
        self.editor = self.editor.with_changes(filter);
        self
    }

    pub fn editor(&self) -> &CanvasEditor {
        &self.editor
    }

    /// Run commands on the editor, announcing what they did.
    pub fn update_editor<R>(
        &mut self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut CanvasEditor) -> R,
    ) -> R {
        let answer = update(&mut self.editor);
        self.announce(cx);
        cx.notify();
        answer
    }

    /// Where the view painted last frame, in window coordinates.
    pub fn bounds(&self) -> Option<Bounds<Pixels>> {
        self.viewport.get()
    }

    /// What `cmd-c` does.
    pub fn copy(&self, cx: &mut App) {
        if let Some(json) = self.editor.copy() {
            cx.write_to_clipboard(ClipboardItem::new_string(json));
        }
    }

    /// What `cmd-x` does.
    pub fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        self.update_editor(cx, CanvasEditor::remove_selected);
    }

    /// What `cmd-v` does.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.update_editor(cx, |editor| editor.paste(&text));
        }
    }

    /// Emit what the editor did, and close an editor whose target is gone or
    /// was taken back.
    fn announce(&mut self, cx: &mut Context<Self>) {
        let canvas = self.editor.canvas();
        let gone = self.editing.as_ref().is_some_and(|s| {
            if s.edge {
                canvas.edge(&s.id).is_none()
            } else {
                canvas.node(&s.id).is_none()
            }
        });
        if self.editor.take_rewound() || gone {
            self.editing = None;
        }
        // An open editor types at the zoom.
        if let Some(session) = &self.editing {
            let text = TextStyle::Body.painted() * self.editor.zoom();
            session
                .input
                .update(cx, |input, cx| input.set_text_size(text, cx));
        }
        for event in self.editor.take_events() {
            cx.emit(event);
        }
    }

    fn local(&self, position: Point<Pixels>) -> Point<f32> {
        let origin = self.viewport.get().map_or(Point::default(), |b| b.origin);
        point(
            (position.x - origin.x).as_f32(),
            (position.y - origin.y).as_f32(),
        )
    }

    fn canvas_point(&self, position: Point<Pixels>) -> Point<f32> {
        let (local, pan, zoom) = (self.local(position), self.editor.pan(), self.editor.zoom());
        point((local.x - pan.x) / zoom, (local.y - pan.y) / zoom)
    }

    /// The node a drag has in hand, once it has moved.
    fn held(&self) -> Option<&str> {
        self.holding.and_then(|ix| self.tools[ix].held())
    }

    /// What the tool holding the pointer draws.
    fn sketch(&self) -> Option<Sketch> {
        self.holding.and_then(|ix| self.tools[ix].sketch())
    }

    /// Edit what a command just added, its typing joining the add.
    fn edit_added(&mut self, added: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = added {
            self.grant(Wish::EditAdded(id), window, cx);
        }
    }

    /// Edit `id` in place, its typing one undo step: `group`'s, or a new one.
    fn edit(
        &mut self,
        id: String,
        group: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_editing(window, cx);
        let Some(node) = self.editor.canvas().node(&id) else {
            return;
        };
        let Some(field) = self.editor.kinds().get(&node.kind).rules.edit.clone() else {
            return;
        };
        let input = self.input(&(field.read)(node), cx);
        let changes = cx.subscribe(&input, move |this, input, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let Some(mut node) = this
                .editing
                .as_ref()
                .filter(|s| !s.edge)
                .and_then(|s| this.editor.canvas().node(&s.id))
                .cloned()
            else {
                return;
            };
            (field.write)(&mut node, input.read(cx).source());
            this.update_editor(cx, |editor| editor.submit([Change::UpdateNode { node }]));
        });
        window.focus(&input.focus_handle(cx), cx);
        // A press that opened it would hand focus back to the canvas.
        window.prevent_default();
        self.editor.group = Some(match group {
            Some(group) => group,
            None => self.editor.next_group(),
        });
        self.editing = Some(Session {
            id,
            edge: false,
            input,
            _changes: changes,
        });
        cx.notify();
    }

    /// Edit an edge's label in place, its typing one undo step.
    fn edit_edge(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_editing(window, cx);
        let Some(edge) = self.editor.canvas().edge(&id) else {
            return;
        };
        let Some(field) = self.editor.edge_kinds().get(edge).rules.edit.clone() else {
            return;
        };
        let input = self.input(&(field.read)(edge), cx);
        let changes = cx.subscribe(&input, move |this, input, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let Some(mut edge) = this
                .editing
                .as_ref()
                .filter(|s| s.edge)
                .and_then(|s| this.editor.canvas().edge(&s.id))
                .cloned()
            else {
                return;
            };
            (field.write)(&mut edge, input.read(cx).source());
            this.update_editor(cx, |editor| editor.submit([Change::UpdateEdge { edge }]));
        });
        window.focus(&input.focus_handle(cx), cx);
        window.prevent_default();
        self.editor.group = Some(self.editor.next_group());
        self.editing = Some(Session {
            id,
            edge: true,
            input,
            _changes: changes,
        });
        cx.notify();
    }

    /// An editor for text typed in place, at the zoom.
    fn input(&self, value: &str, cx: &mut Context<Self>) -> Entity<Editor> {
        let size = TextStyle::Body.painted() * self.editor.zoom();
        cx.new(|cx| {
            Editor::new(value, cx)
                .with_chrome(EditorChrome {
                    handle: false,
                    slash: false,
                    language: false,
                    paste: false,
                })
                .with_text_size(size)
        })
    }

    fn stop_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.group = None;
        if self.editing.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }
}

impl Render for CanvasView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(bounds) = self.viewport.get() {
            let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
            self.editor.set_viewport(size(w, h));
        }
        self.drift(window, cx);
        let measured: Vec<(String, i64)> = self.measured.borrow_mut().drain().collect();
        let held = self.held().map(str::to_owned);
        self.editor.reflow(measured, held.as_deref());
        self.editor.frame();
        self.announce(cx);
        let shown = self.positions(cx.reduced_motion(), window);
        let theme = Theme::of(cx).clone();
        let edges = self.edge_layer(&theme, &shown, cx.entity().downgrade());
        // A container paints under what it holds. What a drag has in hand
        // paints last, with what it holds, over whatever it is carried across.
        let canvas = self.editor.painted();
        let containment = self.editor.painted_containment();
        let carried: HashSet<String> = match self.held() {
            Some(id) => containment
                .with_contents(&[id.to_owned()])
                .into_iter()
                .collect(),
            None => HashSet::new(),
        };
        let mut order: Vec<usize> = (0..canvas.nodes.len()).collect();
        order.sort_by_key(|ix| {
            let id = &canvas.nodes[*ix].id;
            (carried.contains(id), containment.depth(id))
        });
        let connecting = self.connect_target();
        let picked = self.editor.selected_nodes();
        let nodes: Vec<AnyElement> = order
            .into_iter()
            .filter_map(|ix| {
                let at = shown[&canvas.nodes[ix].id];
                self.paint_node(ix, at, connecting.as_deref(), &picked, window, cx)
            })
            .collect();
        let labels = self.labels(&theme, &shown, cx);
        let edge_handles = self.edge_handles(&theme, &shown, cx);
        let marquee = match self.sketch() {
            Some(Sketch::Box { from, to }) => {
                let (pan, zoom) = (self.editor.pan(), self.editor.zoom());
                let a = point(pan.x + from.x * zoom, pan.y + from.y * zoom);
                let b = point(pan.x + to.x * zoom, pan.y + to.y * zoom);
                Some(
                    div()
                        .absolute()
                        .left(px(a.x.min(b.x)))
                        .top(px(a.y.min(b.y)))
                        .w(px((a.x - b.x).abs()))
                        .h(px((a.y - b.y).abs()))
                        .border_1()
                        .border_color(theme.accent)
                        .bg(theme.accent.opacity(self.style.marquee_wash)),
                )
            }
            _ => None,
        };

        div()
            .id("bezel-canvas")
            .key_context(key_context())
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .overflow_hidden()
            .on_action(cx.listener(|this, _: &keys::AddChild, window, cx| {
                let added = this.update_editor(cx, CanvasEditor::add_child);
                this.edit_added(added, window, cx);
            }))
            .on_action(cx.listener(|this, _: &keys::AddSibling, window, cx| {
                let added = this.update_editor(cx, CanvasEditor::add_sibling);
                this.edit_added(added, window, cx);
            }))
            .on_action(command::<keys::Remove>(cx, CanvasEditor::remove_selected))
            .on_action(cx.listener(|this, _: &keys::Edit, window, cx| {
                if let Some(id) = this.editor.selected_edge().map(str::to_owned) {
                    this.edit_edge(id, window, cx);
                } else if let Some(id) = this.editor.selected().map(str::to_owned) {
                    this.edit(id, None, window, cx);
                }
            }))
            .on_action(command::<keys::SelectAll>(cx, CanvasEditor::select_all))
            .on_action(cx.listener(|this, _: &keys::Deselect, _, cx| {
                this.cancel(cx);
                this.update_editor(cx, |editor| {
                    editor.select(None);
                    editor.select_edge(None);
                });
            }))
            .on_action(command::<keys::Undo>(cx, |editor| {
                editor.undo();
            }))
            .on_action(command::<keys::Redo>(cx, |editor| {
                editor.redo();
            }))
            .on_action(cx.listener(|this, _: &keys::Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &keys::Cut, _, cx| this.cut(cx)))
            .on_action(cx.listener(|this, _: &keys::Paste, _, cx| this.paste(cx)))
            .on_action(command::<keys::Duplicate>(cx, CanvasEditor::duplicate))
            .on_action(
                cx.listener(|this, _: &keys::StopEditing, window, cx| {
                    this.stop_editing(window, cx)
                }),
            )
            .on_action(command::<keys::SelectLeft>(cx, |e| {
                e.select_toward(Arrow::Left)
            }))
            .on_action(command::<keys::SelectRight>(cx, |e| {
                e.select_toward(Arrow::Right)
            }))
            .on_action(command::<keys::SelectUp>(cx, |e| {
                e.select_toward(Arrow::Up)
            }))
            .on_action(command::<keys::SelectDown>(cx, |e| {
                e.select_toward(Arrow::Down)
            }))
            .on_action(command::<keys::NudgeLeft>(cx, |e| e.nudge(Arrow::Left)))
            .on_action(command::<keys::NudgeRight>(cx, |e| e.nudge(Arrow::Right)))
            .on_action(command::<keys::NudgeUp>(cx, |e| e.nudge(Arrow::Up)))
            .on_action(command::<keys::NudgeDown>(cx, |e| e.nudge(Arrow::Down)))
            .on_action(command::<keys::ZoomIn>(cx, CanvasEditor::zoom_in))
            .on_action(command::<keys::ZoomOut>(cx, CanvasEditor::zoom_out))
            .on_action(command::<keys::ResetZoom>(cx, |e| e.set_zoom(1.0)))
            .on_action(command::<keys::Fit>(cx, CanvasEditor::fit))
            .on_action(command::<keys::ZoomToSelection>(
                cx,
                CanvasEditor::zoom_to_selection,
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    let hit = this.hit_at(event.position);
                    this.press(event, hit, window, cx);
                }),
            )
            .on_mouse_down(
                MouseButton::Middle,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.press(event, Hit::Nothing, window, cx);
                }),
            )
            .on_scroll_wheel(cx.listener(Self::wheel))
            .on_pinch(cx.listener(Self::pinch))
            .child(edges)
            .children(labels)
            .children(nodes)
            .children(edge_handles)
            .children(marquee)
    }
}
