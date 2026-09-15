---
title: Canvas
description: A mindmap over JSON Canvas — an app's own node kinds, and every edit handed to the app as a change it can refuse or rewrite.
---

```rust
editor::init(cx);
canvas::init(cx);                                            // after editor::init
canvas::set_kinds(cx, Kinds::new().with("session", SESSION)); // optional

let view = cx.new(|cx| CanvasView::new(Canvas::parse(json)?, cx));
cx.subscribe(&view, |_, view, event, cx| {
    if let CanvasEvent::Changed(_) = event {
        save(view.read(cx).canvas().to_json());
    }
}).detach();
```

The document is [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/). Fields the spec does not name are kept and written back, so a file another app wrote survives a save.

## Your own kinds

```rust
const SESSION: Kind = Kind {
    render: |node, zoom, window, cx| sessions(cx).view(&node.id).into_any_element(),
    sizing: Sizing::Fixed,             // or Grows: as tall as the content
    chrome: Chrome::Card,              // Outline, or Bare: the content is the node
    edit: Some(Field { read: title, write: set_title }),  // what f2 edits
    child: kind::blank,                // what tab makes under it
};
```

A kind is keyed by the node's `type`. The spec's four — `kind::TEXT`, `FILE`, `LINK`, `GROUP` — are replaceable the same way, or a starting point: `Kind { chrome: Chrome::Bare, ..kind::TEXT }`.

State stays with the app: look the view up by the node's id. gpui cannot transform an element, so `zoom` is a scale the content applies itself — `canvas::text_style(div(), TextStyle::Callout, zoom)` sets size, leading and weight together, and `Typography::scaled` does a whole document. A node of fixed size clips what it paints.

## Changes

```rust
CanvasView::new(doc, cx).with_changes(|canvas, change, cx| match &change {
    Change::RemoveNodes { ids } if ids.iter().any(|id| is_running(id, cx)) => None, // refuse
    _ => Some(change),                                                              // or rewrite it
})
```

Every edit — a key, a drop, typing in a node — is a batch of graph changes: `AddNode`, `AddEdge`, `RemoveNodes`, `RemoveEdges`, `MoveNodes`, `Resize`, `UpdateNode`, `UpdateEdge`. Each goes through the filter, and one refused refuses the batch. `CanvasEvent::Changed` carries each batch that landed, the view's own measured heights and layout included, so saving on it misses nothing. The filter runs inside the view's update, so reach the view through `cx.defer`; `view.apply(changes, cx)` lands the app's own past it.

Tree edits live in `canvas::mindmap` and answer the batch they make: `child`, `sibling`, `remove` (a branch), `reparent`, `detach`, `carry`. An edge with `"tree": false` is a cross link, never a branch.

## Layouts

```rust
CanvasView::new(doc, cx).with_layout(canvas::layout::DOWN);

const RADIAL: Layout = Layout { arrange: my_arrange, flow: None };
```

A `Layout` answers where nodes go after every change. `layout::MINDMAP` (the default) grows trees right, `BALANCED` splits a root's branches both ways, `DOWN` grows them down, and `FREE` leaves nodes where they are put. A layout with a `flow` grows trees: arrows walk them that way and `backspace` takes a branch. Without one, arrows go to the nearest node and `backspace` takes one node.

## Dragging a node

```rust
CanvasView::new(doc, cx).with_drag(canvas::drag::reparent);

fn my_drag(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    // drag.id, drag.with (the rest of the selection), drag.to(), drag.over, drag.phase (Move…, then one Drop)
}
```

On a move, the handler's `MoveNodes` are applied as they come and the rest are drawn as what the drop would do: a ring on a node an `AddEdge` reaches, the connector it would make, faded connectors a `RemoveEdges` would cut. The drop is one batch through the filter, with the preview put back first, so a refused drop leaves everything where it was. Nodes a layout moves glide there. `drag::pin` (the default) leaves the node where it lands and marks it `"pinned": true`; `drag::reparent` hangs it under the node it is dropped on; `drag::detach` cuts its edges in.

## Selection, undo and the clipboard

Shift- or cmd-click adds to the selection, a shift-drag on empty canvas selects what the box touches, `cmd-a` selects all and `escape` none. The keys act from the primary — the last chosen — and removing, nudging, dragging, copying and duplicating take the whole selection, each node with its branch under a tree layout.

`cmd-z` and `cmd-shift-z` undo and redo what landed through `submit` or `apply`; an add and the typing into it are one step. Copy writes JSON Canvas; paste mints fresh ids and lands under the selection in a tree, or in the middle of the view, and plain text pastes as a text node. `cmd-d` duplicates.

## Edges and boxes

Click an edge to pick it: `backspace` removes it, and a double-click or `f2` edits its label. A picked node shows a handle on each side — drag one onto a node to connect them, or onto nothing to make a node there, a child under a tree — and a corner that resizes it, the width alone for a box that grows. A connector drawn under a tree layout is a cross link.

## Keys

`tab` adds a child (under the first root when nothing is selected), `enter` a sibling, `backspace` removes, `f2` or a double-click edits, arrows move the selection, `shift`-arrows nudge it, `escape` leaves a node. A double-click on nothing adds a node there. `cmd-=`, `cmd--` and `cmd-0` zoom; a pinch or a `cmd`-wheel zooms at the pointer, and a drag, a middle-button drag or a wheel pans.

## API

```rust
impl CanvasView {
    pub fn new(canvas: Canvas, cx: &mut Context<Self>) -> Self;
    pub fn with_layout(self, layout: Layout) -> Self;
    pub fn with_drag(self, handler: DragHandler) -> Self;
    pub fn with_changes(self, filter: impl Fn(&Canvas, Change, &mut App) -> Option<Change> + 'static) -> Self;
    pub fn canvas(&self) -> &Canvas;
    pub fn set_canvas(&mut self, canvas: Canvas, cx: &mut Context<Self>);
    /// Through the filter, as if the reader made it. A toolbar's way in.
    pub fn submit(&mut self, changes: impl IntoIterator<Item = Change>, cx: &mut Context<Self>) -> bool;
    /// Past the filter.
    pub fn apply(&mut self, changes: impl IntoIterator<Item = Change>, cx: &mut Context<Self>);
    /// The primary selection.
    pub fn selected(&self) -> Option<&str>;
    /// The whole selection, the primary last.
    pub fn selection(&self) -> &[String];
    pub fn select(&mut self, id: Option<String>, cx: &mut Context<Self>);
    pub fn set_selection(&mut self, ids: Vec<String>, cx: &mut Context<Self>);
    /// An edge is picked apart from nodes: picking one lets the others go.
    pub fn selected_edge(&self) -> Option<&str>;
    pub fn select_edge(&mut self, id: Option<String>, cx: &mut Context<Self>);
    pub fn select_all(&mut self, cx: &mut Context<Self>);
    /// What `backspace` does.
    pub fn remove_selected(&mut self, cx: &mut Context<Self>);
    pub fn undo(&mut self, cx: &mut Context<Self>) -> bool;
    pub fn redo(&mut self, cx: &mut Context<Self>) -> bool;
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn copy(&self, cx: &mut App);
    pub fn cut(&mut self, cx: &mut Context<Self>);
    pub fn paste(&mut self, cx: &mut Context<Self>);
    pub fn duplicate(&mut self, cx: &mut Context<Self>);
    pub fn zoom(&self) -> f32;
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>);
    /// What `cmd-=` and `cmd--` do.
    pub fn zoom_in(&mut self, cx: &mut Context<Self>);
    pub fn zoom_out(&mut self, cx: &mut Context<Self>);
    /// Centre the document again, as it opened.
    pub fn fit(&mut self, cx: &mut Context<Self>);
    /// The canvas point under the middle of the view.
    pub fn center(&self) -> (i64, i64);
    /// Switching to a tree that grows another way drops every pin.
    pub fn set_layout(&mut self, layout: Layout, cx: &mut Context<Self>);
    pub fn set_drag(&mut self, handler: DragHandler);
}

// canvas::mindmap — pure; the builders answer a Change to submit
pub fn layout(canvas: &mut Canvas);
/// Where layout would move each node, those already there left out.
pub fn arrange(canvas: &Canvas, held: Option<&str>, flow: Flow) -> Vec<(String, (i64, i64))>;
pub fn walk(canvas: &Canvas, id: &str, flow: Flow, arrow: Arrow) -> Option<String>;
pub fn child(canvas: &Canvas, parent: &str, node: Node) -> Option<Vec<Change>>;
pub fn sibling(canvas: &Canvas, of: &str, node: Node) -> Option<Vec<Change>>;
pub fn root(canvas: &Canvas, node: Node, at: (i64, i64)) -> Change;
pub fn remove(canvas: &Canvas, id: &str) -> Change;
pub fn reparent(canvas: &Canvas, ids: &[String], parent: &str) -> Option<Vec<Change>>;
pub fn detach(canvas: &Canvas, ids: &[String]) -> Option<Change>;
pub fn carry(canvas: &Canvas, ids: &[String], by: (i64, i64), pin: bool) -> Vec<Change>;

// canvas::clip — pure
pub fn fragment(canvas: &Canvas, ids: &[String]) -> Canvas;
pub fn paste(canvas: &Canvas, fragment: &Canvas, at: (i64, i64), under: Option<&str>) -> Vec<Change>;

// canvas::change — pure
/// Answers the changes that undo it.
pub fn apply(canvas: &mut Canvas, change: &Change) -> Vec<Change>;
pub fn apply_all(canvas: &mut Canvas, changes: &[Change]) -> Vec<Change>;
/// The first node a batch adds.
pub fn added(changes: &[Change]) -> Option<&str>;
```

The source is at `apps/gallery/src/patterns/canvas.rs`. Copy the file.
