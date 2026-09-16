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
    EventEmitter, FocusHandle, Focusable, Hsla, KeyContext, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point, Render, ScrollWheelEvent,
    Subscription, WeakEntity, Window, canvas as painter, div, fill, point, prelude::*, px, size,
};
use motion::{AppExt as _, LAYOUT};
use theme::{TextStyle, Theme};
use web_time::Instant;

use crate::{
    change::{self, Change},
    contain,
    drag::{Drag, DragHandler, Phase},
    edit::{CanvasEditor, CanvasEvent},
    kind::{self, Kinds, Look, PAD, RADIUS, Sizing, color},
    layout::{Arrow, Layout},
    mindmap,
    model::{Canvas, Edge, End, Node, Side},
    snap::{self, Axis, Guide, Snap},
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
/// How far a press on a node travels before it is a drag, in screen pixels.
const DRAG_SLOP: f32 = 3.0;
/// The accent wash inside a marquee.
const MARQUEE_WASH: f32 = 0.08;
/// A picked node's side and corner handles, in screen pixels.
const HANDLE: f32 = 8.0;
/// How near a press must come to an edge to pick it, in screen pixels.
const EDGE_REACH: f32 = 6.0;
/// The smallest box a corner pulls a node to, in canvas units.
const MIN_SIZE: (i64, i64) = (60, 32);
/// The room an edge label is centred in, in canvas units.
const LABEL: (f32, f32) = (240.0, 32.0);
/// How much of a connector a drop would cut still shows.
const CUT: f32 = 0.25;
/// The accent wash inside a node a drop would land on.
const TARGET_WASH: f32 = 0.12;
/// The most one frame of drift may travel, in seconds, however late it ran.
const DRIFT_STEP: f32 = 0.05;
/// How near a dragged box's line comes to another's before it catches, in
/// screen pixels.
const GUIDE_REACH: f32 = 6.0;
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

/// What a held button is moving.
enum Grab {
    Pan(Point<Pixels>),
    Node {
        id: String,
        /// The rest of the selection, carried along.
        with: Vec<String>,
        from: Point<Pixels>,
        origin: (i64, i64),
        moved: bool,
        /// The pan at the press, so a drift keeps the node under the pointer.
        pan: Point<f32>,
    },
    /// A box drawn from `from`, in canvas units so a drift carries it, to the
    /// pointer, selecting what it touches beside `base`.
    Marquee {
        from: Point<f32>,
        to: Point<Pixels>,
        base: Vec<String>,
    },
    /// A connector drawn out of `from`'s `side`, pressed at `start`, now at `to`.
    Connect {
        from: String,
        side: Side,
        start: Point<Pixels>,
        to: Point<Pixels>,
    },
    /// `id`'s corner pulled from `start`; `grows` pulls a height its content
    /// may run past.
    Resize {
        id: String,
        start: Point<Pixels>,
        grows: bool,
        /// The pan at the press.
        pan: Point<f32>,
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
    /// An edge's label, not a node.
    edge: bool,
    input: Entity<Editor>,
    _changes: Subscription,
}

pub struct CanvasView {
    editor: CanvasEditor,
    focus: FocusHandle,
    editing: Option<Session>,
    grab: Option<Grab>,
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
    /// Painted with the kinds [`kind::set_kinds`] named, else the spec's.
    pub fn new(canvas: Canvas, cx: &mut Context<Self>) -> Self {
        Self {
            editor: CanvasEditor::new(canvas).with_kinds(kind::installed(cx)),
            focus: cx.focus_handle(),
            editing: None,
            grab: None,
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

    /// Who places the nodes. [`crate::layout::MINDMAP`] unless an app says
    /// otherwise.
    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.editor.set_layout(layout);
        self
    }

    /// What dragging a node does. [`crate::drag::pin`] unless an app says
    /// otherwise.
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

    /// The canvas point under a window position.
    fn to_canvas(&self, position: Point<Pixels>) -> (i64, i64) {
        let at = self.canvas_point(position);
        (at.x.round() as i64, at.y.round() as i64)
    }

    fn canvas_point(&self, position: Point<Pixels>) -> Point<f32> {
        let (local, pan, zoom) = (self.local(position), self.editor.pan(), self.editor.zoom());
        point((local.x - pan.x) / zoom, (local.y - pan.y) / zoom)
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

    /// Edit what the editor just added, its typing joining the add.
    fn edit_added(&mut self, added: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = added {
            let group = self.editor.last_group();
            self.edit(id, group, window, cx);
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

    fn press_background(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        if let Some(id) = self.edge_at(event.position) {
            self.press_edge(id, event, window, cx);
            return;
        }
        if event.modifiers.shift {
            self.grab = Some(Grab::Marquee {
                from: self.canvas_point(event.position),
                to: event.position,
                base: self
                    .editor
                    .selected_nodes()
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            });
            cx.notify();
            return;
        }
        self.update_editor(cx, |editor| {
            editor.select(None);
            editor.select_edge(None);
        });
        if event.click_count >= 2 {
            // A double-click on nothing makes a root there.
            let at = self.to_canvas(event.position);
            let added = self.update_editor(cx, |editor| editor.add_root(at));
            self.edit_added(added, window, cx);
            return;
        }
        self.press_pan(event, window, cx);
    }

    fn press_pan(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.grab = Some(Grab::Pan(event.position));
        // The listeners that follow the grab are painted next frame.
        cx.notify();
    }

    fn press_edge(
        &mut self,
        id: String,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.as_ref().is_some_and(|s| s.edge && s.id == id) {
            return;
        }
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        self.update_editor(cx, |editor| editor.select_edge(Some(id.clone())));
        if event.click_count >= 2 {
            self.edit_edge(id, window, cx);
        }
    }

    fn press_connect(
        &mut self,
        from: String,
        side: Side,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.grab = Some(Grab::Connect {
            from,
            side,
            start: event.position,
            to: event.position,
        });
        cx.notify();
    }

    fn press_resize(&mut self, id: String, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(node) = self.editor.canvas().node(&id) else {
            return;
        };
        let grows = self.editor.kinds().get(&node.kind).rules.sizing == Sizing::Grows;
        self.grab = Some(Grab::Resize {
            id,
            start: event.position,
            grows,
            pan: self.editor.pan(),
        });
        cx.notify();
    }

    /// The node a connector being drawn would reach.
    fn connect_target(&self) -> Option<&str> {
        let Some(Grab::Connect { from, to, .. }) = &self.grab else {
            return None;
        };
        self.editor.node_under(self.to_canvas(*to), from)
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

    fn press_node(
        &mut self,
        id: String,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.as_ref().is_some_and(|s| !s.edge && s.id == id) {
            return;
        }
        self.stop_editing(window, cx);
        window.focus(&self.focus, cx);
        let selected: Vec<String> = self
            .editor
            .selected_nodes()
            .into_iter()
            .map(str::to_owned)
            .collect();
        if event.modifiers.shift || event.modifiers.platform {
            let mut ids = selected;
            match ids.iter().position(|s| *s == id) {
                Some(at) => drop(ids.remove(at)),
                None => ids.push(id),
            }
            self.update_editor(cx, |editor| editor.set_selection(ids));
            return;
        }
        if event.click_count >= 2 {
            self.update_editor(cx, |editor| editor.select(Some(id.clone())));
            let node = self.editor.canvas().node(&id).cloned();
            let open = node
                .as_ref()
                .and_then(|node| self.editor.kinds().get(&node.kind).open.clone());
            match (open, node) {
                (Some(open), Some(node)) => open(&node, cx),
                _ => self.edit(id, None, window, cx),
            }
            return;
        }
        // Pressing one of a selection keeps the rest, to drag them together.
        let mut with: Vec<String> = selected.iter().filter(|s| **s != id).cloned().collect();
        if with.len() == selected.len() {
            with.clear();
        }
        let ids = with.iter().cloned().chain([id.clone()]).collect();
        self.update_editor(cx, |editor| editor.set_selection(ids));
        if let Some(node) = self.editor.canvas().node(&id) {
            self.grab = Some(Grab::Node {
                origin: (node.x, node.y),
                id,
                with,
                from: event.position,
                moved: false,
                pan: self.editor.pan(),
            });
            cx.notify();
        }
    }

    fn drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.release(event.position, window, cx);
            return;
        }
        self.aim = Some(event.position);
        match &mut self.grab {
            None => {}
            Some(Grab::Pan(last)) => {
                let dx = (event.position.x - last.x).as_f32();
                let dy = (event.position.y - last.y).as_f32();
                *last = event.position;
                self.editor.pan_by(dx, dy);
                cx.notify();
            }
            Some(Grab::Marquee { to, .. }) => {
                *to = event.position;
                self.mark(cx);
            }
            Some(Grab::Connect { to, .. }) => {
                *to = event.position;
                cx.notify();
            }
            Some(Grab::Resize { .. }) => self.pull(event.position, cx),
            Some(Grab::Node { from, moved, .. }) => {
                let dx = (event.position.x - from.x).as_f32();
                let dy = (event.position.y - from.y).as_f32();
                if !*moved && dx.abs().max(dy.abs()) < DRAG_SLOP {
                    return;
                }
                *moved = true;
                self.preview(event.position, cx);
            }
        }
    }

    /// The held node at `position`: its moves shown over the document, and the
    /// rest of what the drop would do drawn.
    fn preview(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(grab) = &self.grab else {
            return;
        };
        let (changes, guides) = self.gesture(grab, position, Phase::Move);
        let mut shown = self.editor.canvas().clone();
        let mut pending = Vec::new();
        for change in changes {
            match change {
                Change::MoveNodes { .. } => drop(change::apply(&mut shown, &change)),
                other => pending.push(other),
            }
        }
        self.editor.set_preview(shown, pending, guides);
        cx.notify();
    }

    /// The held corner pulled to `position`, on the grid when there is one,
    /// shown over the document.
    fn pull(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(Grab::Resize {
            id,
            start,
            grows,
            pan,
        }) = &self.grab
        else {
            return;
        };
        let (z, now, snap) = (self.editor.zoom(), self.editor.pan(), self.editor.snap());
        let pulled = |to: Pixels, from: Pixels, then: f32, now: f32| {
            (((to - from).as_f32() - (now - then)) / z).round() as i64
        };
        let grid = |value: i64| snap.grid.map_or(value, |g| snap::round_to(value, g));
        let Some(mut node) = self.editor.canvas().node(id).cloned() else {
            return;
        };
        let width = grid(node.width + pulled(position.x, start.x, pan.x, now.x));
        let height = grid(node.height + pulled(position.y, start.y, pan.y, now.y));
        (node.width, node.height) = (width.max(MIN_SIZE.0), height.max(MIN_SIZE.1));
        // Measuring keeps a growing node as tall as its content.
        if *grows {
            node.extra
                .insert(kind::MIN_HEIGHT.into(), node.height.into());
        }
        let mut shown = self.editor.canvas().clone();
        change::apply(&mut shown, &Change::UpdateNode { node });
        self.editor.set_preview(shown, Vec::new(), Vec::new());
        cx.notify();
    }

    /// The button came up: the preview goes, and the drop is submitted against
    /// the document as it was.
    fn release(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        (self.aim, self.drifted) = (None, None);
        let Some(grab) = self.grab.take() else {
            return;
        };
        match &grab {
            Grab::Node {
                moved: true,
                id,
                with,
                ..
            } => {
                self.editor.clear_preview();
                let (mut drop, _) = self.gesture(&grab, position, Phase::Drop);
                let held: Vec<String> = std::iter::once(id.clone())
                    .chain(with.iter().cloned())
                    .collect();
                // A node carried out of the container it names lets it go.
                let mut after = self.editor.canvas().clone();
                change::apply_all(&mut after, &drop);
                drop.extend(contain::loosen(&after, &held));
                self.update_editor(cx, |editor| editor.submit(drop));
            }
            // A click on one of a selection, without a drag, selects just it.
            Grab::Node { id, .. } => {
                let id = id.clone();
                self.update_editor(cx, |editor| editor.select(Some(id)));
            }
            Grab::Marquee { .. } => cx.notify(),
            Grab::Connect {
                from,
                side,
                start,
                to,
            } => {
                let pulled = (to.x - start.x)
                    .as_f32()
                    .abs()
                    .max((to.y - start.y).as_f32().abs());
                if pulled >= DRAG_SLOP {
                    self.connect(from.clone(), *side, *to, window, cx);
                }
                cx.notify();
            }
            // The pull lands as one change: a resize, or for a growing node the
            // node with its least height.
            Grab::Resize { id, grows, .. } => {
                let now = self.editor.painted().node(id).cloned();
                self.editor.clear_preview();
                let Some(now) = now.filter(|now| self.editor.canvas().node(id) != Some(now)) else {
                    cx.notify();
                    return;
                };
                let change = match grows {
                    true => Change::UpdateNode { node: now },
                    false => Change::Resize {
                        size: (now.width, now.height),
                        id: now.id,
                    },
                };
                self.update_editor(cx, |editor| editor.submit([change]));
            }
            Grab::Pan(_) => {}
        }
    }

    /// A connector let go of at `position`.
    fn connect(
        &mut self,
        from: String,
        side: Side,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.to_canvas(position);
        let added = self.update_editor(cx, |editor| editor.connect(&from, side, at));
        self.edit_added(added, window, cx);
    }

    /// Select what the marquee touches, beside what was selected before it.
    fn mark(&mut self, cx: &mut Context<Self>) {
        let Some(Grab::Marquee { from, to, base }) = &self.grab else {
            return;
        };
        let (a, b) = (
            (from.x.round() as i64, from.y.round() as i64),
            self.to_canvas(*to),
        );
        let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
        let touched = self.editor.painted().nodes.iter().filter(|n| {
            n.x < x1
                && n.x + n.width > x0
                && n.y < y1
                && n.y + n.height > y0
                && !base.contains(&n.id)
        });
        let ids = base
            .iter()
            .cloned()
            .chain(touched.map(|n| n.id.clone()))
            .collect();
        self.update_editor(cx, |editor| editor.set_selection(ids));
    }

    /// The drag handler's answer to the held node at `position`, against the
    /// document, settled by the snap, and the guides that caught it.
    fn gesture(
        &self,
        grab: &Grab,
        position: Point<Pixels>,
        phase: Phase,
    ) -> (Vec<Change>, Vec<Guide>) {
        let Grab::Node {
            id,
            with,
            from,
            origin,
            pan,
            ..
        } = grab
        else {
            return (Vec::new(), Vec::new());
        };
        let editor = &self.editor;
        let (zoom, now) = (editor.zoom(), editor.pan());
        // The pointer's travel, less what the view drifted under it.
        let travel = |to: Pixels, from: Pixels, then: f32, now: f32| {
            (((to - from).as_f32() - (now - then)) / zoom).round() as i64
        };
        let mut delta = (
            travel(position.x, from.x, pan.x, now.x),
            travel(position.y, from.y, pan.y, now.y),
        );
        let held: Vec<String> = std::iter::once(id.clone())
            .chain(with.iter().cloned())
            .collect();
        let holds = |node: &Node| editor.kinds().holds(node);
        let canvas = editor.canvas();
        let carried = contain::with_contents(canvas, &held, holds);
        let contents: Vec<String> = carried
            .iter()
            .filter(|id| !held.contains(id))
            .cloned()
            .collect();
        let mut guides = Vec::new();
        if editor.snap() != Snap::default()
            && let Some(node) = canvas.node(id)
        {
            let moving: Vec<String> = carried
                .iter()
                .flat_map(|id| mindmap::branch_of(canvas, id))
                .collect();
            let to = (origin.0 + delta.0, origin.1 + delta.1);
            let reach = (GUIDE_REACH / zoom).round() as i64;
            let size = (node.width, node.height);
            let (settled, caught) =
                snap::settle(editor.painted(), &moving, to, size, editor.snap(), reach);
            delta = (settled.0 - origin.0, settled.1 - origin.1);
            guides = caught;
        }
        let pointer = self.to_canvas(position);
        let gesture = Drag {
            id,
            with,
            contents: &contents,
            origin: *origin,
            delta,
            over: mindmap::node_at(editor.painted(), pointer, &held, |n| !holds(n)),
            phase,
        };
        ((editor.drag())(canvas, &gesture), guides)
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

    fn pinch(&mut self, event: &PinchEvent, _: &mut Window, cx: &mut Context<Self>) {
        let zoom = self.editor.zoom() * (1.0 + event.delta);
        let anchor = self.local(event.position);
        self.update_editor(cx, |editor| editor.zoom_about(zoom, anchor));
    }

    /// Pan while a held press rests near the view's edge, carrying what it
    /// holds along.
    fn drift(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let holding = matches!(
            self.grab,
            Some(
                Grab::Node { moved: true, .. }
                    | Grab::Marquee { .. }
                    | Grab::Connect { .. }
                    | Grab::Resize { .. }
            )
        );
        let (Some(aim), Some(bounds), true) = (self.aim, self.viewport.get(), holding) else {
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
        match self.grab {
            Some(Grab::Node { .. }) => self.preview(aim, cx),
            Some(Grab::Resize { .. }) => self.pull(aim, cx),
            Some(Grab::Marquee { .. }) => self.mark(cx),
            _ => {}
        }
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
        // A node picked alone, with nothing held, shows its handles.
        let handles = selected && draggable && picked.len() == 1 && self.grab.is_none();
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
            .when(handles, |d| {
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
                d.children(sides.map(|(side, x, y)| {
                    let from = node.id.clone();
                    handle(format!("{}-{side:?}", node.id), x, y)
                        .rounded_full()
                        .cursor(CursorStyle::Crosshair)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.press_connect(from.clone(), side, event, cx);
                            }),
                        )
                }))
                .child(
                    handle(format!("{}-corner", node.id), w, h)
                        .rounded(px(2.0))
                        .cursor(CursorStyle::ResizeUpLeftDownRight)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.press_resize(corner.clone(), event, cx);
                            }),
                        ),
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
                            this.press_edge(id.clone(), event, window, cx);
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
        let grabbing = self.grab.is_some();
        let holding = matches!(self.grab, Some(Grab::Node { .. }));
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
        if let Some(Grab::Connect { from, side, to, .. }) = &self.grab
            && let Some(node) = canvas.node(from)
        {
            let (start, out) = anchor(Rect::of(node, shown), *side);
            let end = self.canvas_point(*to);
            let loose = Curve {
                from: start,
                from_out: out,
                to: end,
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
                            let _ = moves.update(cx, |this, cx| this.drag(event, window, cx));
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        let held = matches!(event.button, MouseButton::Left | MouseButton::Middle);
                        if phase == DispatchPhase::Bubble && held {
                            let _ = view
                                .update(cx, |this, cx| this.release(event.position, window, cx));
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
        let connecting = self.connect_target().map(str::to_owned);
        let picked = self.editor.selected_nodes();
        let nodes: Vec<AnyElement> = order
            .into_iter()
            .filter_map(|ix| {
                let at = shown[&canvas.nodes[ix].id];
                self.paint_node(ix, at, connecting.as_deref(), &picked, &theme, window, cx)
            })
            .collect();
        let labels = self.labels(&theme, &shown, cx);
        let marquee = match &self.grab {
            Some(Grab::Marquee { from, to, .. }) => {
                let (pan, zoom) = (self.editor.pan(), self.editor.zoom());
                let a = point(pan.x + from.x * zoom, pan.y + from.y * zoom);
                let b = self.local(*to);
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
            .on_action(command::<keys::Deselect>(cx, |editor| {
                editor.select(None);
                editor.select_edge(None);
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
            .on_mouse_down(MouseButton::Left, cx.listener(Self::press_background))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::press_pan))
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
