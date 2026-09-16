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

use gpui::{Modifiers, MouseButton, Pixels, Point};

use crate::{
    change::{self, Change},
    contain,
    drag::{Drag, Phase},
    edit::CanvasEditor,
    kind::{self, Capability, Sizing},
    mindmap,
    model::{Node, Side},
    snap::{self, Guide, Snap},
};

/// How far a press travels before it is a drag, in screen pixels.
const DRAG_SLOP: f32 = 3.0;
/// How near a dragged box's line comes to another's before it catches, in
/// screen pixels.
const GUIDE_REACH: f32 = 6.0;
/// The smallest box a corner pulls a node to, in canvas units.
const MIN_SIZE: (i64, i64) = (60, 32);

/// What a press landed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hit {
    Node(String),
    Edge(String),
    /// A handle a picked node paints.
    Handle {
        node: String,
        part: Part,
    },
    Nothing,
}

/// Which handle was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// A side dot, which draws a connector.
    Connect(Side),
    /// The corner, which resizes.
    Resize,
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Wish {
    #[default]
    Nothing,
    /// Open a node, or edit it in place when its kind opens nothing.
    Enter(String),
    /// Edit an edge's label.
    EditEdge(String),
    /// Edit what was just added, its typing joining the add.
    EditAdded(String),
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

    /// Whether it is holding a gesture that a press near the view's edge
    /// should pan the view under.
    fn drifts(&self) -> bool {
        false
    }
}

/// Pan, marquee, create, edge pick, select and drag, connect and resize — in
/// the order a press is offered to them.
pub fn defaults() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(Connect::default()),
        Box::new(Resize::default()),
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
        if pointer.modifiers.shift || pointer.modifiers.platform {
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
        if with.len() == selected.len() {
            with.clear();
        }
        let ids = with.iter().cloned().chain([id.clone()]).collect();
        hand.editor.set_selection(ids);
        if !hand.editor.node_can(&id, Capability::Draggable) {
            return true;
        }
        if let Some(node) = hand.editor.canvas().node(&id) {
            self.held = Some(Held {
                origin: (node.x, node.y),
                id,
                with,
                from: pointer.at,
                moved: false,
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
        if !held.moved && travelled * zoom < DRAG_SLOP {
            return;
        }
        held.moved = true;
        let held = self.held.as_ref().expect("just held");
        let (changes, guides) = gesture(hand.editor, held, pointer.at, Phase::Move);
        preview(hand.editor, changes, guides);
    }

    fn release(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(held) = self.held.take() else {
            return;
        };
        // A click on one of a selection, without a drag, selects just it.
        if !held.moved {
            hand.editor.select(Some(held.id));
            return;
        }
        hand.editor.clear_preview();
        let (mut drop, _) = gesture(hand.editor, &held, pointer.at, Phase::Drop);
        let ids: Vec<String> = std::iter::once(held.id.clone())
            .chain(held.with.iter().cloned())
            .collect();
        // A node carried out of the container it names lets it go.
        let mut after = hand.editor.canvas().clone();
        change::apply_all(&mut after, &drop);
        drop.extend(contain::loosen(&after, &ids));
        hand.editor.submit(drop);
    }

    fn cancel(&mut self, hand: &mut Hand) {
        self.held = None;
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
    side: Side,
    start: Point<f32>,
    to: Point<f32>,
}

impl Tool for Connect {
    fn press(&mut self, pointer: &Pointer, _: &mut Hand) -> bool {
        let Hit::Handle {
            node,
            part: Part::Connect(side),
        } = &pointer.hit
        else {
            return false;
        };
        if !pointer.left() {
            return false;
        }
        self.drawing = Some(Drawing {
            from: node.clone(),
            side: *side,
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
        if pulled * hand.editor.zoom() < DRAG_SLOP {
            return;
        }
        let at = (pointer.at.x.round() as i64, pointer.at.y.round() as i64);
        if let Some(added) = hand.editor.connect(&drawing.from, drawing.side, at) {
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
    id: String,
    /// The canvas point the press began at.
    from: Point<f32>,
    /// The box as the press found it.
    size: (i64, i64),
    /// Its content may run past the height it is pulled to.
    grows: bool,
}

impl Tool for Resize {
    fn press(&mut self, pointer: &Pointer, hand: &mut Hand) -> bool {
        let Hit::Handle {
            node,
            part: Part::Resize,
        } = &pointer.hit
        else {
            return false;
        };
        if !pointer.left() || !hand.editor.node_can(node, Capability::Resizable) {
            return false;
        }
        let Some(found) = hand.editor.canvas().node(node) else {
            return false;
        };
        let grows = hand.editor.kinds().get(&found.kind).rules.sizing == Sizing::Grows;
        self.pulling = Some(Pulling {
            id: node.clone(),
            from: pointer.at,
            size: (found.width, found.height),
            grows,
        });
        true
    }

    fn drag(&mut self, pointer: &Pointer, hand: &mut Hand) {
        let Some(pulling) = &self.pulling else {
            return;
        };
        let snap = hand.editor.snap();
        let grid = |value: i64| snap.grid.map_or(value, |step| snap::round_to(value, step));
        let Some(mut node) = hand.editor.canvas().node(&pulling.id).cloned() else {
            return;
        };
        let pulled = |to: f32, from: f32| (to - from).round() as i64;
        let width = grid(pulling.size.0 + pulled(pointer.at.x, pulling.from.x));
        let height = grid(pulling.size.1 + pulled(pointer.at.y, pulling.from.y));
        (node.width, node.height) = (width.max(MIN_SIZE.0), height.max(MIN_SIZE.1));
        // Measuring keeps a growing node as tall as its content.
        if pulling.grows {
            node.extra
                .insert(kind::MIN_HEIGHT.into(), node.height.into());
        }
        let mut shown = hand.editor.canvas().clone();
        change::apply(&mut shown, &Change::UpdateNode { node });
        hand.editor.set_preview(shown, Vec::new(), Vec::new());
    }

    /// The pull lands as one change: a resize, or for a growing node the node
    /// with its least height.
    fn release(&mut self, _: &Pointer, hand: &mut Hand) {
        let Some(pulling) = self.pulling.take() else {
            return;
        };
        let now = hand.editor.painted().node(&pulling.id).cloned();
        hand.editor.clear_preview();
        let Some(now) = now.filter(|now| hand.editor.canvas().node(&pulling.id) != Some(now))
        else {
            return;
        };
        let change = match pulling.grows {
            true => Change::UpdateNode { node: now },
            false => Change::Resize {
                size: (now.width, now.height),
                id: now.id,
            },
        };
        hand.editor.submit([change]);
    }

    fn cancel(&mut self, hand: &mut Hand) {
        self.pulling = None;
        hand.editor.clear_preview();
    }

    fn drifts(&self) -> bool {
        self.pulling.is_some()
    }
}

/// The held node's moves shown over the document, and the rest of what the
/// drop would do kept to draw.
fn preview(editor: &mut CanvasEditor, changes: Vec<Change>, guides: Vec<Guide>) {
    let mut shown = editor.canvas().clone();
    let mut pending = Vec::new();
    for change in changes {
        match change {
            Change::MoveNodes { .. } => drop(change::apply(&mut shown, &change)),
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
    at: Point<f32>,
    phase: Phase,
) -> (Vec<Change>, Vec<Guide>) {
    // The pointer's travel in canvas units, which a drift under it is already
    // part of: both ends are read against the pan of their own frame.
    let mut delta = (
        (at.x - held.from.x).round() as i64,
        (at.y - held.from.y).round() as i64,
    );
    let ids: Vec<String> = std::iter::once(held.id.clone())
        .chain(held.with.iter().cloned())
        .collect();
    let holds = |node: &Node| editor.kinds().holds(node);
    let canvas = editor.canvas();
    let carried = contain::with_contents(canvas, &ids, holds);
    let contents: Vec<String> = carried
        .iter()
        .filter(|id| !ids.contains(id))
        .cloned()
        .collect();
    let mut guides = Vec::new();
    if editor.snap() != Snap::default()
        && let Some(node) = canvas.node(&held.id)
    {
        let moving: Vec<String> = carried
            .iter()
            .flat_map(|id| mindmap::branch_of(canvas, id))
            .collect();
        let to = (held.origin.0 + delta.0, held.origin.1 + delta.1);
        let reach = (GUIDE_REACH / editor.zoom()).round() as i64;
        let size = (node.width, node.height);
        let (settled, caught) =
            snap::settle(editor.painted(), &moving, to, size, editor.snap(), reach);
        delta = (settled.0 - held.origin.0, settled.1 - held.origin.1);
        guides = caught;
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
        over: contain::topmost(editor.painted(), pointer, &except, |n| !holds(n), holds),
        phase,
    };
    ((editor.drag())(canvas, &drag), guides)
}
