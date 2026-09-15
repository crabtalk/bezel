//! A mindmap is a canvas whose edges form a tree: a node's children are the
//! edges leaving it, in edge order.
//!
//! Layout writes positions back into the [`Canvas`], so the file stays plain
//! JSON Canvas that any other viewer opens as it was last laid out.

use std::collections::HashSet;

use serde_json::Value;

use crate::{
    change::{self, Change},
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

pub fn is_pinned(node: &Node) -> bool {
    node.extra
        .get(PINNED)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Put a node where it was dropped and keep it there. Its branch follows it.
pub fn pin(canvas: &mut Canvas, id: &str, x: i64, y: i64) -> Option<()> {
    let node = canvas.node_mut(id)?;
    node.x = x;
    node.y = y;
    node.extra.insert(PINNED.into(), Value::Bool(true));
    Some(())
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
        .find(|edge| edge.to_node == id)
        .map(|edge| edge.from_node.as_str())
}

pub fn children<'a>(canvas: &'a Canvas, id: &'a str) -> impl Iterator<Item = &'a str> {
    canvas
        .edges
        .iter()
        .filter(move |edge| edge.from_node == id)
        .map(|edge| edge.to_node.as_str())
}

/// Nodes nothing points at. Groups are frames, not branches.
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

/// Lay every tree out to the right of its root, which stays where it is.
///
/// A node reached twice hangs off the first parent that reaches it, so a stray
/// cycle or a cross link cannot loop the layout.
pub fn layout(canvas: &mut Canvas) {
    layout_holding(canvas, None);
}

/// [`layout`], keeping `held` where it is as if pinned — the node a drag has
/// in hand.
pub fn layout_holding(canvas: &mut Canvas, held: Option<&str>) {
    let moves = arrange(canvas, held);
    change::apply(canvas, &Change::Layout { moves });
}

/// Where [`layout_holding`] would put each node, leaving out those already
/// there.
pub fn arrange(canvas: &Canvas, held: Option<&str>) -> Vec<(String, (i64, i64))> {
    let mut seen = HashSet::new();
    let mut moves = Vec::new();
    for root in roots(canvas) {
        let Some(ix) = canvas.index_of(&root.id) else {
            continue;
        };
        let tree = branch(canvas, ix, &mut seen);
        let center = root.y + root.height / 2;
        place(canvas, &tree, root.x, center, held, &mut moves);
    }
    moves
}

/// `node` as a root at `at`, under an id nothing holds.
pub fn root(canvas: &Canvas, mut node: Node, at: (i64, i64)) -> Change {
    node.id = canvas.mint();
    (node.x, node.y) = at;
    Change::Add {
        node,
        edge: None,
        index: None,
    }
}

/// `node` as `parent`'s last child, beside it.
///
/// Changes place and do not lay out: a free canvas keeps every other node where
/// it was, and a mindmap lays out after.
pub fn child(canvas: &Canvas, parent: &str, mut node: Node) -> Option<Change> {
    let at = canvas.node(parent)?;
    let [id, edge] = <[String; 2]>::try_from(canvas.mint_n(2)).ok()?;
    node.id = id;
    (node.x, node.y) = (at.x + at.width + GAP_X, at.y);
    let edge = tree_edge(edge, parent, &node.id);
    Some(Change::Add {
        node,
        edge: Some(edge),
        index: None,
    })
}

/// `node` right after `of`, under the same parent.
pub fn sibling(canvas: &Canvas, of: &str, mut node: Node) -> Option<Change> {
    let parent = parent(canvas, of)?;
    let index = canvas.edges.iter().position(|edge| edge.to_node == of)? + 1;
    let below = canvas.node(of)?;
    let [id, edge] = <[String; 2]>::try_from(canvas.mint_n(2)).ok()?;
    node.id = id;
    (node.x, node.y) = (below.x, below.y + below.height + GAP_Y);
    let edge = tree_edge(edge, parent, &node.id);
    Some(Change::Add {
        node,
        edge: Some(edge),
        index: Some(index),
    })
}

/// What to select once `id` is removed: the previous sibling, else the next,
/// else the parent.
pub fn after_removal(canvas: &Canvas, id: &str) -> Option<String> {
    step(canvas, id, Toward::PrevSibling)
        .or_else(|| step(canvas, id, Toward::NextSibling))
        .or_else(|| step(canvas, id, Toward::Parent))
}

/// Remove a node and its branch. Answers what to select next: the previous
/// sibling, else the next, else the parent.
pub fn remove(canvas: &mut Canvas, id: &str) -> Option<String> {
    let root = canvas.index_of(id)?;
    let next = after_removal(canvas, id);
    let mut seen = HashSet::new();
    let mut doomed = Vec::new();
    branch(canvas, root, &mut seen).collect(&mut doomed);
    let doomed: HashSet<String> = doomed
        .into_iter()
        .map(|ix| canvas.nodes[ix].id.clone())
        .collect();
    canvas.nodes.retain(|node| !doomed.contains(&node.id));
    canvas
        .edges
        .retain(|edge| !doomed.contains(&edge.from_node) && !doomed.contains(&edge.to_node));
    next
}

/// Hang `id` under `parent` as its last child, cutting the edges it had in.
/// Refused onto itself or its own branch, which would be a cycle.
pub fn reparent(canvas: &mut Canvas, id: &str, parent: &str) -> bool {
    let (Some(ix), Some(_)) = (canvas.index_of(id), canvas.node(parent)) else {
        return false;
    };
    if branch_ids(canvas, ix).contains(parent) {
        return false;
    }
    detach(canvas, id);
    if let Some(node) = canvas.node_mut(id) {
        node.extra.remove(PINNED);
    }
    let edge = canvas.mint();
    canvas.edges.push(tree_edge(edge, parent, id));
    true
}

/// Cut the edges into `id`, leaving it a root where it is.
pub fn detach(canvas: &mut Canvas, id: &str) {
    canvas.edges.retain(|edge| edge.to_node != id);
}

/// The topmost node containing `at`, outside `except`'s branch.
pub fn node_at<'a>(canvas: &'a Canvas, at: (i64, i64), except: &str) -> Option<&'a str> {
    let skip = canvas
        .index_of(except)
        .map(|ix| branch_ids(canvas, ix))
        .unwrap_or_default();
    canvas
        .nodes
        .iter()
        .rev()
        .find(|n| {
            !skip.contains(&n.id)
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

/// The height a branch takes: its node, or its children stacked, whichever is
/// taller.
fn span(canvas: &Canvas, branch: &Branch, held: Option<&str>) -> i64 {
    canvas.nodes[branch.ix]
        .height
        .max(stack(canvas, &branch.children, held))
}

/// Whether layout leaves a node where it is: pinned, or in a drag's hand.
fn fixed(node: &Node, held: Option<&str>) -> bool {
    is_pinned(node) || held == Some(node.id.as_str())
}

/// The children still in the column: a fixed one left it.
fn stack(canvas: &Canvas, branches: &[Branch], held: Option<&str>) -> i64 {
    let flowing: Vec<&Branch> = branches
        .iter()
        .filter(|b| !fixed(&canvas.nodes[b.ix], held))
        .collect();
    let gaps = GAP_Y * (flowing.len() as i64 - 1).max(0);
    flowing.iter().map(|b| span(canvas, b, held)).sum::<i64>() + gaps
}

fn place(
    canvas: &Canvas,
    branch: &Branch,
    x: i64,
    center: i64,
    held: Option<&str>,
    moves: &mut Vec<(String, (i64, i64))>,
) {
    let node = &canvas.nodes[branch.ix];
    let y = center - node.height / 2;
    if (node.x, node.y) != (x, y) {
        moves.push((node.id.clone(), (x, y)));
    }
    let column = x + node.width + GAP_X;
    let mut top = center - stack(canvas, &branch.children, held) / 2;
    for child in &branch.children {
        let kid = &canvas.nodes[child.ix];
        if fixed(kid, held) {
            let (x, center) = (kid.x, kid.y + kid.height / 2);
            place(canvas, child, x, center, held, moves);
            continue;
        }
        let span = span(canvas, child, held);
        place(canvas, child, column, top + span / 2, held, moves);
        top += span + GAP_Y;
    }
}

fn tree_edge(id: String, from: &str, to: &str) -> Edge {
    Edge {
        from_side: Some(Side::Right),
        to_side: Some(Side::Left),
        // The spec's default end is an arrow; a branch is not a direction.
        to_end: Some(End::None),
        ..Edge::new(id, from, to)
    }
}
