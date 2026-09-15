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
    AnyElement, App, Bounds, ClipboardItem, Context, CursorStyle, DispatchPhase, ElementId, Entity,
    EventEmitter, FocusHandle, Focusable, Hsla, KeyContext, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point, Render, Rgba,
    ScrollWheelEvent, Size, Subscription, WeakEntity, Window, canvas as painter, div, point,
    prelude::*, px,
};
use motion::{AppExt as _, LAYOUT};
use theme::{TextStyle, Theme};
use web_time::Instant;

use crate::{
    change::{self, Change},
    clip,
    drag::{self, Drag, DragHandler, Phase},
    kind::{self, Chrome, Sizing},
    layout::{self, Arrow, Layout},
    mindmap,
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
/// One `shift`-arrow, in canvas units.
const NUDGE: i64 = 8;
/// How far a duplicate sits from what it copies, in canvas units.
const DUPLICATE: i64 = 24;
/// Undo steps kept.
const HISTORY: usize = 200;
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
/// A coloured frame's wash of its colour.
const FRAME_WASH: f32 = 0.06;

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

/// Install the canvas key bindings, and the spec's kinds unless an app set its
/// own. Call after `editor::init`.
pub fn init(cx: &mut App) {
    kind::ensure(cx);
    cx.bind_keys(keys::bindings());
}

// A change is handed on and dropped, never kept in bulk; boxing its node would
// only put a `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    /// A batch landed in the document.
    Changed(Vec<Change>),
    /// The selection, the primary last.
    Selected(Vec<String>),
    /// The picked edge.
    EdgeSelected(Option<String>),
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
        /// The rest of the selection, carried along.
        with: Vec<String>,
        from: Point<Pixels>,
        origin: (i64, i64),
        moved: bool,
        /// Where each node the preview moved sat before, to put back on a drop.
        before: HashMap<String, (i64, i64)>,
    },
    /// A box drawn from `from`, selecting what it touches beside `base`.
    Marquee {
        from: Point<Pixels>,
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
    /// `id`'s corner pulled from `start`: `size` is its box before, and `grows`
    /// leaves the height to its content.
    Resize {
        id: String,
        start: Point<Pixels>,
        size: (i64, i64),
        grows: bool,
    },
}

/// One undoable edit: the changes that take it back, and the typing session it
/// belongs to, which later typing joins.
struct Step {
    changes: Vec<Change>,
    group: Option<u64>,
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
    editor: Entity<Editor>,
    _changes: Subscription,
}

pub struct CanvasView {
    canvas: Canvas,
    layout: Layout,
    drag: DragHandler,
    filter: Option<Filter>,
    focus: FocusHandle,
    /// The primary is last.
    selected: Vec<String>,
    /// The picked edge, apart from the nodes.
    edge: Option<String>,
    editing: Option<Session>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The typing session edits now join, and the last one handed out.
    group: Option<u64>,
    groups: u64,
    /// Screen position of the canvas origin, from the view's top left.
    pan: Point<f32>,
    zoom: f32,
    grab: Option<Grab>,
    /// What the drop would do, drawn while a node is held: the drag handler's
    /// answer to the last move, less its `MoveNodes`.
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
            layout: layout::MINDMAP,
            drag: drag::pin,
            filter: None,
            focus: cx.focus_handle(),
            selected: Vec::new(),
            edge: None,
            editing: None,
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
            groups: 0,
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

    /// Who places the nodes. [`layout::MINDMAP`] unless an app says otherwise.
    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
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

    /// A new document, with no history.
    pub fn set_canvas(&mut self, canvas: Canvas, cx: &mut Context<Self>) {
        self.canvas = canvas;
        self.editing = None;
        (self.undo, self.redo) = (Vec::new(), Vec::new());
        self.prune(cx);
        self.stale = true;
        cx.notify();
    }

    /// Land a batch as if the reader made it: each change through the filter
    /// first, against the document before the batch. `false` when one was
    /// refused, and then none lands.
    pub fn submit(
        &mut self,
        changes: impl IntoIterator<Item = Change>,
        cx: &mut Context<Self>,
    ) -> bool {
        let changes: Vec<Change> = match self.filter.clone() {
            Some(filter) => {
                let canvas = &self.canvas;
                match changes
                    .into_iter()
                    .map(|change| filter(canvas, change, cx))
                    .collect()
                {
                    Some(changes) => changes,
                    None => return false,
                }
            }
            None => changes.into_iter().collect(),
        };
        self.apply(changes, cx);
        true
    }

    /// Land a batch of the app's own, past the filter. It can be undone.
    pub fn apply(&mut self, changes: impl IntoIterator<Item = Change>, cx: &mut Context<Self>) {
        let changes: Vec<Change> = changes.into_iter().collect();
        if changes.is_empty() {
            return;
        }
        let undo = change::apply_all(&mut self.canvas, &changes);
        if !undo.is_empty() {
            self.redo.clear();
            match self.undo.last_mut() {
                Some(step) if self.group.is_some() && step.group == self.group => {
                    step.changes.splice(0..0, undo);
                }
                _ => {
                    self.undo.push(Step {
                        changes: undo,
                        group: self.group,
                    });
                    if self.undo.len() > HISTORY {
                        self.undo.remove(0);
                    }
                }
            }
        }
        self.landed(changes, cx);
    }

    /// What `cmd-z` does: take back the last edit, past the filter. `false`
    /// when there is none.
    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(step) = self.undo.pop() else {
            return false;
        };
        let redo = change::apply_all(&mut self.canvas, &step.changes);
        self.redo.push(Step {
            changes: redo,
            group: None,
        });
        self.rewound(step.changes, cx);
        true
    }

    /// What `cmd-shift-z` does.
    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(step) = self.redo.pop() else {
            return false;
        };
        let undo = change::apply_all(&mut self.canvas, &step.changes);
        self.undo.push(Step {
            changes: undo,
            group: None,
        });
        self.rewound(step.changes, cx);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// An open editor would hold what was taken back, and typing after starts
    /// a step of its own.
    fn rewound(&mut self, changes: Vec<Change>, cx: &mut Context<Self>) {
        self.group = None;
        self.editing = None;
        self.landed(changes, cx);
    }

    /// Tidy up after the document changed, and announce what did.
    fn landed(&mut self, changes: Vec<Change>, cx: &mut Context<Self>) {
        let gone = self.editing.as_ref().is_some_and(|s| {
            if s.edge {
                self.canvas.edge(&s.id).is_none()
            } else {
                self.canvas.node(&s.id).is_none()
            }
        });
        if gone {
            self.editing = None;
        }
        self.prune(cx);
        self.stale = true;
        cx.emit(CanvasEvent::Changed(changes));
        cx.notify();
    }

    /// The primary selection: the last chosen, which the keys act from.
    pub fn selected(&self) -> Option<&str> {
        self.selected.last().map(String::as_str)
    }

    /// Everything selected, the primary last.
    pub fn selection(&self) -> &[String] {
        &self.selected
    }

    /// Select only `id`, or nothing.
    pub fn select(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.set_selection(id.into_iter().collect(), cx);
    }

    pub fn set_selection(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        if !ids.is_empty() {
            self.select_edge(None, cx);
        }
        if self.selected != ids {
            self.selected = ids.clone();
            cx.emit(CanvasEvent::Selected(ids));
            cx.notify();
        }
    }

    pub fn selected_edge(&self) -> Option<&str> {
        self.edge.as_deref()
    }

    /// Pick an edge, or none. Picking one lets the nodes go.
    pub fn select_edge(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if id.is_some() {
            self.set_selection(Vec::new(), cx);
        }
        if self.edge != id {
            self.edge = id.clone();
            cx.emit(CanvasEvent::EdgeSelected(id));
            cx.notify();
        }
    }

    /// What `cmd-a` does.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let ids = self
            .canvas
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect();
        self.set_selection(ids, cx);
    }

    /// Let go of what the document no longer holds.
    fn prune(&mut self, cx: &mut Context<Self>) {
        let kept = self
            .selected
            .iter()
            .filter(|id| self.canvas.node(id).is_some())
            .cloned()
            .collect();
        self.set_selection(kept, cx);
        if self
            .edge
            .as_deref()
            .is_some_and(|id| self.canvas.edge(id).is_none())
        {
            self.select_edge(None, cx);
        }
    }

    /// What `cmd-c` does: the selection, with its branches under a tree, as
    /// JSON Canvas.
    pub fn copy(&self, cx: &mut App) {
        if !self.selected.is_empty() {
            let fragment = clip::fragment(&self.canvas, &self.reach());
            cx.write_to_clipboard(ClipboardItem::new_string(fragment.to_json()));
        }
    }

    /// What `cmd-x` does.
    pub fn cut(&mut self, cx: &mut Context<Self>) {
        self.copy(cx);
        self.remove_selected(cx);
    }

    /// What `cmd-v` does: a copied canvas, or text as a text node — under the
    /// selection in a tree, else in the middle of the view.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        let fragment = Canvas::parse(&text).unwrap_or_else(|_| {
            let blank = (kind::kind(cx, model::TEXT).child)(&Node::default());
            Canvas {
                nodes: vec![Node {
                    text: Some(text),
                    ..blank
                }],
                ..Canvas::default()
            }
        });
        let under = self.layout.flow.and(self.selected().map(str::to_owned));
        let at = match under.as_deref().and_then(|id| self.canvas.node(id)) {
            Some(parent) => (parent.x + parent.width + mindmap::GAP_X, parent.y),
            None => {
                let (x, y) = self.center();
                let (_, (w, h)) = clip::bounds(&fragment).unwrap_or_default();
                (x - w / 2, y - h / 2)
            }
        };
        self.place(&fragment, at, under.as_deref(), cx);
    }

    /// What `cmd-d` does: the selection copied beside itself, or under the same
    /// parent in a tree.
    pub fn duplicate(&mut self, cx: &mut Context<Self>) {
        let Some(primary) = self.selected().map(str::to_owned) else {
            return;
        };
        let fragment = clip::fragment(&self.canvas, &self.reach());
        let Some(((x, y), _)) = clip::bounds(&fragment) else {
            return;
        };
        let under = self
            .layout
            .flow
            .and_then(|_| mindmap::parent(&self.canvas, &primary))
            .map(str::to_owned);
        self.place(
            &fragment,
            (x + DUPLICATE, y + DUPLICATE),
            under.as_deref(),
            cx,
        );
    }

    /// A fragment added through the filter, and selected.
    fn place(
        &mut self,
        fragment: &Canvas,
        at: (i64, i64),
        under: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let changes = clip::paste(&self.canvas, fragment, at, under);
        let added = changes
            .iter()
            .filter_map(|change| match change {
                Change::AddNode { node, .. } => Some(node.id.clone()),
                _ => None,
            })
            .collect();
        if self.submit(changes, cx) {
            self.set_selection(added, cx);
        }
    }

    /// The selection, each with its branch under a layout that grows trees.
    fn reach(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for id in &self.selected {
            let branch = match self.layout.flow {
                Some(_) => mindmap::branch_of(&self.canvas, id),
                None => vec![id.clone()],
            };
            for id in branch {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids
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

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Switching to a tree that grows another way tidies the document: every
    /// pin is dropped and the trees are laid out again.
    pub fn set_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        let tidy = layout.flow.is_some() && layout.flow != self.layout.flow;
        self.layout = layout;
        if tidy {
            let unpins: Vec<Change> = self
                .canvas
                .nodes
                .iter()
                .filter_map(mindmap::unpin)
                .collect();
            self.apply(unpins, cx);
        }
        self.stale = true;
        cx.notify();
    }

    /// Swap what dragging a node does, from the next press.
    pub fn set_drag(&mut self, handler: DragHandler) {
        self.drag = handler;
    }

    /// What `backspace` does, through the filter: the picked edge, or the
    /// selection, each node with its branch under a layout that grows trees.
    pub fn remove_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.edge.clone() {
            self.submit([Change::RemoveEdges { ids: vec![id] }], cx);
            return;
        }
        let Some(primary) = self.selected().map(str::to_owned) else {
            return;
        };
        let next = mindmap::after_removal(&self.canvas, &primary);
        if self.submit([Change::RemoveNodes { ids: self.reach() }], cx) {
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

    /// The canvas point under a window position.
    fn to_canvas(&self, position: Point<Pixels>) -> (i64, i64) {
        let local = self.local(position);
        (
            ((local.x - self.pan.x) / self.zoom).round() as i64,
            ((local.y - self.pan.y) / self.zoom).round() as i64,
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

    fn added(&mut self, changes: Option<Vec<Change>>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(changes) = changes else { return };
        let Some(id) = change::added(&changes).map(str::to_owned) else {
            return;
        };
        // Typing into what was added joins the add in one undo step.
        let group = self.next_group();
        self.group = Some(group);
        if self.submit(changes, cx) && self.canvas.node(&id).is_some() {
            self.select(Some(id.clone()), cx);
            self.edit(id, Some(group), window, cx);
        }
        if self.editing.is_none() {
            self.group = None;
        }
    }

    fn next_group(&mut self) -> u64 {
        self.groups += 1;
        self.groups
    }

    fn add_child(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Nothing selected: under the first root, or a root of its own.
        let parent = self.selected().map(str::to_owned).or_else(|| {
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
                Some(vec![mindmap::root(&self.canvas, node, (0, 0))])
            }
        };
        self.added(change, window, cx);
    }

    /// A root has no siblings, so Enter on one adds a child.
    fn add_sibling(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(of) = self.selected().map(str::to_owned) else {
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

    /// An arrow walks the tree the layout grows, or to the nearest node.
    fn arrow(&mut self, arrow: Arrow, cx: &mut Context<Self>) {
        let next = match (self.selected(), self.layout.flow) {
            (Some(id), Some(flow)) => mindmap::walk(&self.canvas, id, flow, arrow),
            (Some(id), None) => layout::nearest(&self.canvas, id, arrow).map(str::to_owned),
            (None, _) => mindmap::roots(&self.canvas).next().map(|n| n.id.clone()),
        };
        if next.is_some() {
            self.select(next, cx);
        }
    }

    /// A `shift`-arrow moves the selection, pinned under a tree.
    fn nudge(&mut self, arrow: Arrow, cx: &mut Context<Self>) {
        if self.selected.is_empty() {
            return;
        }
        let (dx, dy) = arrow.unit();
        let pin = self.layout.flow.is_some();
        let by = (dx * NUDGE, dy * NUDGE);
        let changes = mindmap::carry(&self.canvas, &self.selected, by, pin);
        self.submit(changes, cx);
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
        let Some(node) = self.canvas.node(&id) else {
            return;
        };
        let Some(field) = kind::kind(cx, &node.kind).edit else {
            return;
        };
        let value = (field.read)(node);
        let editor = self.editor(&value, cx);
        let changes = cx.subscribe(&editor, move |this, editor, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let Some(mut node) = this
                .editing
                .as_ref()
                .filter(|s| !s.edge)
                .and_then(|s| this.canvas.node(&s.id))
                .cloned()
            else {
                return;
            };
            (field.write)(&mut node, editor.read(cx).source());
            this.submit([Change::UpdateNode { node }], cx);
        });
        window.focus(&editor.focus_handle(cx), cx);
        // A press that opened it would hand focus back to the canvas.
        window.prevent_default();
        self.group = Some(match group {
            Some(group) => group,
            None => self.next_group(),
        });
        self.editing = Some(Session {
            id,
            edge: false,
            editor,
            _changes: changes,
        });
        cx.notify();
    }

    /// Edit an edge's label in place, its typing one undo step.
    fn edit_edge(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_editing(window, cx);
        let Some(edge) = self.canvas.edge(&id) else {
            return;
        };
        let editor = self.editor(edge.label.as_deref().unwrap_or_default(), cx);
        let changes = cx.subscribe(&editor, |this, editor, event, cx| {
            if *event != EditorEvent::Changed {
                return;
            }
            let Some(mut edge) = this
                .editing
                .as_ref()
                .filter(|s| s.edge)
                .and_then(|s| this.canvas.edge(&s.id))
                .cloned()
            else {
                return;
            };
            let label = editor.read(cx).source();
            edge.label = (!label.is_empty()).then_some(label);
            this.submit([Change::UpdateEdge { edge }], cx);
        });
        window.focus(&editor.focus_handle(cx), cx);
        window.prevent_default();
        self.group = Some(self.next_group());
        self.editing = Some(Session {
            id,
            edge: true,
            editor,
            _changes: changes,
        });
        cx.notify();
    }

    /// An editor for text typed in place, at the zoom.
    fn editor(&self, value: &str, cx: &mut Context<Self>) -> Entity<Editor> {
        let size = TextStyle::Body.painted() * self.zoom;
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
        self.group = None;
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
                from: event.position,
                to: event.position,
                base: self.selected.clone(),
            });
            cx.notify();
            return;
        }
        self.select(None, cx);
        self.select_edge(None, cx);
        if event.click_count >= 2 {
            // A double-click on nothing makes a root there.
            let node = (kind::kind(cx, model::TEXT).child)(&Node::default());
            let (x, y) = self.to_canvas(event.position);
            let at = (x - node.width / 2, y - node.height / 2);
            let root = mindmap::root(&self.canvas, node, at);
            self.added(Some(vec![root]), window, cx);
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
        self.select_edge(Some(id.clone()), cx);
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
        let Some(node) = self.canvas.node(&id) else {
            return;
        };
        let grows = kind::kind(cx, &node.kind).sizing == Sizing::Grows;
        self.grab = Some(Grab::Resize {
            size: (node.width, node.height),
            id,
            start: event.position,
            grows,
        });
        cx.notify();
    }

    /// A connector let go of at `position`: onto the node there, else to a new
    /// node, a child under a tree. Between two nodes under a tree, it is a
    /// cross link.
    fn connect(
        &mut self,
        from: String,
        side: Side,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at = self.to_canvas(position);
        let tree = self.layout.flow.is_some();
        if let Some(to) = self.node_under(at, &from).map(str::to_owned) {
            let mut edge = Edge {
                from_side: Some(side),
                ..Edge::new(self.canvas.mint(), from, to)
            };
            if tree {
                edge.extra.insert(mindmap::TREE.into(), false.into());
            }
            self.submit([Change::AddEdge { edge, index: None }], cx);
            return;
        }
        let Some(mut node) = self.template(&from, cx) else {
            return;
        };
        let changes = if tree {
            mindmap::child(&self.canvas, &from, node)
        } else {
            let Ok([id, edge]) = <[String; 2]>::try_from(self.canvas.mint_n(2)) else {
                return;
            };
            node.id = id;
            (node.x, node.y) = (at.0 - node.width / 2, at.1 - node.height / 2);
            let edge = Edge {
                from_side: Some(side),
                ..Edge::new(edge, from.as_str(), node.id.as_str())
            };
            Some(vec![
                Change::AddNode { node, index: None },
                Change::AddEdge { edge, index: None },
            ])
        };
        self.added(changes, window, cx);
    }

    /// The topmost node at a canvas point, other than `except`; a node before
    /// the group it sits in.
    fn node_under(&self, at: (i64, i64), except: &str) -> Option<&str> {
        self.canvas
            .nodes
            .iter()
            .rev()
            .filter(|n| {
                n.id != except
                    && (n.x..n.x + n.width).contains(&at.0)
                    && (n.y..n.y + n.height).contains(&at.1)
            })
            .min_by_key(|n| n.kind == model::GROUP)
            .map(|n| n.id.as_str())
    }

    /// The node a connector being drawn would reach.
    fn connect_target(&self) -> Option<&str> {
        let Some(Grab::Connect { from, to, .. }) = &self.grab else {
            return None;
        };
        self.node_under(self.to_canvas(*to), from)
    }

    /// The topmost edge passing near a window position.
    fn edge_at(&self, position: Point<Pixels>) -> Option<String> {
        let local = self.local(position);
        let at = point(
            (local.x - self.pan.x) / self.zoom,
            (local.y - self.pan.y) / self.zoom,
        );
        let reach = EDGE_REACH / self.zoom;
        self.canvas
            .edges
            .iter()
            .rev()
            .find(|edge| {
                curve(&self.canvas, &self.shown, edge).is_some_and(|c| c.distance(at) <= reach)
            })
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
        if event.modifiers.shift || event.modifiers.platform {
            let mut ids = self.selected.clone();
            match ids.iter().position(|s| *s == id) {
                Some(at) => drop(ids.remove(at)),
                None => ids.push(id),
            }
            self.set_selection(ids, cx);
            return;
        }
        if event.click_count >= 2 {
            self.select(Some(id.clone()), cx);
            let node = self.canvas.node(&id).cloned();
            match node.and_then(|node| Some((kind::kind(cx, &node.kind).open?, node))) {
                Some((open, node)) => open(&node, cx),
                None => self.edit(id, None, window, cx),
            }
            return;
        }
        // Pressing one of a selection keeps the rest, to drag them together.
        let mut with: Vec<String> = self
            .selected
            .iter()
            .filter(|s| **s != id)
            .cloned()
            .collect();
        if with.len() == self.selected.len() {
            with.clear();
        }
        self.set_selection(with.iter().cloned().chain([id.clone()]).collect(), cx);
        if let Some(node) = self.canvas.node(&id) {
            self.grab = Some(Grab::Node {
                origin: (node.x, node.y),
                id,
                with,
                from: event.position,
                moved: false,
                before: HashMap::new(),
            });
            cx.notify();
        }
    }

    fn drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            self.release(event.position, window, cx);
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
            Some(Grab::Marquee { to, .. }) => {
                *to = event.position;
                self.mark(cx);
                cx.notify();
            }
            Some(Grab::Connect { to, .. }) => {
                *to = event.position;
                cx.notify();
            }
            Some(Grab::Resize {
                id,
                start,
                size,
                grows,
            }) => {
                let z = self.zoom;
                let pulled = |from: Pixels, to: Pixels| ((to - from).as_f32() / z).round() as i64;
                let width = (size.0 + pulled(start.x, event.position.x)).max(MIN_SIZE.0);
                let height = if *grows {
                    self.canvas.node(id).map_or(size.1, |node| node.height)
                } else {
                    (size.1 + pulled(start.y, event.position.y)).max(MIN_SIZE.1)
                };
                let change = Change::Resize {
                    id: id.clone(),
                    size: (width, height),
                };
                change::apply(&mut self.canvas, &change);
                self.stale = true;
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
                    let Change::MoveNodes { moves } = &change else {
                        pending.push(change);
                        continue;
                    };
                    if let Some(Grab::Node { before, .. }) = &mut self.grab {
                        for (id, _) in moves {
                            if let Some(node) = self.canvas.node(id) {
                                before.entry(id.clone()).or_insert((node.x, node.y));
                            }
                        }
                    }
                    change::apply(&mut self.canvas, &change);
                }
                self.pending = pending;
                self.stale = true;
                cx.notify();
            }
        }
    }

    /// The button came up: what the preview moved is put back, and the drop is
    /// submitted.
    fn release(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.pending.clear();
        match &mut self.grab {
            Some(Grab::Node {
                moved: true,
                before,
                ..
            }) => {
                let back = Change::MoveNodes {
                    moves: before.drain().collect(),
                };
                change::apply(&mut self.canvas, &back);
                let drop = self.gesture(position, Phase::Drop);
                self.grab = None;
                self.submit(drop, cx);
                self.stale = true;
                cx.notify();
            }
            // A click on one of a selection, without a drag, selects just it.
            Some(Grab::Node { id, .. }) => {
                let id = id.clone();
                self.grab = None;
                self.select(Some(id), cx);
            }
            Some(Grab::Marquee { .. }) => cx.notify(),
            Some(Grab::Connect {
                from,
                side,
                start,
                to,
            }) => {
                let pulled = (to.x - start.x)
                    .as_f32()
                    .abs()
                    .max((to.y - start.y).as_f32().abs());
                let (from, side, to) = (from.clone(), *side, *to);
                self.grab = None;
                if pulled >= DRAG_SLOP {
                    self.connect(from, side, to, window, cx);
                }
                cx.notify();
            }
            // The preview is put back, and the pull submitted as one resize.
            Some(Grab::Resize { id, size, .. }) => {
                let (id, before) = (id.clone(), *size);
                self.grab = None;
                if let Some(node) = self.canvas.node(&id) {
                    let now = (node.width, node.height);
                    let back = Change::Resize {
                        id: id.clone(),
                        size: before,
                    };
                    change::apply(&mut self.canvas, &back);
                    if now != before {
                        self.submit([Change::Resize { id, size: now }], cx);
                    }
                }
                self.stale = true;
                cx.notify();
            }
            Some(Grab::Pan(_)) | None => {}
        }
        self.grab = None;
    }

    /// Select what the marquee touches, beside what was selected before it.
    fn mark(&mut self, cx: &mut Context<Self>) {
        let Some(Grab::Marquee { from, to, base }) = &self.grab else {
            return;
        };
        let (a, b) = (self.to_canvas(*from), self.to_canvas(*to));
        let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
        let touched = self.canvas.nodes.iter().filter(|n| {
            n.x < x1
                && n.x + n.width > x0
                && n.y < y1
                && n.y + n.height > y0
                && !base.contains(&n.id)
        });
        let ids = base.iter().chain(touched.map(|n| &n.id)).cloned().collect();
        self.set_selection(ids, cx);
    }

    /// The drag handler's answer to the held node at `position`.
    fn gesture(&self, position: Point<Pixels>, phase: Phase) -> Vec<Change> {
        let Some(Grab::Node {
            id,
            with,
            from,
            origin,
            ..
        }) = &self.grab
        else {
            return Vec::new();
        };
        let zoom = self.zoom;
        let delta = (
            ((position.x - from.x).as_f32() / zoom).round() as i64,
            ((position.y - from.y).as_f32() / zoom).round() as i64,
        );
        let pointer = self.to_canvas(position);
        let held: Vec<String> = std::iter::once(id.clone())
            .chain(with.iter().cloned())
            .collect();
        let gesture = Drag {
            id,
            with,
            origin: *origin,
            delta,
            over: mindmap::node_at(&self.canvas, pointer, &held),
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

    /// The view's own batch, past the filter and announced: last frame's
    /// measurements, and the layout if anything moved.
    fn reflow(&mut self, cx: &mut Context<Self>) {
        let mut measured: Vec<_> = self.measured.borrow_mut().drain().collect();
        measured.sort();
        let mut changes = Vec::new();
        for (id, height) in measured {
            if let Some(node) = self.canvas.node(&id)
                && node.height != height
            {
                changes.push(Change::Resize {
                    size: (node.width, height),
                    id,
                });
                change::apply(&mut self.canvas, changes.last().expect("just pushed"));
                self.stale = true;
            }
        }
        if std::mem::take(&mut self.stale) {
            let held = self.held().map(str::to_owned);
            let moves = (self.layout.arrange)(&self.canvas, held.as_deref());
            if !moves.is_empty() {
                changes.push(Change::MoveNodes { moves });
                change::apply(&mut self.canvas, changes.last().expect("just pushed"));
            }
        }
        if !changes.is_empty() {
            cx.emit(CanvasEvent::Changed(changes));
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
        connecting: Option<&str>,
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
        let editing = self.editing.as_ref().filter(|s| !s.edge && s.id == node.id);
        // A node being typed in keeps the editor's own cursor.
        let draggable = editing.is_none();
        let content = match editing {
            Some(session) => session.editor.clone().into_any_element(),
            None => (kind.render)(node, z, window, cx),
        };
        let grows = kind.sizing == Sizing::Grows;
        let selected = self.selected.contains(&node.id);
        // A drop, or a connector let go here, would connect to this node.
        let target = connecting == Some(node.id.as_str())
            || self.held() != Some(node.id.as_str())
                && self.pending.iter().any(|change| {
                    matches!(change, Change::AddEdge { edge, .. }
                        if edge.from_node == node.id || edge.to_node == node.id)
                });
        // A node picked alone, with nothing held, shows its handles.
        let handles = selected && draggable && self.selected.len() == 1 && self.grab.is_none();
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
                Chrome::Frame => d.border_1().border_color(border).bg(node
                    .color
                    .as_deref()
                    .and_then(|c| color(theme, c))
                    .map_or(gpui::transparent_black(), |c| c.opacity(FRAME_WASH))),
            })
            .when(draggable, |d| d.cursor_grab())
            // A box of fixed size keeps whatever a renderer paints inside it;
            // a frame's label sits outside it.
            .child(if grows || kind.chrome == Chrome::Frame {
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
                        .cursor(if grows {
                            CursorStyle::ResizeLeftRight
                        } else {
                            CursorStyle::ResizeUpLeftDownRight
                        })
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
        let z = self.zoom;
        self.canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let editing = self.editing.as_ref().filter(|s| s.edge && s.id == edge.id);
                let text = edge.label.as_deref().filter(|label| !label.is_empty());
                if editing.is_none() && text.is_none() {
                    return None;
                }
                let middle = curve(&self.canvas, shown, edge)?.middle();
                let (x, y) = (self.pan.x + middle.x * z, self.pan.y + middle.y * z);
                let body = match editing {
                    Some(session) => div()
                        .min_w(px(LABEL.0 / 2.0 * z))
                        .child(session.editor.clone())
                        .into_any_element(),
                    None => kind::text_style(div(), TextStyle::Callout, z)
                        .text_color(theme.text_muted)
                        .child(text.unwrap_or_default().to_owned())
                        .into_any_element(),
                };
                let picked = self.edge.as_deref() == Some(edge.id.as_str());
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
        let (z, pan) = (self.zoom, self.pan);
        let grabbing = self.grab.is_some();
        let holding = matches!(self.grab, Some(Grab::Node { .. }));
        // Connectors a drop would cut, and the ones it would make.
        let cut: HashSet<&str> = self
            .pending
            .iter()
            .flat_map(|change| match change {
                Change::RemoveEdges { ids } => ids.as_slice(),
                _ => &[],
            })
            .map(String::as_str)
            .collect();
        // Each curve, its colour and its weight.
        let mut strokes: Vec<(Curve, Hsla, f32)> = self
            .canvas
            .edges
            .iter()
            .filter_map(|edge| {
                let curve = curve(&self.canvas, shown, edge)?;
                if self.edge.as_deref() == Some(edge.id.as_str()) {
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
        strokes.extend(self.pending.iter().filter_map(|change| {
            let Change::AddEdge { edge, .. } = change else {
                return None;
            };
            Some((curve(&self.canvas, shown, edge)?, theme.accent, 1.0))
        }));
        // The connector being drawn, out to the pointer.
        if let Some(Grab::Connect { from, side, to, .. }) = &self.grab
            && let Some(node) = self.canvas.node(from)
        {
            let (start, out) = anchor(Rect::of(node, shown), *side);
            let local = self.local(*to);
            let end = point((local.x - pan.x) / z, (local.y - pan.y) / z);
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
        self.reflow(cx);
        self.frame();
        let shown = self.positions(cx.reduced_motion(), window);
        let theme = Theme::of(cx).clone();
        let edges = self.edge_layer(&theme, &shown, cx.entity().downgrade());
        // Groups paint first, under what they frame. The held node paints
        // last, over whatever it is carried across, unless it frames them.
        let group = |ix: usize| self.canvas.nodes[ix].kind == model::GROUP;
        let held = self
            .held()
            .and_then(|id| self.canvas.index_of(id))
            .filter(|ix| !group(*ix));
        let (groups, rest): (Vec<usize>, Vec<usize>) = (0..self.canvas.nodes.len())
            .filter(|ix| Some(*ix) != held)
            .partition(|ix| group(*ix));
        let order = groups.into_iter().chain(rest).chain(held);
        let connecting = self.connect_target().map(str::to_owned);
        let nodes: Vec<AnyElement> = order
            .filter_map(|ix| {
                let at = shown[&self.canvas.nodes[ix].id];
                self.paint_node(ix, at, connecting.as_deref(), &theme, window, cx)
            })
            .collect();
        let labels = self.labels(&theme, &shown, cx);
        let marquee = match &self.grab {
            Some(Grab::Marquee { from, to, .. }) => {
                let (a, b) = (self.local(*from), self.local(*to));
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
            .on_action(
                cx.listener(|this, _: &keys::AddChild, window, cx| this.add_child(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &keys::AddSibling, window, cx| this.add_sibling(window, cx)),
            )
            .on_action(cx.listener(|this, _: &keys::Remove, _, cx| this.remove_selected(cx)))
            .on_action(cx.listener(|this, _: &keys::Edit, window, cx| {
                if let Some(id) = this.edge.clone() {
                    this.edit_edge(id, window, cx);
                } else if let Some(id) = this.selected().map(str::to_owned) {
                    this.edit(id, None, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &keys::SelectAll, _, cx| this.select_all(cx)))
            .on_action(cx.listener(|this, _: &keys::Deselect, _, cx| {
                this.select(None, cx);
                this.select_edge(None, cx);
            }))
            .on_action(cx.listener(|this, _: &keys::Undo, _, cx| {
                this.undo(cx);
            }))
            .on_action(cx.listener(|this, _: &keys::Redo, _, cx| {
                this.redo(cx);
            }))
            .on_action(cx.listener(|this, _: &keys::Copy, _, cx| this.copy(cx)))
            .on_action(cx.listener(|this, _: &keys::Cut, _, cx| this.cut(cx)))
            .on_action(cx.listener(|this, _: &keys::Paste, _, cx| this.paste(cx)))
            .on_action(cx.listener(|this, _: &keys::Duplicate, _, cx| this.duplicate(cx)))
            .on_action(
                cx.listener(|this, _: &keys::StopEditing, window, cx| {
                    this.stop_editing(window, cx)
                }),
            )
            .on_action(cx.listener(|this, _: &keys::SelectLeft, _, cx| this.arrow(Arrow::Left, cx)))
            .on_action(
                cx.listener(|this, _: &keys::SelectRight, _, cx| this.arrow(Arrow::Right, cx)),
            )
            .on_action(cx.listener(|this, _: &keys::SelectUp, _, cx| this.arrow(Arrow::Up, cx)))
            .on_action(cx.listener(|this, _: &keys::SelectDown, _, cx| this.arrow(Arrow::Down, cx)))
            .on_action(cx.listener(|this, _: &keys::NudgeLeft, _, cx| this.nudge(Arrow::Left, cx)))
            .on_action(
                cx.listener(|this, _: &keys::NudgeRight, _, cx| this.nudge(Arrow::Right, cx)),
            )
            .on_action(cx.listener(|this, _: &keys::NudgeUp, _, cx| this.nudge(Arrow::Up, cx)))
            .on_action(cx.listener(|this, _: &keys::NudgeDown, _, cx| this.nudge(Arrow::Down, cx)))
            .on_action(cx.listener(|this, _: &keys::ZoomIn, _, cx| this.zoom_in(cx)))
            .on_action(cx.listener(|this, _: &keys::ZoomOut, _, cx| this.zoom_out(cx)))
            .on_action(cx.listener(|this, _: &keys::ResetZoom, _, cx| this.set_zoom(1.0, cx)))
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

/// A JSON Canvas color: a preset, or hex. Yellow and cyan have no token, so
/// they turn the hue of the token beside them.
fn color(theme: &Theme, color: &str) -> Option<Hsla> {
    match color {
        "1" => Some(theme.danger),
        "2" => Some(theme.warning),
        "3" => Some(Hsla {
            h: 50.0 / 360.0,
            ..theme.warning
        }),
        "4" => Some(theme.success),
        "5" => Some(Hsla {
            h: 185.0 / 360.0,
            ..theme.success
        }),
        "6" => Some(theme.accent),
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

fn curve(canvas: &Canvas, shown: &Positions, edge: &Edge) -> Option<Curve> {
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
