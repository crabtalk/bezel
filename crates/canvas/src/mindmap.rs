//! A mindmap is a canvas whose edges form a tree: a node's children are the
//! edges leaving it, in edge order. An edge marked `"tree": false` is a cross
//! link, never a branch.
//!
//! Layout writes positions back into the [`Canvas`], so the file stays plain
//! JSON Canvas that any other viewer opens as it was last laid out. The edits
//! here answer the [`Change`]s they make, for the view to submit.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::{
    change::{self, Change},
    group,
    layout::Arrow,
    model::{Canvas, Edge, End, GROUP, Node, Side},
};

/// Between a parent's right side and its children's left.
pub const GAP_X: i64 = 64;
/// Between two sibling subtrees.
pub const GAP_Y: i64 = 16;
/// A new node's box, until its content is measured.
pub const NODE_WIDTH: i64 = 200;
pub const NODE_HEIGHT: i64 = 40;
/// Our own node field, not the spec's: set when a node is dragged, and layout
/// leaves its position alone from then on.
pub const PINNED: &str = "pinned";
/// Our own edge field: `false` makes the edge a cross link.
pub const TREE: &str = "tree";

pub fn is_pinned(node: &Node) -> bool {
    flag(&node.extra, PINNED).unwrap_or(false)
}

pub fn is_branch(edge: &Edge) -> bool {
    flag(&edge.extra, TREE).unwrap_or(true)
}

fn flag(extra: &serde_json::Map<String, Value>, key: &str) -> Option<bool> {
    extra.get(key).and_then(Value::as_bool)
}

/// Put a node where it was dropped and keep it there.
pub fn pin(canvas: &mut Canvas, id: &str, x: i64, y: i64) -> Option<()> {
    let node = canvas.node_mut(id)?;
    node.x = x;
    node.y = y;
    node.extra.insert(PINNED.into(), Value::Bool(true));
    Some(())
}

/// Hand a pinned node back to layout; `None` when it is not pinned.
pub fn unpin(node: &Node) -> Option<Change> {
    is_pinned(node).then(|| {
        let mut node = node.clone();
        node.extra.remove(PINNED);
        Change::UpdateNode { node }
    })
}

/// `ids` moved `by`, a group with its members, each with its pinned
/// descendants keeping their place beside it as layout keeps the rest. `pin`
/// keeps them there.
pub fn carry(canvas: &Canvas, ids: &[String], by: (i64, i64), pin: bool) -> Vec<Change> {
    let shifted = |node: &Node| (node.x + by.0, node.y + by.1);
    let mut changes = Vec::new();
    let mut moves = Vec::new();
    let mut seen = HashSet::new();
    for id in &group::with_members(canvas, ids) {
        let Some(node) = canvas.node(id) else {
            continue;
        };
        if seen.insert(id.clone()) {
            if pin && !is_pinned(node) {
                let mut pinned = node.clone();
                (pinned.x, pinned.y) = shifted(node);
                pinned.extra.insert(PINNED.into(), Value::Bool(true));
                changes.push(Change::UpdateNode { node: pinned });
            }
            moves.push((id.clone(), shifted(node)));
        }
        let mut below: Vec<String> = descendants(canvas, id).into_iter().collect();
        below.sort();
        for below in below {
            if let Some(node) = canvas.node(&below).filter(|node| is_pinned(node))
                && seen.insert(below.clone())
            {
                moves.push((below, shifted(node)));
            }
        }
    }
    changes.push(Change::MoveNodes { moves });
    changes
}

/// Tree motion from a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    Parent,
    FirstChild,
    PrevSibling,
    NextSibling,
}

pub fn parent<'a>(canvas: &'a Canvas, id: &str) -> Option<&'a str> {
    canvas
        .edges
        .iter()
        .find(|edge| edge.to_node == id && is_branch(edge))
        .map(|edge| edge.from_node.as_str())
}

pub fn children<'a>(canvas: &'a Canvas, id: &'a str) -> impl Iterator<Item = &'a str> {
    canvas
        .edges
        .iter()
        .filter(move |edge| edge.from_node == id && is_branch(edge))
        .map(|edge| edge.to_node.as_str())
}

/// Nodes no branch points at. Groups are frames, not branches.
pub fn roots(canvas: &Canvas) -> impl Iterator<Item = &Node> {
    canvas
        .nodes
        .iter()
        .filter(|node| node.kind != GROUP && parent(canvas, &node.id).is_none())
}

pub fn step(canvas: &Canvas, id: &str, toward: Toward) -> Option<String> {
    let sibling = |offset: isize| {
        let siblings: Vec<&str> = children(canvas, parent(canvas, id)?).collect();
        let at = siblings.iter().position(|s| *s == id)?;
        siblings
            .get(at.checked_add_signed(offset)?)
            .map(|s| s.to_string())
    };
    match toward {
        Toward::Parent => parent(canvas, id).map(str::to_owned),
        Toward::FirstChild => children(canvas, id).next().map(str::to_owned),
        Toward::PrevSibling => sibling(-1),
        Toward::NextSibling => sibling(1),
    }
}

/// The way a tree grows from its root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Right,
    Left,
    Down,
    /// A root's children split right and left, by the room they take.
    Both,
}

/// Tree motion by arrow key, the way `flow` grows: toward the root is the
/// parent, away is the first child, across is a sibling. On a balanced root,
/// left and right pick a side.
pub fn walk(canvas: &Canvas, id: &str, flow: Flow, arrow: Arrow) -> Option<String> {
    let mid = |id: &str| canvas.node(id).map(|node| node.x + node.width / 2);
    // Which way a node grows from its parent.
    let side = |id: &str| match flow {
        Flow::Both => match (parent(canvas, id).and_then(mid), mid(id)) {
            (Some(parent), Some(x)) if x < parent => Flow::Left,
            _ => Flow::Right,
        },
        flow => flow,
    };
    let here = match (flow, parent(canvas, id), arrow) {
        (Flow::Both, None, Arrow::Left) => Flow::Left,
        (Flow::Both, None, _) => Flow::Right,
        _ => side(id),
    };
    let sibling = |offset: isize| {
        let siblings: Vec<&str> = children(canvas, parent(canvas, id)?)
            .filter(|sibling| side(sibling) == here)
            .collect();
        let at = siblings.iter().position(|s| *s == id)?;
        siblings
            .get(at.checked_add_signed(offset)?)
            .map(|s| s.to_string())
    };
    match (here, arrow) {
        (Flow::Right | Flow::Both, Arrow::Left)
        | (Flow::Left, Arrow::Right)
        | (Flow::Down, Arrow::Up) => parent(canvas, id).map(str::to_owned),
        (Flow::Right | Flow::Both, Arrow::Right)
        | (Flow::Left, Arrow::Left)
        | (Flow::Down, Arrow::Down) => children(canvas, id)
            .find(|child| side(child) == here)
            .map(str::to_owned),
        (_, Arrow::Up) | (Flow::Down, Arrow::Left) => sibling(-1),
        (_, Arrow::Down) | (Flow::Down, Arrow::Right) => sibling(1),
    }
}

/// Lay every tree out to the right of its root, which stays where it is.
///
/// A node reached twice hangs off the first parent that reaches it, so a stray
/// cycle cannot loop the layout.
pub fn layout(canvas: &mut Canvas) {
    layout_holding(canvas, None);
}

/// [`layout`], keeping `held` where it is as if pinned — the node a drag has
/// in hand.
pub fn layout_holding(canvas: &mut Canvas, held: Option<&str>) {
    let moves = arrange(canvas, held, Flow::Right);
    change::apply(canvas, &Change::MoveNodes { moves });
}

/// Where every tree growing in `flow` puts each node, leaving out those
/// already there. Roots stay; `held` stays as if pinned.
pub fn arrange(canvas: &Canvas, held: Option<&str>, flow: Flow) -> Vec<(String, (i64, i64))> {
    let index = Index::of(canvas);
    let mut seen = HashSet::new();
    let mut pass = Pass {
        canvas,
        held,
        spans: HashMap::new(),
        moves: Vec::new(),
    };
    let roots = canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.kind != GROUP && !index.parented.contains(node.id.as_str()));
    for (ix, root) in roots {
        let tree = index.branch(canvas, ix, &mut seen);
        pass.measure(&tree, flow == Flow::Down);
        let kids: Vec<&Branch> = tree.children.iter().collect();
        let at = (root.x, root.y);
        if flow == Flow::Both {
            let (right, left) = pass.split(&kids);
            pass.column(root, at, &right, Flow::Right);
            pass.column(root, at, &left, Flow::Left);
        } else {
            pass.column(root, at, &kids, flow);
        }
    }
    pass.moves
}

/// `node` as a root at `at`, under an id nothing holds.
pub fn root(canvas: &Canvas, mut node: Node, at: (i64, i64)) -> Change {
    node.id = canvas.mint();
    (node.x, node.y) = at;
    Change::AddNode { node, index: None }
}

/// `node` as `parent`'s last child, beside it.
///
/// Changes place and do not lay out: a free canvas keeps every other node where
/// it was, and a mindmap lays out after.
pub fn child(canvas: &Canvas, parent: &str, mut node: Node) -> Option<Vec<Change>> {
    let at = canvas.node(parent)?;
    let [id, edge] = <[String; 2]>::try_from(canvas.mint_n(2)).ok()?;
    node.id = id;
    (node.x, node.y) = (at.x + at.width + GAP_X, at.y);
    let edge = branch_edge(edge, parent, &node.id);
    Some(vec![
        Change::AddNode { node, index: None },
        Change::AddEdge { edge, index: None },
    ])
}

/// `node` right after `of`, under the same parent.
pub fn sibling(canvas: &Canvas, of: &str, mut node: Node) -> Option<Vec<Change>> {
    let parent = parent(canvas, of)?;
    let index = canvas
        .edges
        .iter()
        .position(|edge| edge.to_node == of && is_branch(edge))?
        + 1;
    let below = canvas.node(of)?;
    let [id, edge] = <[String; 2]>::try_from(canvas.mint_n(2)).ok()?;
    node.id = id;
    (node.x, node.y) = (below.x, below.y + below.height + GAP_Y);
    let edge = branch_edge(edge, parent, &node.id);
    Some(vec![
        Change::AddNode { node, index: None },
        Change::AddEdge {
            edge,
            index: Some(index),
        },
    ])
}

/// What to select once `id` is removed: the previous sibling, else the next,
/// else the parent.
pub fn after_removal(canvas: &Canvas, id: &str) -> Option<String> {
    step(canvas, id, Toward::PrevSibling)
        .or_else(|| step(canvas, id, Toward::NextSibling))
        .or_else(|| step(canvas, id, Toward::Parent))
}

/// A node and its branch.
pub fn remove(canvas: &Canvas, id: &str) -> Change {
    Change::RemoveNodes {
        ids: branch_of(canvas, id),
    }
}

/// `id` and every node below it, top down.
pub fn branch_of(canvas: &Canvas, id: &str) -> Vec<String> {
    let mut ixs = Vec::new();
    if let Some(ix) = canvas.index_of(id) {
        branch(canvas, ix, &mut HashSet::new()).collect(&mut ixs);
    }
    ixs.into_iter()
        .map(|ix| canvas.nodes[ix].id.clone())
        .collect()
}

/// Hang each of `ids` under `parent` as its last child, cutting the branches
/// it had in. One that would make a cycle stays, and so does one below another
/// of `ids`, which it follows. `None` when none moves.
pub fn reparent(canvas: &Canvas, ids: &[String], parent: &str) -> Option<Vec<Change>> {
    canvas.node(parent)?;
    let moving: Vec<String> = ids
        .iter()
        .filter(|id| {
            let cycle = canvas
                .index_of(id)
                .is_none_or(|ix| branch_ids(canvas, ix).contains(parent));
            let follows = ids
                .iter()
                .any(|other| other != *id && descendants(canvas, other).contains(id.as_str()));
            !cycle && !follows
        })
        .cloned()
        .collect();
    if moving.is_empty() {
        return None;
    }
    let mut changes: Vec<Change> = detach(canvas, &moving).into_iter().collect();
    for (id, edge) in moving.iter().zip(canvas.mint_n(moving.len())) {
        changes.push(Change::AddEdge {
            edge: branch_edge(edge, parent, id),
            index: None,
        });
    }
    changes.extend(
        moving
            .iter()
            .filter_map(|id| canvas.node(id).and_then(unpin)),
    );
    Some(changes)
}

/// Cut the branches into each of `ids`, leaving it a root where it is; `None`
/// when all are.
pub fn detach(canvas: &Canvas, ids: &[String]) -> Option<Change> {
    let cut: Vec<String> = canvas
        .edges
        .iter()
        .filter(|edge| ids.contains(&edge.to_node) && is_branch(edge))
        .map(|edge| edge.id.clone())
        .collect();
    (!cut.is_empty()).then_some(Change::RemoveEdges { ids: cut })
}

/// The topmost node containing `at`, outside the branches of `except`. A group
/// is a frame, never a target.
pub fn node_at<'a>(canvas: &'a Canvas, at: (i64, i64), except: &[String]) -> Option<&'a str> {
    let skip: HashSet<String> = except
        .iter()
        .filter_map(|id| canvas.index_of(id))
        .flat_map(|ix| branch_ids(canvas, ix))
        .collect();
    canvas
        .nodes
        .iter()
        .rev()
        .find(|n| {
            !skip.contains(&n.id)
                && n.kind != GROUP
                && (n.x..n.x + n.width).contains(&at.0)
                && (n.y..n.y + n.height).contains(&at.1)
        })
        .map(|n| n.id.as_str())
}

/// Every node below `id`, however deep.
pub fn descendants(canvas: &Canvas, id: &str) -> HashSet<String> {
    let Some(ix) = canvas.index_of(id) else {
        return HashSet::new();
    };
    let mut below = branch_ids(canvas, ix);
    below.remove(id);
    below
}

fn branch_ids(canvas: &Canvas, ix: usize) -> HashSet<String> {
    let mut ixs = Vec::new();
    branch(canvas, ix, &mut HashSet::new()).collect(&mut ixs);
    ixs.into_iter()
        .map(|ix| canvas.nodes[ix].id.clone())
        .collect()
}

struct Branch {
    ix: usize,
    children: Vec<Branch>,
}

impl Branch {
    fn collect(&self, into: &mut Vec<usize>) {
        into.push(self.ix);
        for child in &self.children {
            child.collect(into);
        }
    }
}

fn branch(canvas: &Canvas, ix: usize, seen: &mut HashSet<usize>) -> Branch {
    seen.insert(ix);
    let mut kids = Vec::new();
    for to in children(canvas, &canvas.nodes[ix].id) {
        if let Some(kid) = canvas.index_of(to)
            && seen.insert(kid)
        {
            kids.push(kid);
        }
    }
    Branch {
        ix,
        children: kids
            .into_iter()
            .map(|kid| branch(canvas, kid, seen))
            .collect(),
    }
}

/// Who hangs under whom, looked up once for a whole layout.
struct Index<'a> {
    ix: HashMap<&'a str, usize>,
    kids: HashMap<&'a str, Vec<&'a str>>,
    parented: HashSet<&'a str>,
}

impl<'a> Index<'a> {
    fn of(canvas: &'a Canvas) -> Self {
        let ix = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(ix, node)| (node.id.as_str(), ix))
            .collect();
        let mut kids: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut parented = HashSet::new();
        for edge in canvas.edges.iter().filter(|edge| is_branch(edge)) {
            kids.entry(edge.from_node.as_str())
                .or_default()
                .push(edge.to_node.as_str());
            parented.insert(edge.to_node.as_str());
        }
        Self { ix, kids, parented }
    }

    /// [`branch`], from the index.
    fn branch(&self, canvas: &Canvas, ix: usize, seen: &mut HashSet<usize>) -> Branch {
        seen.insert(ix);
        let kids: Vec<usize> = self
            .kids
            .get(canvas.nodes[ix].id.as_str())
            .into_iter()
            .flatten()
            .filter_map(|to| self.ix.get(to).copied())
            .filter(|kid| seen.insert(*kid))
            .collect();
        Branch {
            ix,
            children: kids
                .into_iter()
                .map(|kid| self.branch(canvas, kid, seen))
                .collect(),
        }
    }
}

/// One layout's walk over the trees.
struct Pass<'a> {
    canvas: &'a Canvas,
    held: Option<&'a str>,
    /// The room each branch takes across the flow, measured once a tree.
    spans: HashMap<usize, i64>,
    moves: Vec<(String, (i64, i64))>,
}

impl<'a> Pass<'a> {
    /// Whether layout leaves a node where it is: pinned, or in a drag's hand.
    fn fixed(&self, node: &Node) -> bool {
        is_pinned(node) || self.held == Some(node.id.as_str())
    }

    /// Measure `branch` and all below it: its node, or its children stacked,
    /// whichever takes more room across the flow.
    fn measure(&mut self, branch: &Branch, down: bool) -> i64 {
        let canvas = self.canvas;
        let mut stacked = Vec::new();
        for child in &branch.children {
            let span = self.measure(child, down);
            if !self.fixed(&canvas.nodes[child.ix]) {
                stacked.push(span);
            }
        }
        let node = &canvas.nodes[branch.ix];
        let across = if down { node.width } else { node.height };
        let gaps = GAP_Y * (stacked.len() as i64 - 1).max(0);
        let span = across.max(stacked.iter().sum::<i64>() + gaps);
        self.spans.insert(branch.ix, span);
        span
    }

    fn span(&self, branch: &Branch) -> i64 {
        self.spans[&branch.ix]
    }

    /// The children still in the column: a fixed one left it.
    fn stack<'b>(&self, branches: impl IntoIterator<Item = &'b Branch>) -> i64 {
        let spans: Vec<i64> = branches
            .into_iter()
            .filter(|b| !self.fixed(&self.canvas.nodes[b.ix]))
            .map(|b| self.span(b))
            .collect();
        spans.iter().sum::<i64>() + GAP_Y * (spans.len() as i64 - 1).max(0)
    }

    /// A balanced root's children, right and left, each to the side with less
    /// room taken so far.
    fn split<'b>(&self, kids: &[&'b Branch]) -> (Vec<&'b Branch>, Vec<&'b Branch>) {
        let (mut right, mut left) = ((0, Vec::new()), (0, Vec::new()));
        for &kid in kids {
            let side = if right.0 <= left.0 {
                &mut right
            } else {
                &mut left
            };
            side.0 += self.span(kid);
            side.1.push(kid);
        }
        (right.1, left.1)
    }

    /// `kids` beside a parent sitting at `at`, growing in `flow`.
    fn column(&mut self, parent: &Node, at: (i64, i64), kids: &[&Branch], flow: Flow) {
        let canvas = self.canvas;
        let center = match flow {
            Flow::Down => at.0 + parent.width / 2,
            _ => at.1 + parent.height / 2,
        };
        let mut top = center - self.stack(kids.iter().copied()) / 2;
        for &kid in kids {
            let node = &canvas.nodes[kid.ix];
            if self.fixed(node) {
                self.place(kid, (node.x, node.y), flow);
                continue;
            }
            let span = self.span(kid);
            let mid = top + span / 2;
            let to = match flow {
                Flow::Down => (mid - node.width / 2, at.1 + parent.height + GAP_X),
                Flow::Left => (at.0 - GAP_X - node.width, mid - node.height / 2),
                Flow::Right | Flow::Both => (at.0 + parent.width + GAP_X, mid - node.height / 2),
            };
            self.place(kid, to, flow);
            top += span + GAP_Y;
        }
    }

    fn place(&mut self, branch: &Branch, to: (i64, i64), flow: Flow) {
        let canvas = self.canvas;
        let node = &canvas.nodes[branch.ix];
        if (node.x, node.y) != to {
            self.moves.push((node.id.clone(), to));
        }
        let kids: Vec<&Branch> = branch.children.iter().collect();
        self.column(node, to, &kids, flow);
    }
}

/// An edge that hangs `to` under `from`.
pub fn branch_edge(id: String, from: &str, to: &str) -> Edge {
    Edge {
        from_side: Some(Side::Right),
        to_side: Some(Side::Left),
        // The spec's default end is an arrow; a branch is not a direction.
        to_end: Some(End::None),
        ..Edge::new(id, from, to)
    }
}
