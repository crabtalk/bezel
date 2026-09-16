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
    Subscription, WeakEntity, Window, canvas as painter, div, fill, point, prelude::*, px, size,
};
use motion::{AppExt as _, LAYOUT};
use theme::{TextStyle, Theme};
use web_time::Instant;

use crate::{
    change::Change,
    contain,
    drag::DragHandler,
    edit::{CanvasEditor, CanvasEvent},
    kind::{self, Capability, Kinds, Look, PAD, RADIUS, Sizing, color},
    layout::{Arrow, Layout},
    mindmap,
    model::{Canvas, Edge, End, Node, Side},
    snap::{Axis, Snap},
    tool::{self, Hand, Hit, Part, Pointer, Sketch, Tool, Wish},
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

/// Zoom per pixel of a modified wheel.
const WHEEL_ZOOM: f32 = 0.01;
/// Arrowhead length, in canvas units.
const ARROW: f32 = 8.0;
/// How far the selection ring sits outside a node, in screen pixels.
const RING: f32 = 3.0;
/// The accent wash inside a marquee.
const MARQUEE_WASH: f32 = 0.08;
/// A picked node's side and corner handles, in screen pixels.
const HANDLE: f32 = 8.0;
/// How near a press must come to an edge to pick it, in screen pixels.
const EDGE_REACH: f32 = 6.0;
/// The room an edge label is centred in, in canvas units.
const LABEL: (f32, f32) = (240.0, 32.0);
/// How much of a connector a drop would cut still shows.
const CUT: f32 = 0.25;
/// The accent wash inside a node a drop would land on.
const TARGET_WASH: f32 = 0.12;
/// The most one frame of drift may travel, in seconds, however late it ran.
const DRIFT_STEP: f32 = 0.05;
/// Below this zoom a node paints as its box, its content unread.
const FAR_ZOOM: f32 = 0.4;
/// The closest grid dots come, in screen pixels; a denser grid skips rows.
const DOT_SPACING: f32 = 12.0;
/// A grid dot, in screen pixels.
const DOT: f32 = 1.5;

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
            SelectLeft,
            SelectRight,
            SelectUp,
            SelectDown,
            NudgeLeft,
            NudgeRight,
            NudgeUp,
            NudgeDown,
            SelectAll,
            Deselect,
            Undo,
            Redo,
            Copy,
            Cut,
            Paste,
            Duplicate,
            Fit,
            ZoomToSelection,
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
            KeyBinding::new("left", SelectLeft, ctx),
            KeyBinding::new("right", SelectRight, ctx),
            KeyBinding::new("up", SelectUp, ctx),
            KeyBinding::new("down", SelectDown, ctx),
            KeyBinding::new("shift-left", NudgeLeft, ctx),
            KeyBinding::new("shift-right", NudgeRight, ctx),
            KeyBinding::new("shift-up", NudgeUp, ctx),
            KeyBinding::new("shift-down", NudgeDown, ctx),
            KeyBinding::new("escape", Deselect, ctx),
            KeyBinding::new("shift-1", Fit, ctx),
            KeyBinding::new("shift-2", ZoomToSelection, ctx),
            KeyBinding::new("escape", StopEditing, Some(&editing)),
        ];
        let platform = if cfg!(target_os = "macos") {
            "cmd"
        } else {
            "ctrl"
        };
        let chord = |key: &str| format!("{platform}-{key}");
        bindings.extend([
            KeyBinding::new(&chord("a"), SelectAll, ctx),
            KeyBinding::new(&chord("z"), Undo, ctx),
            KeyBinding::new(&chord("shift-z"), Redo, ctx),
            KeyBinding::new(&chord("c"), Copy, ctx),
            KeyBinding::new(&chord("x"), Cut, ctx),
            KeyBinding::new(&chord("v"), Paste, ctx),
            KeyBinding::new(&chord("d"), Duplicate, ctx),
            KeyBinding::new(&chord("="), ZoomIn, ctx),
            KeyBinding::new(&chord("-"), ZoomOut, ctx),
            KeyBinding::new(&chord("0"), ResetZoom, ctx),
        ]);
        bindings
    }
}

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
        let input = self.input(edge.label.as_deref().unwrap_or_default(), cx);
        let changes = cx.subscribe(&input, |this, input, event, cx| {
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
            let label = input.read(cx).source();
            edge.label = (!label.is_empty()).then_some(label);
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

    /// What a press landed on. A node and its handles answer for themselves,
    /// as they are painted over the canvas; the background asks the edges.
    fn hit_at(&self, position: Point<Pixels>) -> Hit {
        match self.edge_at(position) {
            Some(id) => Hit::Edge(id),
            None => Hit::Nothing,
        }
    }

    fn pointer(&self, screen: Point<Pixels>, hit: Hit) -> Pointer {
        Pointer {
            screen,
            at: self.canvas_point(screen),
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
            clicks: 1,
            hit,
        }
    }

    /// Offer a press to the tools: the first that takes it holds the pointer
    /// until it comes up.
    fn press(
        &mut self,
        event: &MouseDownEvent,
        hit: Hit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A press inside what is being typed in belongs to the editor.
        let typing = self.editing.as_ref().is_some_and(|session| match &hit {
            Hit::Node(id) => !session.edge && session.id == *id,
            Hit::Edge(id) => session.edge && session.id == *id,
            _ => false,
        });
        if typing {
            return;
        }
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        let pointer = Pointer {
            button: event.button,
            modifiers: event.modifiers,
            clicks: event.click_count,
            ..self.pointer(event.position, hit)
        };
        self.aim = Some(event.position);
        let mut wish = Wish::Nothing;
        self.holding = None;
        for (ix, tool) in self.tools.iter_mut().enumerate() {
            let mut hand = Hand::new(&mut self.editor);
            if tool.press(&pointer, &mut hand) {
                wish = hand.wish;
                self.holding = Some(ix);
                break;
            }
        }
        self.announce(cx);
        cx.notify();
        self.grant(wish, window, cx);
    }

    /// What the tool asked of the view once its commands had landed.
    fn grant(&mut self, wish: Wish, window: &mut Window, cx: &mut Context<Self>) {
        match wish {
            Wish::Nothing => {}
            Wish::Enter(id) => {
                let node = self.editor.canvas().node(&id).cloned();
                let open = node
                    .as_ref()
                    .and_then(|node| self.editor.kinds().get(&node.kind).open.clone());
                match (open, node) {
                    (Some(open), Some(node)) => open(&node, cx),
                    _ => self.edit(id, None, window, cx),
                }
            }
            Wish::EditEdge(id) => self.edit_edge(id, window, cx),
            // Typing into what was added joins the add in one undo step.
            Wish::EditAdded(id) => {
                let group = self.editor.last_group();
                self.edit(id, group, window, cx);
            }
        }
    }

    /// Hand the held tool where the pointer is now.
    fn feed(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(ix) = self.holding else {
            return;
        };
        let pointer = self.pointer(position, Hit::Nothing);
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].drag(&pointer, &mut hand);
        self.announce(cx);
        cx.notify();
    }

    /// The button came up: the held tool lands its gesture.
    fn lift(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        (self.aim, self.drifted) = (None, None);
        let Some(ix) = self.holding.take() else {
            return;
        };
        let pointer = self.pointer(position, Hit::Nothing);
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].release(&pointer, &mut hand);
        let wish = hand.wish;
        self.announce(cx);
        cx.notify();
        self.grant(wish, window, cx);
    }

    /// Give up the gesture in hand, as `escape` does.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        let Some(ix) = self.holding.take() else {
            return;
        };
        let mut hand = Hand::new(&mut self.editor);
        self.tools[ix].cancel(&mut hand);
        self.announce(cx);
        cx.notify();
    }

    /// The node a connector being drawn would reach.
    fn connect_target(&self) -> Option<String> {
        let Some(Sketch::Connector { from, to, .. }) = self.sketch() else {
            return None;
        };
        let at = (to.x.round() as i64, to.y.round() as i64);
        self.editor.node_under(at, &from).map(str::to_owned)
    }

    /// The topmost edge passing near a window position.
    fn edge_at(&self, position: Point<Pixels>) -> Option<String> {
        let at = self.canvas_point(position);
        let reach = EDGE_REACH / self.editor.zoom();
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        canvas
            .edges
            .iter()
            .rev()
            .find(|edge| curve(&nodes, &self.shown, edge).is_some_and(|c| c.distance(at) <= reach))
            .map(|edge| edge.id.clone())
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(window.line_height());
        if event.modifiers.platform || event.modifiers.control {
            let zoom = self.editor.zoom() * (delta.y.as_f32() * WHEEL_ZOOM).exp();
            let anchor = self.local(event.position);
            self.update_editor(cx, |editor| editor.zoom_about(zoom, anchor));
        } else {
            self.update_editor(cx, |editor| {
                editor.pan_by(delta.x.as_f32(), delta.y.as_f32())
            });
        }
        cx.stop_propagation();
    }

    /// The pointer moved with a button down: the tool holding it hears.
    fn moved(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.lift(event.position, window, cx);
            return;
        }
        self.aim = Some(event.position);
        self.feed(event.position, cx);
    }

    fn pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let zoom = self.editor.zoom() * (1.0 + event.delta);
        let anchor = self.local(event.position);
        self.update_editor(cx, |editor| editor.zoom_about(zoom, anchor));
    }

    /// Pan while a held press rests near the view's edge, carrying what it
    /// holds along.
    fn drift(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drifting = self.holding.is_some_and(|ix| self.tools[ix].drifts());
        let (Some(aim), Some(bounds), true) = (self.aim, self.viewport.get(), drifting) else {
            self.drifted = None;
            return;
        };
        let velocity = point(
            ui::scroll::drift_velocity(aim.x, bounds.left(), bounds.right()),
            ui::scroll::drift_velocity(aim.y, bounds.top(), bounds.bottom()),
        );
        if velocity.x == 0.0 && velocity.y == 0.0 {
            self.drifted = None;
            return;
        }
        window.request_animation_frame();
        let now = Instant::now();
        // The first frame starts the clock; there is no interval to travel yet.
        let Some(last) = self.drifted.replace(now) else {
            return;
        };
        let step = (now - last).as_secs_f32().min(DRIFT_STEP);
        self.editor.pan_by(velocity.x * step, velocity.y * step);
        // The pointer has not moved, but the canvas under it has.
        self.feed(aim, cx);
    }

    /// Where each node paints this frame. A node the document moved glides
    /// there from where it was painted; the held node follows the pointer, and
    /// a node never painted yet starts where it is.
    fn positions(&mut self, reduced: bool, window: &mut Window) -> Positions {
        let held = self.held().map(str::to_owned);
        let fixed =
            |id: &str, shown: &Positions| held.as_deref() == Some(id) || !shown.contains_key(id);
        let canvas = self.editor.painted();
        let to: Positions = canvas
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
        let shown: Positions = canvas
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

    #[allow(clippy::too_many_arguments)]
    fn paint_node(
        &self,
        ix: usize,
        at: (f32, f32),
        connecting: Option<&str>,
        picked: &[&str],
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let node = &self.editor.painted().nodes[ix];
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let (x, y) = (pan.x + at.0 * z, pan.y + at.1 * z);
        let (w, h) = (node.width as f32 * z, node.height as f32 * z);
        if let Some(viewport) = self.viewport.get() {
            let (vw, vh) = (viewport.size.width.as_f32(), viewport.size.height.as_f32());
            if x > vw || y > vh || x + w < 0.0 || y + h < 0.0 {
                return None;
            }
        }
        let kind = self.editor.kinds().get(&node.kind);
        let editing = self.editing.as_ref().filter(|s| !s.edge && s.id == node.id);
        // A node being typed in keeps the editor's own cursor.
        let draggable = editing.is_none();
        // Far out a node is a plain box, its content too small to read.
        let far = z < FAR_ZOOM && draggable;
        let content = if far {
            div()
                .size_full()
                .rounded(px(RADIUS * z))
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface_card)
                .into_any_element()
        } else {
            let look = Look {
                zoom: z,
                editor: editing.map(|session| session.input.clone().into_any_element()),
            };
            (kind.render)(node, look, window, cx)
        };
        let grows = kind.rules.sizing == Sizing::Grows && !far;
        let selected = picked.contains(&node.id.as_str());
        // A drop, or a connector let go here, would connect to this node.
        let target = connecting == Some(node.id.as_str())
            || self.held() != Some(node.id.as_str())
                && self.editor.pending.iter().any(|change| {
                    matches!(change, Change::AddEdge { edge, .. }
                        if edge.from_node == node.id || edge.to_node == node.id)
                });
        // A node picked alone, with nothing held, shows the handles it allows.
        let handles = selected && draggable && picked.len() == 1 && self.holding.is_none();
        let connects = handles && self.editor.node_can(&node.id, Capability::Connectable);
        let resizes = handles && self.editor.node_can(&node.id, Capability::Resizable);
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

        // The box is the canvas's; everything painted inside it is the kind's.
        let element = div()
            .id(ElementId::Name(node.id.clone().into()))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .flex()
            .flex_col()
            .map(|d| {
                if grows {
                    let least = kind::min_height(node).unwrap_or(mindmap::NODE_HEIGHT);
                    d.min_h(px(least as f32 * z))
                } else {
                    d.h(px(h))
                }
            })
            .when(draggable, |d| d.cursor_grab())
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
            .when(connects || resizes, |d| {
                let handle = |key: String, x: f32, y: f32| {
                    div()
                        .id(ElementId::Name(key.into()))
                        .absolute()
                        .left(px(x - HANDLE / 2.0))
                        .top(px(y - HANDLE / 2.0))
                        .size(px(HANDLE))
                        .border_1()
                        .border_color(theme.accent)
                        .bg(theme.surface_card)
                };
                let sides = [
                    (Side::Top, w / 2.0, 0.0),
                    (Side::Right, w, h / 2.0),
                    (Side::Bottom, w / 2.0, h),
                    (Side::Left, 0.0, h / 2.0),
                ];
                let corner = node.id.clone();
                d.when(connects, |d| {
                    d.children(sides.map(|(side, x, y)| {
                        let from = node.id.clone();
                        handle(format!("{}-{side:?}", node.id), x, y)
                            .rounded_full()
                            .cursor(CursorStyle::Crosshair)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    let hit = Hit::Handle {
                                        node: from.clone(),
                                        part: Part::Connect(side),
                                    };
                                    this.press(event, hit, window, cx);
                                }),
                            )
                    }))
                })
                .when(resizes, |d| {
                    d.child(
                        handle(format!("{}-corner", node.id), w, h)
                            .rounded(px(2.0))
                            .cursor(CursorStyle::ResizeUpLeftDownRight)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    let hit = Hit::Handle {
                                        node: corner.clone(),
                                        part: Part::Resize,
                                    };
                                    this.press(event, hit, window, cx);
                                }),
                            ),
                    )
                })
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.press(event, Hit::Node(id.clone()), window, cx);
                }),
            );
        Some(element.into_any_element())
    }

    /// Edge labels, at the middle of their curves, and the one being typed.
    fn labels(&self, theme: &Theme, shown: &Positions, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let editing = self.editing.as_ref().filter(|s| s.edge && s.id == edge.id);
                let text = edge.label.as_deref().filter(|label| !label.is_empty());
                if editing.is_none() && (text.is_none() || z < FAR_ZOOM) {
                    return None;
                }
                let middle = curve(&nodes, shown, edge)?.middle();
                let (x, y) = (pan.x + middle.x * z, pan.y + middle.y * z);
                let body = match editing {
                    Some(session) => div()
                        .min_w(px(LABEL.0 / 2.0 * z))
                        .child(session.input.clone())
                        .into_any_element(),
                    None => kind::text_style(div(), TextStyle::Callout, z)
                        .text_color(theme.text_muted)
                        .child(text.unwrap_or_default().to_owned())
                        .into_any_element(),
                };
                let picked = self.editor.selected_edge() == Some(edge.id.as_str());
                let id = edge.id.clone();
                let label = div()
                    .id(ElementId::Name(format!("edge-label-{}", edge.id).into()))
                    .px(px(PAD / 2.0 * z))
                    .rounded(px(RADIUS * z))
                    .border_1()
                    .border_color(if picked { theme.accent } else { theme.border })
                    .bg(theme.surface_card)
                    .child(body)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.press(event, Hit::Edge(id.clone()), window, cx);
                        }),
                    );
                Some(
                    div()
                        .absolute()
                        .left(px(x - LABEL.0 * z / 2.0))
                        .top(px(y - LABEL.1 * z / 2.0))
                        .w(px(LABEL.0 * z))
                        .h(px(LABEL.1 * z))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(label)
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn edge_layer(
        &self,
        theme: &Theme,
        shown: &Positions,
        view: WeakEntity<Self>,
    ) -> impl IntoElement + use<> {
        let (z, pan) = (self.editor.zoom(), self.editor.pan());
        let grabbing = self.holding.is_some();
        let holding = self.held().is_some();
        let pending = &self.editor.pending;
        // Connectors a drop would cut, and the ones it would make.
        let cut: HashSet<&str> = pending
            .iter()
            .flat_map(|change| match change {
                Change::RemoveEdges { ids } => ids.as_slice(),
                _ => &[],
            })
            .map(String::as_str)
            .collect();
        // Each curve in view, its colour and its weight.
        let canvas = self.editor.painted();
        let nodes = canvas.lookup();
        let seen = self.editor.visible();
        let picked = self.editor.selected_edge();
        let mut strokes: Vec<(Curve, Hsla, f32)> = canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let curve = curve(&nodes, shown, edge)?;
                let (x0, y0, x1, y1) = curve.hull();
                if seen.is_some_and(|(x, y, w, h)| x1 < x || x0 > x + w || y1 < y || y0 > y + h) {
                    return None;
                }
                if picked == Some(edge.id.as_str()) {
                    return Some((curve, theme.accent, 2.0));
                }
                let mut paint = edge
                    .color
                    .as_deref()
                    .and_then(|c| color(theme, c))
                    .unwrap_or(theme.border_strong);
                if cut.contains(edge.id.as_str()) {
                    paint = paint.opacity(CUT);
                }
                Some((curve, paint, 1.0))
            })
            .collect();
        strokes.extend(pending.iter().filter_map(|change| {
            let Change::AddEdge { edge, .. } = change else {
                return None;
            };
            Some((curve(&nodes, shown, edge)?, theme.accent, 1.0))
        }));
        // The connector being drawn, out to the pointer.
        if let Some(Sketch::Connector { from, side, to }) = self.sketch()
            && let Some(node) = canvas.node(&from)
        {
            let (start, out) = anchor(Rect::of(node, shown), side);
            let loose = Curve {
                from: start,
                from_out: out,
                to,
                to_out: point(-out.x, -out.y),
                from_arrow: false,
                to_arrow: true,
            };
            strokes.push((loose, theme.accent, 1.0));
        }
        let dots = self.editor.snap().grid.map(|grid| grid as f32);
        let (dot, accent) = (theme.border, theme.accent);
        let guides = self.editor.guides.clone();
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
                if let Some(grid) = dots {
                    let mut step = grid * z;
                    while step < DOT_SPACING {
                        step *= 2.0;
                    }
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    let mut y = pan.y.rem_euclid(step);
                    while y < h {
                        let mut x = pan.x.rem_euclid(step);
                        while x < w {
                            let at =
                                point(px(origin.x + x - DOT / 2.0), px(origin.y + y - DOT / 2.0));
                            window.paint_quad(fill(Bounds::new(at, size(px(DOT), px(DOT))), dot));
                            x += step;
                        }
                        y += step;
                    }
                }
                for (curve, paint, weight) in strokes {
                    let [(p0, c0, mid), (_, c1, p1)] = curve
                        .segments()
                        .map(|(a, c, b)| (screen(a), screen(c), screen(b)));
                    let mut path = PathBuilder::stroke(px(weight * z.max(1.0)));
                    path.move_to(pt(p0));
                    path.curve_to(pt(mid), pt(c0));
                    path.curve_to(pt(p1), pt(c1));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, paint);
                    }
                    if curve.to_arrow {
                        arrow(window, p1, curve.to_out, ARROW * z, paint);
                    }
                    if curve.from_arrow {
                        arrow(window, p0, curve.from_out, ARROW * z, paint);
                    }
                }
                for guide in guides {
                    let (at, from, to) = (guide.at as f32, guide.from as f32, guide.to as f32);
                    let (a, b) = match guide.axis {
                        Axis::X => (point(at, from), point(at, to)),
                        Axis::Y => (point(from, at), point(to, at)),
                    };
                    let mut path = PathBuilder::stroke(px(1.0));
                    path.move_to(pt(screen(a)));
                    path.line_to(pt(screen(b)));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, accent);
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
                            let _ = moves.update(cx, |this, cx| this.moved(event, window, cx));
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        let held = matches!(event.button, MouseButton::Left | MouseButton::Middle);
                        if phase == DispatchPhase::Bubble && held {
                            let _ =
                                view.update(cx, |this, cx| this.lift(event.position, window, cx));
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
        let holds = |node: &Node| self.editor.kinds().holds(node);
        let depths = contain::depths(canvas, holds);
        let carried: HashSet<String> = match self.held() {
            Some(id) => contain::with_contents(canvas, &[id.to_owned()], holds)
                .into_iter()
                .collect(),
            None => HashSet::new(),
        };
        let mut order: Vec<usize> = (0..canvas.nodes.len()).collect();
        order.sort_by_key(|ix| {
            let id = &canvas.nodes[*ix].id;
            (carried.contains(id), depths.get(id).copied().unwrap_or(0))
        });
        let connecting = self.connect_target();
        let picked = self.editor.selected_nodes();
        let nodes: Vec<AnyElement> = order
            .into_iter()
            .filter_map(|ix| {
                let at = shown[&canvas.nodes[ix].id];
                self.paint_node(ix, at, connecting.as_deref(), &picked, &theme, window, cx)
            })
            .collect();
        let labels = self.labels(&theme, &shown, cx);
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
                        .bg(theme.accent.opacity(MARQUEE_WASH)),
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
            .children(marquee)
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

/// An edge where it paints, in canvas units.
struct Curve {
    from: Point<f32>,
    /// Out of the node, at the anchor.
    from_out: Point<f32>,
    to: Point<f32>,
    to_out: Point<f32>,
    from_arrow: bool,
    to_arrow: bool,
}

impl Curve {
    /// The two quadratic halves painted: start, control, end.
    fn segments(&self) -> [(Point<f32>, Point<f32>, Point<f32>); 2] {
        let (p0, p1) = (self.from, self.to);
        let reach = (p1.x - p0.x).abs().max((p1.y - p0.y).abs()) / 2.0;
        let c0 = point(
            p0.x + self.from_out.x * reach,
            p0.y + self.from_out.y * reach,
        );
        let c1 = point(p1.x + self.to_out.x * reach, p1.y + self.to_out.y * reach);
        let mid = point((c0.x + c1.x) / 2.0, (c0.y + c1.y) / 2.0);
        [(p0, c0, mid), (mid, c1, p1)]
    }

    fn middle(&self) -> Point<f32> {
        self.segments()[0].2
    }

    /// A box the curve stays inside: left, top, right, bottom.
    fn hull(&self) -> (f32, f32, f32, f32) {
        let points = self.segments().into_iter().flat_map(|(a, c, b)| [a, c, b]);
        points.fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
        )
    }

    /// How far `at` is from the curve.
    fn distance(&self, at: Point<f32>) -> f32 {
        const STEPS: usize = 16;
        let mut nearest = f32::MAX;
        for (a, c, b) in self.segments() {
            let along = |t: f32| {
                let u = 1.0 - t;
                point(
                    u * u * a.x + 2.0 * u * t * c.x + t * t * b.x,
                    u * u * a.y + 2.0 * u * t * c.y + t * t * b.y,
                )
            };
            let mut last = a;
            for step in 1..=STEPS {
                let next = along(step as f32 / STEPS as f32);
                nearest = nearest.min(to_segment(at, last, next));
                last = next;
            }
        }
        nearest
    }
}

/// How far `p` is from the segment `a`–`b`.
fn to_segment(p: Point<f32>, a: Point<f32>, b: Point<f32>) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    };
    ((p.x - a.x - t * dx).powi(2) + (p.y - a.y - t * dy).powi(2)).sqrt()
}

fn curve(nodes: &HashMap<&str, &Node>, shown: &Positions, edge: &Edge) -> Option<Curve> {
    let a = Rect::of(nodes.get(edge.from_node.as_str())?, shown);
    let b = Rect::of(nodes.get(edge.to_node.as_str())?, shown);
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
