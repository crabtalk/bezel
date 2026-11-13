//! Every gesture as a tool: the view says what a press landed on, and the
//! first tool to take it holds the pointer until it comes up.
//!
//! ```ignore
//! cx.new(|cx| CanvasView::new(doc, layout::FREE, cx).with_tools(tool::defaults()))
//! ```
//!
//! [`defaults`] are answers, not the list: an app drops one, reorders them, or
//! writes its own with the same trait. A tool reads and commands a
//! [`CanvasEditor`] and says what it draws; the view paints it.

use crate::{
    change::{self, Change},
    clip, contain,
    drag::{Drag, Phase},
    edit::CanvasEditor,
    handle::{Handle, Role, Which},
    kind::{self, Capability, Sizing},
    mindmap,
    model::{Canvas, Edge, Node, Side},
    snap::{self, Guide, Snap},
};
use gpui::{CursorStyle, Modifiers, MouseButton, Pixels, Point};

/// What a press landed on.
// A handle carries where it sits, which is a measure, not a whole number.
#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    Node(String),
    Edge(String),
    /// A handle a picked node or edge paints, and what it is on.
    Handle {
        owner: String,
        handle: Handle,
    },
    Nothing,
}

/// A pointer event in the window's terms and the canvas's.
#[derive(Clone, Debug)]
pub struct Pointer {
    /// Where in the window, for what moves the view itself.
    pub screen: Point<Pixels>,
    /// The canvas point under it.
    pub at: Point<f32>,
    pub button: MouseButton,
    pub modifiers: Modifiers,
    pub space: bool,
    pub clicks: usize,
    pub hit: Hit,
}

impl Pointer {
    /// The canvas point, to the unit.
    pub fn round(&self) -> (i64, i64) {
        (self.at.x.round() as i64, self.at.y.round() as i64)
    }

    fn left(&self) -> bool {
        self.button == MouseButton::Left
    }
}

/// What a tool asks the view for once its commands have landed — the view
/// owns the editor typed into, so a tool names what it wants instead.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Wish {
    #[default]
    Nothing,
    /// Open a node, or edit it in place when its kind opens nothing.
    Enter(String),
    /// Edit an edge's label.
    EditEdge(String),
    /// Edit what was just added, its typing joining the add.
    EditAdded(String),
    ChooseConnection {
        from: String,
        handle: Handle,
        at: (i64, i64),
    },
}

/// What a tool works on: the editor, and what it asks of the view.
pub struct Hand<'a> {
    pub editor: &'a mut CanvasEditor,
    pub wish: Wish,
}

impl Hand<'_> {
    pub fn new(editor: &mut CanvasEditor) -> Hand<'_> {
        Hand {
            editor,
            wish: Wish::Nothing,
        }
    }
}

/// What a tool draws while it is held, in canvas units. The view paints it.
#[derive(Clone, Debug, PartialEq)]
pub enum Sketch {
    /// A box drawn from one point to another: a marquee.
    Box { from: Point<f32>, to: Point<f32> },
    /// A connector out of a node's side, out to the pointer.
    Connector {
        from: String,
        side: Side,
        to: Point<f32>,
    },
}

pub trait Tool {
    /// Whether this tool takes the press. It then holds the pointer until it
    /// comes up, and no other tool sees it.
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool;

    fn drag(&mut self, _pointer: &Pointer, _hand: &mut Hand) {}

    fn release(&mut self, _pointer: &Pointer, _hand: &mut Hand) {}

    /// The gesture was given up: `escape`, or a new document under it.
    fn cancel(&mut self, _hand: &mut Hand) {}

    /// What it draws while it is held.
    fn sketch(&self) -> Option<Sketch> {
        None
    }

    /// The node it has in hand, which layout leaves where the pointer has it.
    fn held(&self) -> Option<&str> {
        None
    }

    /// The cursor while this tool holds the pointer.
    fn cursor(&self) -> Option<CursorStyle> {
        self.held().map(|_| CursorStyle::ClosedHand)
    }

    /// Whether it is holding a gesture that a press near the view's edge
    /// should pan the view under.
    fn drifts(&self) -> bool {
        false
    }
}

/// Connect, resize, reconnect, edge pick, marquee, create, select and drag,
/// and pan — in the order a press is offered to them.
pub fn defaults() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(SpacePan::default()),
        Box::new(Connect::default()),
        Box::new(Resize::default()),
        Box::new(Reconnect::default()),
        Box::new(PickEdge),
        Box::new(Marquee::default()),
        Box::new(Create),
        Box::new(Select::default()),
        Box::new(Pan::default()),
    ]
}

/// Drag the background, or the middle button anywhere, to pan.
#[derive(Default)]
pub struct Pan {
    last: Option<Point<Pixels>>,
}

impl Tool for Pan {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let middle = pointer.button == MouseButton::Middle;
        if !middle && !(pointer.left() && pointer.hit == Hit::Nothing) {
            return false;
        }
        if !middle {
            hand.editor.select(None);
            hand.editor.select_edge(None);
        }
        self.last = Some(pointer.screen);
        true
    }

    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(last) = self.last.replace(pointer.screen) else {
            return;
        };
        let (dx, dy) = (
            (pointer.screen.x - last.x).as_f32(),
            (pointer.screen.y - last.y).as_f32(),
        );
        hand.editor.pan_by(dx, dy);
    }

    fn release(&mut self, _: &Pointer, _: &mut Hand) {
        self.last = None;
    }

    fn cancel(&mut self, _: &mut Hand) {
        self.last = None;
    }

    fn cursor(&self) -> Option<CursorStyle> {
        self.last.map(|_| CursorStyle::ClosedHand)
    }
}

#[derive(Default)]
pub struct SpacePan(Pan);

impl Tool for SpacePan {
    fn press(&mut self, pointer: &Pointer, _hand: &mut Hand) -> bool {
        if !pointer.space || !pointer.left() {
            return false;
        }
        self.0.last = Some(pointer.screen);
        true
    }
    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        self.0.drag(pointer, hand);
    }
    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        self.0.release(pointer, hand);
    }
    fn cancel(&mut self, hand: &mut Hand) {
        self.0.cancel(hand);
    }
    fn cursor(&self) -> Option<CursorStyle> {
        self.0.cursor()
    }
}

/// A shift-drag on the background selects what its box touches.
#[derive(Default)]
pub struct Marquee {
    marking: Option<Marking>,
}

struct Marking {
    from: Point<f32>,
    to: Point<f32>,
    /// What was selected before the box was drawn.
    base: Vec<String>,
}

impl Tool for Marquee {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        if !pointer.left() || pointer.hit != Hit::Nothing || !pointer.modifiers.shift {
            return false;
        }
        self.marking = Some(Marking {
            from: pointer.at,
            to: pointer.at,
            base: hand
                .editor
                .selected_nodes()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        });
        true
    }

    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(marking) = &mut self.marking else {
            return;
        };
        marking.to = pointer.at;
        let (a, b) = (marking.from, marking.to);
        let (x0, x1) = (a.x.min(b.x) as i64, a.x.max(b.x) as i64);
        let (y0, y1) = (a.y.min(b.y) as i64, a.y.max(b.y) as i64);
        let touched = hand.editor.painted().nodes.iter().filter(|n| {
            n.x < x1
                && n.x + n.width > x0
                && n.y < y1
                && n.y + n.height > y0
                && !marking.base.contains(&n.id)
        });
        let ids = marking
            .base
            .iter()
            .cloned()
            .chain(touched.map(|n| n.id.clone()))
            .collect();
        hand.editor.set_selection(ids);
    }

    fn release(&mut self, _: &Pointer, _: &mut Hand) {
        self.marking = None;
    }

    fn cancel(&mut self, _: &mut Hand) {
        self.marking = None;
    }

    fn sketch(&self) -> Option<Sketch> {
        let marking = self.marking.as_ref()?;
        Some(Sketch::Box {
            from: marking.from,
            to: marking.to,
        })
    }

    fn drifts(&self) -> bool {
        self.marking.is_some()
    }
}

/// A double-click on the background makes a node there.
pub struct Create;

impl Tool for Create {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        if !pointer.left() || pointer.hit != Hit::Nothing || pointer.clicks < 2 {
            return false;
        }
        hand.editor.select(None);
        hand.editor.select_edge(None);
        if let Some(added) = hand.editor.add_root(pointer.round()) {
            hand.wish = Wish::EditAdded(added);
        }
        true
    }
}

/// A click picks an edge; a double-click edits its label.
pub struct PickEdge;

impl Tool for PickEdge {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let Hit::Edge(id) = &pointer.hit else {
            return false;
        };
        if !pointer.left() {
            return false;
        }
        let id = id.clone();
        hand.editor.select_edge(Some(id.clone()));
        if pointer.clicks >= 2 {
            hand.wish = Wish::EditEdge(id);
        }
        true
    }
}

/// Press a node to select it, drag it to move it, double-click to open it.
#[derive(Default)]
pub struct Select {
    held: Option<Held>,
}

struct Held {
    id: String,
    /// The rest of the selection, carried along.
    with: Vec<String>,
    /// Where the node was when the press began.
    origin: (i64, i64),
    /// The canvas point the press began at.
    from: Point<f32>,
    moved: bool,
    duplicate: Option<Canvas>,
    additive: bool,
    toggle: bool,
}

impl Tool for Select {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let Hit::Node(id) = &pointer.hit else {
            return false;
        };
        if !pointer.left() {
            return false;
        }
        let id = id.clone();
        let selected: Vec<String> = hand
            .editor
            .selected_nodes()
            .into_iter()
            .map(str::to_owned)
            .collect();
        if pointer.modifiers.platform {
            let mut ids = selected;
            match ids.iter().position(|s| *s == id) {
                Some(at) => drop(ids.remove(at)),
                None => ids.push(id),
            }
            hand.editor.set_selection(ids);
            return true;
        }
        if pointer.clicks >= 2 {
            hand.editor.select(Some(id.clone()));
            hand.wish = Wish::Enter(id);
            return true;
        }
        // Pressing one of a selection keeps the rest, to drag them together.
        let mut with: Vec<String> = selected.iter().filter(|s| **s != id).cloned().collect();
        if with.len() == selected.len() && !pointer.modifiers.shift {
            with.clear();
        }
        let ids = with.iter().cloned().chain([id.clone()]).collect();
        hand.editor.set_selection(ids);
        if !hand.editor.node_can(&id, Capability::Draggable) {
            return true;
        }
        with.retain(|id| hand.editor.node_can(id, Capability::Draggable));
        let duplicate = pointer.modifiers.alt.then(|| {
            let ids = hand
                .editor
                .selected_nodes()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            clip::fragment(hand.editor.canvas(), &hand.editor.contents(&ids))
        });
        if duplicate.is_none() {
            hand.editor.group = Some(hand.editor.next_group());
            hand.editor.bring_to_front();
        }
        if let Some(node) = hand.editor.canvas().node(&id) {
            self.held = Some(Held {
                origin: (node.x, node.y),
                id: id.clone(),
                with,
                from: pointer.at,
                moved: false,
                duplicate,
                additive: pointer.modifiers.shift,
                toggle: pointer.modifiers.shift && selected.contains(&id),
            });
        }
        true
    }

    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let zoom = hand.editor.zoom();
        let Some(held) = &mut self.held else {
            return;
        };
        let travelled = (pointer.at.x - held.from.x)
            .abs()
            .max((pointer.at.y - held.from.y).abs());
        if !held.moved && travelled * zoom < hand.editor.options().drag_threshold {
            return;
        }
        held.moved = true;
        let held = self.held.as_ref().expect("just held");
        let (changes, guides) = gesture(hand.editor, held, pointer, Phase::Move);
        if held.duplicate.is_some() {
            hand.editor.set_preview(changes, Vec::new(), guides);
        } else {
            preview(hand.editor, changes, guides);
        }
    }

    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(held) = self.held.take() else {
            return;
        };
        // A click on one of a selection, without a drag, selects just it.
        if !held.moved {
            if held.toggle {
                hand.editor.set_selection(held.with);
            } else if !held.additive {
                hand.editor.select(Some(held.id));
            }
            hand.editor.group = None;
            return;
        }
        hand.editor.clear_preview();
        let (mut drop, _) = gesture(hand.editor, &held, pointer, Phase::Drop);
        let ids: Vec<String> = std::iter::once(held.id.clone())
            .chain(held.with.iter().cloned())
            .collect();
        // A node carried out of the container it names lets it go.
        let mut after = hand.editor.canvas().clone();
        change::apply_all(&mut after, &drop);
        if held.duplicate.is_none() {
            drop.extend(contain::loosen(&after, &ids));
        }
        let copies = if held.duplicate.is_some() {
            drop.iter()
                .filter_map(|change| match change {
                    Change::AddNode { node, .. } => Some(node.id.clone()),
                    _ => None,
                })
                .collect()
        } else {
            Vec::new()
        };
        if hand.editor.submit(drop) && !copies.is_empty() {
            hand.editor.set_selection(copies);
        }
        hand.editor.group = None;
    }

    fn cancel(&mut self, hand: &mut Hand) {
        self.held = None;
        hand.editor.group = None;
        hand.editor.clear_preview();
    }

    fn held(&self) -> Option<&str> {
        self.held
            .as_ref()
            .filter(|held| held.moved)
            .map(|held| held.id.as_str())
    }

    fn drifts(&self) -> bool {
        self.held.as_ref().is_some_and(|held| held.moved)
    }
}

/// Drag a side dot to draw a connector.
#[derive(Default)]
pub struct Connect {
    drawing: Option<Drawing>,
}

struct Drawing {
    from: String,
    /// The handle it was drawn out of, which says where it leaves and what
    /// the edge writes down.
    handle: Handle,
    side: Side,
    start: Point<f32>,
    to: Point<f32>,
}

impl Tool for Connect {
    fn press(&mut self, pointer: &Pointer, _: &mut Hand) -> bool {
        let Hit::Handle { owner, handle } = &pointer.hit else {
            return false;
        };
        let Some(side) = handle.side().filter(|_| handle.role == Role::Connect) else {
            return false;
        };
        if !pointer.left() {
            return false;
        }
        self.drawing = Some(Drawing {
            from: owner.clone(),
            handle: handle.clone(),
            side,
            start: pointer.at,
            to: pointer.at,
        });
        true
    }

    fn drag(&mut self, pointer: &Pointer, _: &mut Hand) {
        if let Some(drawing) = &mut self.drawing {
            drawing.to = pointer.at;
        }
    }

    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(drawing) = self.drawing.take() else {
            return;
        };
        let pulled = (drawing.to.x - drawing.start.x)
            .abs()
            .max((drawing.to.y - drawing.start.y).abs());
        if pulled * hand.editor.zoom() < hand.editor.options().drag_threshold {
            return;
        }
        let at = (pointer.at.x.round() as i64, pointer.at.y.round() as i64);
        if hand.editor.options().choose_connection_node
            && hand.editor.node_under(at, &drawing.from).is_none()
            && hand.editor.node_can(&drawing.from, Capability::Connectable)
        {
            hand.wish = Wish::ChooseConnection {
                from: drawing.from,
                handle: drawing.handle,
                at,
            };
            return;
        }
        let drawn = hand.editor.connect(&drawing.from, &drawing.handle, at);
        if let Some(added) = drawn {
            hand.wish = Wish::EditAdded(added);
        }
    }

    fn cancel(&mut self, _: &mut Hand) {
        self.drawing = None;
    }

    fn sketch(&self) -> Option<Sketch> {
        let drawing = self.drawing.as_ref()?;
        Some(Sketch::Connector {
            from: drawing.from.clone(),
            side: drawing.side,
            to: drawing.to,
        })
    }

    fn drifts(&self) -> bool {
        self.drawing.is_some()
    }
}

/// Pull the corner to resize a node.
#[derive(Default)]
pub struct Resize {
    pulling: Option<Pulling>,
}

struct Pulling {
    node: Node,
    from: Point<f32>,
    axes: (i64, i64),
    cursor: CursorStyle,
    grows: bool,
}

impl Tool for Resize {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let Hit::Handle { owner, handle } = &pointer.hit else {
            return false;
        };
        if handle.role != Role::Resize {
            return false;
        }
        if !pointer.left() || !hand.editor.node_can(owner, Capability::Resizable) {
            return false;
        }
        let Some(found) = hand.editor.canvas().node(owner) else {
            return false;
        };
        let grows = hand.editor.kinds().sizing(found) == Sizing::Grows;
        let Some(axes) = handle.spot.resize_axes() else {
            return false;
        };
        self.pulling = Some(Pulling {
            node: found.clone(),
            from: pointer.at,
            axes,
            cursor: handle.cursor(),
            grows,
        });
        true
    }

    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(pulling) = &self.pulling else {
            return;
        };
        let original = &pulling.node;
        let (ax, ay) = pulling.axes;
        let snap = hand.editor.snap();
        let grid = |value: i64| {
            if pointer.space {
                value
            } else {
                snap.grid.map_or(value, |step| snap::round_to(value, step))
            }
        };
        let dx = (pointer.at.x - pulling.from.x).round() as i64;
        let dy = (pointer.at.y - pulling.from.y).round() as i64;
        let mut width = if ax == 0 {
            original.width
        } else {
            grid(original.width + ax * dx)
        };
        let mut height = if ay == 0 {
            original.height
        } else {
            grid(original.height + ay * dy)
        };
        let (min_w, min_h) = hand.editor.options().min_size;
        let min_h = if original.is_label() { 1 } else { min_h };
        let scales_text = original.is_label() && ax != 0 && ay != 0;
        if scales_text || pointer.modifiers.shift && !original.is_label() {
            let sx = width as f64 / original.width.max(1) as f64;
            let sy = height as f64 / original.height.max(1) as f64;
            let scale = if ax == 0 {
                sy
            } else if ay == 0 || (sx - 1.0).abs() >= (sy - 1.0).abs() {
                sx
            } else {
                sy
            };
            let scale = scale
                .max(min_w as f64 / original.width.max(1) as f64)
                .max(min_h as f64 / original.height.max(1) as f64);
            width = (original.width as f64 * scale).round() as i64;
            height = (original.height as f64 * scale).round() as i64;
        }
        let mut node = original.clone();
        node.width = width.max(min_w);
        node.height = height.max(min_h);
        if scales_text {
            node = original.clone();
            node.scale(width as f32 / original.width.max(1) as f32);
        }
        if ax < 0 {
            node.x += original.width - node.width;
        }
        if ay < 0 {
            node.y += original.height - node.height;
        }
        // Measuring keeps a growing node as tall as its content.
        if pulling.grows && !node.is_label() {
            node.extra
                .insert(kind::MIN_HEIGHT.into(), node.height.into());
        }
        hand.editor
            .set_preview(vec![Change::UpdateNode { node }], Vec::new(), Vec::new());
    }

    /// The pull lands as one change: a resize, or for a growing node the node
    /// with its least height.
    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        self.drag(pointer, hand);
        let Some(pulling) = self.pulling.take() else {
            return;
        };
        let now = hand.editor.painted().node(&pulling.node.id).cloned();
        hand.editor.clear_preview();
        if let Some(node) = now.filter(|node| node != &pulling.node) {
            hand.editor.submit([Change::UpdateNode { node }]);
        }
    }

    fn cancel(&mut self, hand: &mut Hand) {
        self.pulling = None;
        hand.editor.clear_preview();
    }

    fn held(&self) -> Option<&str> {
        self.pulling
            .as_ref()
            .map(|pulling| pulling.node.id.as_str())
    }

    fn cursor(&self) -> Option<CursorStyle> {
        self.pulling.as_ref().map(|pulling| pulling.cursor)
    }
}

/// Carry an edge's end onto another node.
#[derive(Default)]
pub struct Reconnect {
    carrying: Option<Carrying>,
}

struct Carrying {
    edge: String,
    which: Which,
}

impl Tool for Reconnect {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let Hit::Handle { owner, handle } = &pointer.hit else {
            return false;
        };
        let Role::Reconnect(which) = handle.role else {
            return false;
        };
        if !pointer.left() || !hand.editor.edge_can(owner, Capability::Reconnectable) {
            return false;
        }
        self.carrying = Some(Carrying {
            edge: owner.clone(),
            which,
        });
        true
    }

    /// The end follows the pointer as far as the node under it, which the
    /// preview shows joined.
    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(carrying) = &self.carrying else {
            return;
        };
        let Some(edge) = carried(hand.editor, carrying, pointer.round()) else {
            hand.editor.clear_preview();
            return;
        };
        hand.editor
            .set_preview(vec![Change::UpdateEdge { edge }], Vec::new(), Vec::new());
    }

    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(carrying) = self.carrying.take() else {
            return;
        };
        hand.editor.clear_preview();
        if let Some(edge) = carried(hand.editor, &carrying, pointer.round()) {
            hand.editor.submit([Change::UpdateEdge { edge }]);
        }
    }

    fn cancel(&mut self, hand: &mut Hand) {
        self.carrying = None;
        hand.editor.clear_preview();
    }

    fn drifts(&self) -> bool {
        self.carrying.is_some()
    }
}

/// The edge with its end carried onto the node at `at`, if there is one there
/// that will take it and it is not where the end already is.
fn carried(editor: &CanvasEditor, carrying: &Carrying, at: (i64, i64)) -> Option<Edge> {
    let mut edge = editor.canvas().edge(&carrying.edge)?.clone();
    let onto = editor.node_under(at, "")?.to_owned();
    if !editor.node_can(&onto, Capability::Connectable) {
        return None;
    }
    let end = match carrying.which {
        Which::From => &mut edge.from_node,
        Which::To => &mut edge.to_node,
    };
    if *end == onto {
        return None;
    }
    *end = onto;
    // The side it left is the pointer's to find again.
    match carrying.which {
        Which::From => edge.from_side = None,
        Which::To => edge.to_side = None,
    }
    Some(edge)
}

/// The held node's moves shown over the document, and the rest of what the
/// drop would do kept to draw.
fn preview(editor: &mut CanvasEditor, changes: Vec<Change>, guides: Vec<Guide>) {
    let mut shown = Vec::new();
    let mut pending = Vec::new();
    for change in changes {
        match change {
            Change::MoveNodes { .. } => shown.push(change),
            other => pending.push(other),
        }
    }
    editor.set_preview(shown, pending, guides);
}

/// The drag handler's answer to the held node carried to `at`, against the
/// document, settled by the snap, and the guides that caught it.
fn gesture(
    editor: &CanvasEditor,
    held: &Held,
    pointer: &Pointer,
    phase: Phase,
) -> (Vec<Change>, Vec<Guide>) {
    let at = pointer.at;
    // The pointer's travel in canvas units, which a drift under it is already
    // part of: both ends are read against the pan of their own frame.
    let mut delta = (
        (at.x - held.from.x).round() as i64,
        (at.y - held.from.y).round() as i64,
    );
    if pointer.modifiers.shift {
        if delta.0.abs() >= delta.1.abs() {
            delta.1 = 0;
        } else {
            delta.0 = 0;
        }
    }
    let ids: Vec<String> = std::iter::once(held.id.clone())
        .chain(held.with.iter().cloned())
        .collect();
    let holds = |node: &Node| editor.kinds().holds(node);
    let canvas = editor.canvas();
    let carried = editor.contents(&ids);
    let contents: Vec<String> = carried
        .iter()
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect();
    let mut guides = Vec::new();
    let mut snapping = editor.snap();
    snapping.guides ^= pointer.modifiers.platform;
    if !pointer.space
        && snapping != Snap::default()
        && let Some(node) = canvas.node(&held.id)
    {
        let moving: Vec<String> = carried
            .iter()
            .flat_map(|id| mindmap::branch_of(canvas, id))
            .collect();
        let to = (held.origin.0 + delta.0, held.origin.1 + delta.1);
        let reach = (editor.options().snap_reach / editor.zoom()).round() as i64;
        let size = (node.width, node.height);
        if phase == Phase::Move {
            snapping.grid = None;
        }
        let visible = editor.visible();
        let (settled, caught) = snap::settle(
            editor.painted(),
            &moving,
            to,
            size,
            snapping,
            reach,
            visible,
        );
        delta = (settled.0 - held.origin.0, settled.1 - held.origin.1);
        guides = caught;
    }
    if pointer.modifiers.shift {
        if (at.x - held.from.x).abs() >= (at.y - held.from.y).abs() {
            delta.1 = 0;
        } else {
            delta.0 = 0;
        }
    }
    if let Some(fragment) = &held.duplicate {
        let ((x, y), _) = clip::bounds(fragment).expect("a selected node has bounds");
        return (
            clip::paste(canvas, fragment, (x + delta.0, y + delta.1), None),
            guides,
        );
    }
    // A node is not dropped on itself, on what it holds, or on what the layout
    // carries with it — its branch, under a tree.
    let except: Vec<String> = ids
        .iter()
        .flat_map(|id| (editor.layout().reach)(canvas, id))
        .chain(contents.iter().cloned())
        .collect();
    let pointer = (at.x.round() as i64, at.y.round() as i64);
    let drag = Drag {
        id: &held.id,
        with: &held.with,
        contents: &contents,
        origin: held.origin,
        delta,
        over: editor
            .painted_containment()
            .topmost(editor.painted(), pointer, &except, |n| !holds(n)),
        phase,
    };
    ((editor.drag())(canvas, &drag), guides)
}
