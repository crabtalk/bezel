//! The canvas without a window: the document, its kinds and layout, the
//! selection, history and the part in view, behind commands. [`CanvasView`]
//! turns keys and the pointer into them and paints what they leave.
//!
//! ```ignore
//! let mut editor = CanvasEditor::new(Canvas::parse(json)?, layout::FREE);
//! editor.select(Some("a".into()));
//! editor.remove_selected();
//! assert!(editor.undo());
//! ```
//!
//! [`CanvasView`]: crate::CanvasView

use std::{cell::OnceCell, rc::Rc};

use gpui::{Point, Size, point};

use crate::{
    change::{self, Change},
    clip, contain,
    drag::DragHandler,
    edge::EdgeKinds,
    handle::Handle,
    kind::{Capability, Kinds, PAD},
    layout::{Arrow, Layout},
    mindmap,
    model::{Canvas, Node, Side},
    options::Options,
    snap::{Guide, Snap},
};

// A change is handed on and dropped, never kept in bulk; boxing its node would
// only put a `Box::new` in every filter.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    /// A batch landed in the document.
    Changed(Vec<Change>),
    /// The selected nodes, the primary last.
    Selected(Vec<String>),
    /// The selected edge.
    EdgeSelected(Option<String>),
}

/// Something selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Node(String),
    Edge(String),
}

/// Decides each change before it lands: it, another, or `None` to refuse.
type Filter = Rc<dyn Fn(&Canvas, Change) -> Option<Change>>;

/// One undoable edit: the changes that take it back, and the typing session it
/// belongs to, which later typing joins.
struct Step {
    changes: Vec<Change>,
    group: Option<u64>,
}

pub struct CanvasEditor {
    canvas: Canvas,
    kinds: Kinds,
    edge_kinds: EdgeKinds,
    layout: Layout,
    /// What an app said dragging does, in place of the layout's.
    drag: Option<DragHandler>,
    snap: Snap,
    options: Options,
    filter: Option<Filter>,
    /// Nodes, or one edge; the primary is last.
    selection: Vec<Item>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The typing session edits now join, and the last one handed out.
    pub(crate) group: Option<u64>,
    groups: u64,
    /// Where the canvas origin sits, in pixels from the view's top left.
    pan: Point<f32>,
    zoom: f32,
    viewport: Option<Size<f32>>,
    /// The viewport the document was last centred in.
    framed: Option<Size<f32>>,
    /// Panned or zoomed, so a new viewport no longer re-centres.
    touched: bool,
    /// A node to bring into view once layout has placed it.
    reveal: Option<String>,
    stale: bool,
    /// The document as a gesture in hand would leave it.
    preview: Option<Canvas>,
    preview_stale: bool,
    containment: OnceCell<contain::Index>,
    preview_containment: OnceCell<contain::Index>,
    /// What a drop would do beyond its moves, drawn while a node is held.
    pub(crate) pending: Vec<Change>,
    /// The lines a drag caught on.
    pub(crate) guides: Vec<Guide>,
    events: Vec<CanvasEvent>,
    /// History or a new document replaced what an open editor holds.
    rewound: bool,
}

impl CanvasEditor {
    /// `layout` places the nodes and says what its edits mean; the kinds are
    /// the spec's until [`Self::with_kinds`].
    pub fn new(canvas: Canvas, layout: Layout) -> Self {
        Self {
            canvas,
            kinds: Kinds::new(),
            edge_kinds: EdgeKinds::new(),
            layout,
            drag: None,
            snap: Snap::default(),
            options: Options::default(),
            filter: None,
            selection: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
            groups: 0,
            pan: point(0.0, 0.0),
            zoom: 1.0,
            viewport: None,
            framed: None,
            touched: false,
            reveal: None,
            stale: true,
            preview: None,
            preview_stale: false,
            containment: OnceCell::new(),
            preview_containment: OnceCell::new(),
            pending: Vec::new(),
            guides: Vec::new(),
            events: Vec::new(),
            rewound: false,
        }
    }

    pub fn with_kinds(mut self, kinds: Kinds) -> Self {
        self.set_kinds(kinds);
        self
    }

    pub fn with_edge_kinds(mut self, kinds: EdgeKinds) -> Self {
        self.set_edge_kinds(kinds);
        self
    }

    /// What dragging a node does, in place of the layout's own.
    pub fn with_drag(mut self, handler: DragHandler) -> Self {
        self.drag = Some(handler);
        self
    }

    /// How dragged and resized boxes settle.
    pub fn with_snap(mut self, snap: Snap) -> Self {
        self.snap = snap;
        self
    }

    /// What the canvas is tuned by. The tools read it too.
    pub fn with_options(mut self, options: Options) -> Self {
        self.options = options;
        self
    }

    pub fn options(&self) -> Options {
        self.options
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Sees every change about to land through [`Self::submit`], and answers
    /// what lands: the change, another, or `None`.
    pub fn with_changes(
        mut self,
        filter: impl Fn(&Canvas, Change) -> Option<Change> + 'static,
    ) -> Self {
        self.filter = Some(Rc::new(filter));
        self
    }

    /// The document as a save would write it.
    pub fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// The document with the gesture in hand over it: what paints.
    pub fn painted(&self) -> &Canvas {
        self.preview.as_ref().unwrap_or(&self.canvas)
    }

    pub fn kinds(&self) -> &Kinds {
        &self.kinds
    }

    pub fn set_kinds(&mut self, kinds: Kinds) {
        self.kinds = kinds;
        self.containment.take();
        self.preview_containment.take();
        self.stale = true;
    }

    pub fn edge_kinds(&self) -> &EdgeKinds {
        &self.edge_kinds
    }

    pub fn set_edge_kinds(&mut self, kinds: EdgeKinds) {
        self.edge_kinds = kinds;
    }

    /// A new document, with no history.
    pub fn set_canvas(&mut self, canvas: Canvas) {
        self.canvas = canvas;
        self.containment.take();
        (self.undo, self.redo, self.group) = (Vec::new(), Vec::new(), None);
        self.clear_preview();
        self.rewound = true;
        self.prune();
        self.stale = true;
    }

    /// Land a batch as if the reader made it: each change through the filter
    /// first, against the document before the batch. `false` when one was
    /// refused, and then none lands.
    pub fn submit(&mut self, changes: impl IntoIterator<Item = Change>) -> bool {
        let changes: Vec<Change> = match &self.filter {
            Some(filter) => match changes
                .into_iter()
                .map(|change| filter(&self.canvas, change))
                .collect()
            {
                Some(changes) => changes,
                None => return false,
            },
            None => changes.into_iter().collect(),
        };
        self.apply(changes);
        true
    }

    /// Land a batch of the app's own, past the filter. It can be undone.
    pub fn apply(&mut self, changes: impl IntoIterator<Item = Change>) {
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
                    if self.undo.len() > self.options.history {
                        self.undo.remove(0);
                    }
                }
            }
        }
        self.landed(changes);
    }

    /// What `cmd-z` does: take back the last edit, past the filter. `false`
    /// when there is none.
    pub fn undo(&mut self) -> bool {
        let Some(step) = self.undo.pop() else {
            return false;
        };
        let redo = change::apply_all(&mut self.canvas, &step.changes);
        self.redo.push(Step {
            changes: redo,
            group: None,
        });
        self.rewind(step.changes);
        true
    }

    /// What `cmd-shift-z` does.
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.redo.pop() else {
            return false;
        };
        let undo = change::apply_all(&mut self.canvas, &step.changes);
        self.undo.push(Step {
            changes: undo,
            group: None,
        });
        self.rewind(step.changes);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Typing after a step taken back starts a step of its own.
    fn rewind(&mut self, changes: Vec<Change>) {
        self.group = None;
        self.rewound = true;
        self.landed(changes);
    }

    fn landed(&mut self, changes: Vec<Change>) {
        self.containment.take();
        self.prune();
        self.stale = true;
        self.events.push(CanvasEvent::Changed(changes));
    }

    pub fn selection(&self) -> &[Item] {
        &self.selection
    }

    /// The selected nodes, the primary last.
    pub fn selected_nodes(&self) -> Vec<&str> {
        self.selection
            .iter()
            .filter_map(|item| match item {
                Item::Node(id) => Some(id.as_str()),
                Item::Edge(_) => None,
            })
            .collect()
    }

    /// The primary selection: the last node chosen, which the keys act from.
    pub fn selected(&self) -> Option<&str> {
        self.selection.iter().rev().find_map(|item| match item {
            Item::Node(id) => Some(id.as_str()),
            Item::Edge(_) => None,
        })
    }

    pub fn selected_edge(&self) -> Option<&str> {
        self.selection.iter().find_map(|item| match item {
            Item::Edge(id) => Some(id.as_str()),
            Item::Node(_) => None,
        })
    }

    /// Whether `item` lets the reader do `what`. Every command honours this,
    /// so a key, a toolbar and a gesture all stop at the same place.
    pub fn can(&self, item: &Item, what: Capability) -> bool {
        match item {
            Item::Node(id) => self.node_can(id, what),
            Item::Edge(id) => self.edge_can(id, what),
        }
    }

    pub fn node_can(&self, id: &str, what: Capability) -> bool {
        self.canvas
            .node(id)
            .is_some_and(|node| self.kinds.allows(node, what))
    }

    pub fn edge_can(&self, id: &str, what: Capability) -> bool {
        self.canvas
            .edge(id)
            .is_some_and(|edge| self.edge_kinds.allows(edge, what))
    }

    /// Select only `id`, or nothing.
    pub fn select(&mut self, id: Option<String>) {
        self.set_selection(id.into_iter().collect());
    }

    /// Select these nodes, the primary last, those that may be selected. Any
    /// lets the edge go.
    pub fn set_selection(&mut self, ids: Vec<String>) {
        let ids: Vec<String> = ids
            .into_iter()
            .filter(|id| self.node_can(id, Capability::Selectable))
            .collect();
        if !ids.is_empty() {
            self.select_edge(None);
        }
        if self.selected_nodes() != ids {
            self.selection.retain(|item| matches!(item, Item::Edge(_)));
            self.selection.extend(ids.iter().cloned().map(Item::Node));
            self.events.push(CanvasEvent::Selected(ids));
        }
    }

    /// Select an edge, or none. Selecting one lets the nodes go.
    pub fn select_edge(&mut self, id: Option<String>) {
        let id = id.filter(|id| self.edge_can(id, Capability::Selectable));
        if id.is_some() {
            self.set_selection(Vec::new());
        }
        if self.selected_edge() != id.as_deref() {
            self.selection.retain(|item| matches!(item, Item::Node(_)));
            self.selection.extend(id.clone().map(Item::Edge));
            self.events.push(CanvasEvent::EdgeSelected(id));
        }
    }

    /// What `cmd-a` does.
    pub fn select_all(&mut self) {
        let ids = self.canvas.nodes.iter().map(|n| n.id.clone()).collect();
        self.set_selection(ids);
    }

    /// Let go of what the document no longer holds.
    fn prune(&mut self) {
        let kept = self
            .selected_nodes()
            .into_iter()
            .filter(|id| self.canvas.node(id).is_some())
            .map(str::to_owned)
            .collect();
        self.set_selection(kept);
        if self
            .selected_edge()
            .is_some_and(|id| self.canvas.edge(id).is_none())
        {
            self.select_edge(None);
        }
    }

    fn nodes(&self) -> Vec<String> {
        self.selected_nodes()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    /// What `cmd-c` copies: the selection, with its branches under a tree, as
    /// JSON Canvas.
    pub fn copy(&self) -> Option<String> {
        if self.selection.iter().all(|i| matches!(i, Item::Edge(_))) {
            return None;
        }
        Some(clip::fragment(&self.canvas, &self.contents(&self.reach())).to_json())
    }

    /// What `cmd-x` does: the copy, and the selection gone.
    pub fn cut(&mut self) -> Option<String> {
        let copied = self.copy();
        self.remove_selected();
        copied
    }

    /// What `cmd-v` does with `text`: a copied canvas, or a node holding it —
    /// under the selection in a tree, else in the middle of the view.
    pub fn paste(&mut self, text: &str) {
        let fragment = Canvas::parse(text).unwrap_or_else(|_| Canvas {
            nodes: vec![self.kinds.fresh(Some(text.to_owned()))],
            ..Canvas::default()
        });
        let under = self
            .selected()
            .and_then(|id| (self.layout.paste_under)(&self.canvas, id));
        let at = match under.as_deref().and_then(|id| self.canvas.node(id)) {
            Some(parent) => (parent.x + parent.width + mindmap::GAP_X, parent.y),
            None => {
                let (x, y) = self.center();
                let (_, (w, h)) = clip::bounds(&fragment).unwrap_or_default();
                (x - w / 2, y - h / 2)
            }
        };
        self.place(&fragment, at, under.as_deref());
    }

    /// What `cmd-d` does: the selection copied beside itself, or under the same
    /// parent in a tree.
    pub fn duplicate(&mut self) {
        let Some(primary) = self.selected().map(str::to_owned) else {
            return;
        };
        let fragment = clip::fragment(&self.canvas, &self.contents(&self.reach()));
        let Some(((x, y), _)) = clip::bounds(&fragment) else {
            return;
        };
        let under = (self.layout.duplicate_under)(&self.canvas, &primary);
        self.place(
            &fragment,
            (
                x + self.options.duplicate_offset,
                y + self.options.duplicate_offset,
            ),
            under.as_deref(),
        );
    }

    /// A fragment added through the filter, and selected.
    fn place(&mut self, fragment: &Canvas, at: (i64, i64), under: Option<&str>) {
        let changes = clip::paste(&self.canvas, fragment, at, under);
        let added: Vec<String> = changes
            .iter()
            .filter_map(|change| match change {
                Change::AddNode { node, .. } => Some(node.id.clone()),
                _ => None,
            })
            .collect();
        if self.submit(changes) {
            self.reveal = added.last().cloned();
            self.set_selection(added);
        }
    }

    /// The selection, each with its branch under a layout that grows trees.
    fn reach(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for id in self.selected_nodes() {
            for id in (self.layout.reach)(&self.canvas, id) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    /// `ids`, and everything they hold.
    pub(crate) fn contents(&self, ids: &[String]) -> Vec<String> {
        self.containment().with_contents(ids)
    }

    fn containment(&self) -> &contain::Index {
        self.containment
            .get_or_init(|| contain::Index::new(&self.canvas, |node| self.kinds.holds(node)))
    }

    pub(crate) fn painted_containment(&self) -> &contain::Index {
        match &self.preview {
            Some(preview) => self
                .preview_containment
                .get_or_init(|| contain::Index::new(preview, |node| self.kinds.holds(node))),
            None => self.containment(),
        }
    }

    /// What `backspace` does, through the filter: the selected edge, or the
    /// selection, each node with its branch under a layout that grows trees.
    pub fn remove_selected(&mut self) {
        if let Some(id) = self.selected_edge().map(str::to_owned) {
            if self.edge_can(&id, Capability::Deletable) {
                self.submit([Change::RemoveEdges { ids: vec![id] }]);
            }
            return;
        }
        let Some(primary) = self.selected().map(str::to_owned) else {
            return;
        };
        // What will not be deleted stays, and the rest goes.
        let ids: Vec<String> = self
            .reach()
            .into_iter()
            .filter(|id| self.node_can(id, Capability::Deletable))
            .collect();
        if ids.is_empty() {
            return;
        }
        let next = mindmap::after_removal(&self.canvas, &primary);
        if self.submit([Change::RemoveNodes { ids }]) {
            self.select(next.filter(|next| self.canvas.node(next).is_some()));
        }
    }

    /// What an arrow does, the way the layout walks.
    pub fn select_toward(&mut self, arrow: Arrow) {
        let next = match self.selected() {
            Some(id) => (self.layout.walk)(&self.canvas, id, arrow),
            None => self.first_root(),
        };
        if next.is_some() {
            self.reveal = next.clone();
            self.select(next);
        }
    }

    /// What a `shift`-arrow does: move what may be dragged, pinned under a
    /// tree. What a container holds goes with it either way.
    pub fn nudge(&mut self, arrow: Arrow) {
        let ids: Vec<String> = self
            .nodes()
            .into_iter()
            .filter(|id| self.node_can(id, Capability::Draggable))
            .collect();
        if ids.is_empty() {
            return;
        }
        let (dx, dy) = arrow.unit();
        let step = self.snap.grid.unwrap_or(self.options.nudge);
        let ids = self.contents(&ids);
        let pin = self.layout.pins;
        let changes = mindmap::carry(&self.canvas, &ids, (dx * step, dy * step), pin);
        self.submit(changes);
    }

    /// What `tab` does: a child of the primary, or of the first root, or a
    /// root of its own. Answers what was added and selected.
    pub fn add_child(&mut self) -> Option<String> {
        let parent = self
            .selected()
            .map(str::to_owned)
            .or_else(|| self.first_root());
        let changes = match parent {
            Some(parent) => self
                .template(&parent)
                .and_then(|node| mindmap::child(&self.canvas, &parent, node)),
            None => {
                let node = self.kinds.fresh(None);
                Some(vec![mindmap::root(&self.canvas, node, (0, 0))])
            }
        };
        self.added(changes)
    }

    /// What `enter` does. A root has no siblings, so it adds a child.
    pub fn add_sibling(&mut self) -> Option<String> {
        let of = self.selected()?.to_owned();
        let changes = match mindmap::parent(&self.canvas, &of).map(str::to_owned) {
            Some(parent) => self
                .template(&parent)
                .and_then(|node| mindmap::sibling(&self.canvas, &of, node)),
            None => self
                .template(&of)
                .and_then(|node| mindmap::child(&self.canvas, &of, node)),
        };
        self.added(changes)
    }

    /// What a double-click on nothing does: a node made from nothing,
    /// centred on `at`.
    pub fn add_root(&mut self, at: (i64, i64)) -> Option<String> {
        let node = self.kinds.fresh(None);
        let at = (at.0 - node.width / 2, at.1 - node.height / 2);
        let root = mindmap::root(&self.canvas, node, at);
        self.added(Some(vec![root]))
    }

    /// A connector out of `from`'s `side` let go at `at`, as the layout reads
    /// it: onto the node there, else a node made there. Answers what it added.
    pub fn connect(&mut self, from: &str, handle: &Handle, at: (i64, i64)) -> Option<String> {
        // A connector leaves a side; a handle that sits on none draws nothing.
        let side = handle.side()?;
        if !self.node_can(from, Capability::Connectable) {
            return None;
        }
        if let Some(to) = self.node_under(at, from).map(str::to_owned) {
            if self.node_can(&to, Capability::Connectable) {
                let changes = (self.layout.link)(&self.canvas, from, side, &to);
                self.submit(named(changes, &handle.id, side));
            }
            return None;
        }
        let node = self.template(from)?;
        let changes = (self.layout.extend)(&self.canvas, from, side, at, node);
        self.added(changes.map(|changes| named(changes, &handle.id, side)))
    }

    /// A batch that adds a node, as one undo step through the filter; the
    /// node selected and brought into view.
    fn added(&mut self, changes: Option<Vec<Change>>) -> Option<String> {
        let changes = changes?;
        let id = change::added(&changes)?.to_owned();
        let group = self.next_group();
        self.group = Some(group);
        let landed = self.submit(changes);
        self.group = None;
        if !landed || self.canvas.node(&id).is_none() {
            return None;
        }
        self.select(Some(id.clone()));
        self.reveal = Some(id.clone());
        Some(id)
    }

    /// The typing session of the last step, which typing into what it added
    /// joins.
    pub(crate) fn last_group(&self) -> Option<u64> {
        self.undo.last().and_then(|step| step.group)
    }

    pub(crate) fn next_group(&mut self) -> u64 {
        self.groups += 1;
        self.groups
    }

    /// The first node no branch points at, which is what `tab` and an arrow
    /// reach for when nothing is selected.
    fn first_root(&self) -> Option<String> {
        mindmap::roots(&self.canvas, |node| self.kinds.holds(node))
            .next()
            .map(|root| root.id.clone())
    }

    /// What `tab` makes under `parent`, from its kind.
    fn template(&self, parent: &str) -> Option<Node> {
        let parent = self.canvas.node(parent)?;
        Some((self.kinds.get(&parent.kind).rules.child)(parent))
    }

    /// The topmost node at a canvas point, other than `except`: what a
    /// container holds before the container.
    pub(crate) fn node_under(&self, at: (i64, i64), except: &str) -> Option<&str> {
        self.painted_containment()
            .topmost(self.painted(), at, &[except.to_owned()], |_| true)
    }

    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Where the canvas origin sits, in pixels from the view's top left.
    pub fn pan(&self) -> Point<f32> {
        self.pan
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.pan.x += dx;
        self.pan.y += dy;
        self.touched = true;
    }

    /// Zoom about the middle of the view.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom_about(zoom, self.middle());
    }

    /// What `cmd-=` does.
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom * self.options.zoom_step);
    }

    /// What `cmd--` does.
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom / self.options.zoom_step);
    }

    /// Zoom keeping the canvas point under `anchor`, in pixels from the view's
    /// top left, where it is.
    pub fn zoom_about(&mut self, zoom: f32, anchor: Point<f32>) {
        let zoom = zoom.clamp(self.options.min_zoom, self.options.max_zoom);
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
    }

    /// The view's size in pixels, which the view sets as it paints.
    pub fn viewport(&self) -> Option<Size<f32>> {
        self.viewport
    }

    pub fn set_viewport(&mut self, size: Size<f32>) {
        self.viewport = Some(size);
    }

    /// What `shift-1` does: the whole document in view, no closer than 100%.
    pub fn fit(&mut self) {
        self.show(extent(&self.canvas.nodes), 1.0);
    }

    /// What `shift-2` does: the selection in view.
    pub fn zoom_to_selection(&mut self) {
        let nodes = self
            .selected_nodes()
            .into_iter()
            .filter_map(|id| self.canvas.node(id));
        self.show(extent(nodes), self.options.selection_zoom);
    }

    /// The part of the canvas in view, in canvas units: left, top, width,
    /// height.
    pub fn visible(&self) -> Option<(f32, f32, f32, f32)> {
        let viewport = self.viewport?;
        let z = self.zoom;
        Some((
            -self.pan.x / z,
            -self.pan.y / z,
            viewport.width / z,
            viewport.height / z,
        ))
    }

    /// Pan so canvas point `at` sits in the middle of the view.
    pub fn center_on(&mut self, at: (f32, f32)) {
        let middle = self.middle();
        self.pan = point(middle.x - at.0 * self.zoom, middle.y - at.1 * self.zoom);
        self.touched = true;
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

    /// Zoom and pan so the box `(left, top, right, bottom)` fills the view, no
    /// closer than `most`.
    fn show(&mut self, extent: Option<(i64, i64, i64, i64)>, most: f32) {
        let (Some((x0, y0, x1, y1)), Some(viewport)) = (extent, self.viewport) else {
            return;
        };
        let (vw, vh) = (viewport.width, viewport.height);
        let (bw, bh) = (((x1 - x0) as f32).max(1.0), ((y1 - y0) as f32).max(1.0));
        let zoom = ((vw - 2.0 * self.options.fit_padding) / bw)
            .min((vh - 2.0 * self.options.fit_padding) / bh)
            .clamp(
                self.options.min_zoom,
                most.clamp(self.options.min_zoom, self.options.max_zoom),
            );
        self.zoom = zoom;
        self.pan = point(
            vw / 2.0 - (x0 as f32 + bw / 2.0) * zoom,
            vh / 2.0 - (y0 as f32 + bh / 2.0) * zoom,
        );
        self.touched = true;
    }

    fn middle(&self) -> Point<f32> {
        self.viewport
            .map_or(point(0.0, 0.0), |v| point(v.width / 2.0, v.height / 2.0))
    }

    /// Centre the document whenever the viewport changes, until the reader
    /// pans or zooms — an axis the document overflows starts at its edge, so
    /// a wide tree still shows its root — and bring a node asked for into
    /// view. The view calls it each frame, after [`Self::reflow`].
    pub fn frame(&mut self) {
        if let Some(viewport) = self
            .viewport
            .filter(|v| !self.touched && self.framed != Some(*v))
        {
            self.framed = Some(viewport);
            self.pan = match extent(&self.canvas.nodes) {
                None => point(viewport.width / 2.0, viewport.height / 2.0),
                Some((x0, y0, x1, y1)) => {
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
                    point(fit(viewport.width, x0, x1), fit(viewport.height, y0, y1))
                }
            };
        }
        self.bring_into_view();
    }

    /// Pan the least that brings the node asked for into view.
    fn bring_into_view(&mut self) {
        let (Some(id), Some(viewport)) = (self.reveal.take(), self.viewport) else {
            return;
        };
        let Some(node) = self.painted().node(&id) else {
            return;
        };
        let z = self.zoom;
        let shift = |start: f32, length: f32, view: f32| {
            if length + 2.0 * self.options.reveal_margin > view {
                view / 2.0 - (start + length / 2.0)
            } else if start < self.options.reveal_margin {
                self.options.reveal_margin - start
            } else if start + length > view - self.options.reveal_margin {
                view - self.options.reveal_margin - (start + length)
            } else {
                0.0
            }
        };
        let dx = shift(
            self.pan.x + node.x as f32 * z,
            node.width as f32 * z,
            viewport.width,
        );
        let dy = shift(
            self.pan.y + node.y as f32 * z,
            node.height as f32 * z,
            viewport.height,
        );
        if dx != 0.0 || dy != 0.0 {
            self.pan_by(dx, dy);
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// Switching to a tree that grows another way tidies the document: every
    /// pin is dropped and the trees are laid out again.
    pub fn set_layout(&mut self, layout: Layout) {
        let tidy = layout.flow.is_some() && layout.flow != self.layout.flow;
        self.layout = layout;
        if tidy {
            let unpins: Vec<Change> = self
                .canvas
                .nodes
                .iter()
                .filter_map(mindmap::unpin)
                .collect();
            self.apply(unpins);
        }
        self.stale = true;
    }

    /// What an app said, else the layout's.
    pub fn drag(&self) -> DragHandler {
        self.drag.unwrap_or(self.layout.drag)
    }

    /// Swap what dragging a node does, from the next press.
    pub fn set_drag(&mut self, handler: DragHandler) {
        self.drag = Some(handler);
    }

    pub fn snap(&self) -> Snap {
        self.snap
    }

    pub fn set_snap(&mut self, snap: Snap) {
        self.snap = snap;
    }

    /// Show `preview` in place of the document until [`Self::clear_preview`]:
    /// what a gesture in hand would leave, and what else its drop would do.
    pub(crate) fn set_preview(
        &mut self,
        preview: Canvas,
        pending: Vec<Change>,
        guides: Vec<Guide>,
    ) {
        self.preview = Some(preview);
        self.preview_containment.take();
        self.preview_stale = true;
        (self.pending, self.guides) = (pending, guides);
    }

    pub(crate) fn clear_preview(&mut self) {
        self.preview = None;
        self.preview_containment.take();
        self.pending.clear();
        self.guides.clear();
    }

    /// Settle what the view measured and the layout, `held` staying where the
    /// pointer has it. The document's part lands past the filter and outside
    /// history, announced; a gesture's part stays in its preview.
    pub fn reflow(
        &mut self,
        measured: impl IntoIterator<Item = (String, i64)>,
        held: Option<&str>,
    ) {
        let mut measured: Vec<_> = measured.into_iter().collect();
        measured.sort();
        let mut changes = Vec::new();
        for (id, height) in measured {
            if let Some(preview) = &mut self.preview
                && preview.node(&id) != self.canvas.node(&id)
            {
                // Measured at the size the gesture gave it.
                if let Some(width) = preview
                    .node(&id)
                    .filter(|n| n.height != height)
                    .map(|n| n.width)
                {
                    change::apply(
                        preview,
                        &Change::Resize {
                            id,
                            size: (width, height),
                        },
                    );
                    self.preview_stale = true;
                }
                continue;
            }
            let Some(node) = self.canvas.node(&id).filter(|n| n.height != height) else {
                continue;
            };
            let change = Change::Resize {
                size: (node.width, height),
                id,
            };
            change::apply(&mut self.canvas, &change);
            if let Some(preview) = &mut self.preview {
                change::apply(preview, &change);
                self.preview_stale = true;
            }
            changes.push(change);
            self.stale = true;
        }
        let (arrange, kinds) = (self.layout.arrange, &self.kinds);
        let holds = |node: &Node| kinds.holds(node);
        if std::mem::take(&mut self.stale) {
            let moves = arrange(&self.canvas, held, &holds);
            if !moves.is_empty() {
                let change = Change::MoveNodes { moves };
                change::apply(&mut self.canvas, &change);
                changes.push(change);
            }
        }
        if std::mem::take(&mut self.preview_stale)
            && let Some(preview) = &mut self.preview
        {
            self.preview_containment.take();
            let moves = arrange(preview, held, &holds);
            change::apply(preview, &Change::MoveNodes { moves });
        }
        if !changes.is_empty() {
            self.containment.take();
            self.events.push(CanvasEvent::Changed(changes));
        }
    }

    /// Drain pending events in order. Headless callers should drain regularly
    /// to release retained changes; the view does this automatically.
    pub fn take_events(&mut self) -> Vec<CanvasEvent> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn take_rewound(&mut self) -> bool {
        std::mem::take(&mut self.rewound)
    }
}

/// The batch, with the handle a connector left from written onto the edge it
/// adds — unless it was the plain middle of that side, which the side says.
fn named(changes: Vec<Change>, handle: &str, side: Side) -> Vec<Change> {
    if crate::handle::name(side) == handle {
        return changes;
    }
    changes
        .into_iter()
        .map(|change| match change {
            Change::AddEdge { mut edge, index } => {
                edge.extra
                    .insert(crate::edge::FROM_HANDLE.into(), handle.into());
                Change::AddEdge { edge, index }
            }
            other => other,
        })
        .collect()
}

/// The box around `nodes`: left, top, right, bottom.
pub(crate) fn extent<'a>(
    nodes: impl IntoIterator<Item = &'a Node>,
) -> Option<(i64, i64, i64, i64)> {
    nodes
        .into_iter()
        .map(|n| (n.x, n.y, n.x + n.width, n.y + n.height))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
}
