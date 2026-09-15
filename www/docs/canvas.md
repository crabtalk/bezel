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

Tree edits live in `canvas::mindmap` and answer the batch they make: `child`, `sibling`, `remove` (a branch), `reparent`, `detach`, `carry`. An edge with `"tree": false` is a cross link, never a branch. `backspace` removes a branch under `Arrange::Mindmap` and only the node under `Arrange::Free`.

## Dragging a node

```rust
CanvasView::new(doc, cx).with_drag(canvas::drag::reparent);

fn my_drag(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    // drag.id, drag.to(), drag.over, drag.phase (Move…, then one Drop)
}
```

On a move, the handler's `MoveNodes` are applied as they come and the rest are drawn as what the drop would do: a ring on a node an `AddEdge` reaches, the connector it would make, faded connectors a `RemoveEdges` would cut. The drop is one batch through the filter, with the preview put back first, so a refused drop leaves everything where it was. Nodes a layout moves glide there. `drag::pin` (the default) leaves the node where it lands and marks it `"pinned": true`; `drag::reparent` hangs it under the node it is dropped on; `drag::detach` cuts its edges in.

## Keys

`tab` adds a child (under the first root when nothing is selected), `enter` a sibling, `backspace` removes a branch, `f2` or a double-click edits, arrows walk the tree, `escape` leaves a node. `cmd-=`, `cmd--` and `cmd-0` zoom; a pinch or a `cmd`-wheel zooms at the pointer, and a drag or a wheel pans.

## API

```rust
impl CanvasView {
    pub fn new(canvas: Canvas, cx: &mut Context<Self>) -> Self;
    /// `Arrange::Free` keeps positions as the document has them.
    pub fn with_arrange(self, arrange: Arrange) -> Self;
    pub fn with_drag(self, handler: DragHandler) -> Self;
    pub fn with_changes(self, filter: impl Fn(&Canvas, Change, &mut App) -> Option<Change> + 'static) -> Self;
    pub fn canvas(&self) -> &Canvas;
    pub fn set_canvas(&mut self, canvas: Canvas, cx: &mut Context<Self>);
    /// Through the filter, as if the reader made it. A toolbar's way in.
    pub fn submit(&mut self, changes: impl IntoIterator<Item = Change>, cx: &mut Context<Self>) -> bool;
    /// Past the filter.
    pub fn apply(&mut self, changes: impl IntoIterator<Item = Change>, cx: &mut Context<Self>);
    pub fn selected(&self) -> Option<&str>;
    pub fn select(&mut self, id: Option<String>, cx: &mut Context<Self>);
    /// What `backspace` does.
    pub fn remove_selected(&mut self, cx: &mut Context<Self>);
    pub fn zoom(&self) -> f32;
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>);
    /// What `cmd-=` and `cmd--` do.
    pub fn zoom_in(&mut self, cx: &mut Context<Self>);
    pub fn zoom_out(&mut self, cx: &mut Context<Self>);
    /// Centre the document again, as it opened.
    pub fn fit(&mut self, cx: &mut Context<Self>);
    /// The canvas point under the middle of the view.
    pub fn center(&self) -> (i64, i64);
    /// Turning `Arrange::Mindmap` on drops every pin and lays the trees out again.
    pub fn set_arrange(&mut self, arrange: Arrange, cx: &mut Context<Self>);
    pub fn set_drag(&mut self, handler: DragHandler);
}

// canvas::mindmap — pure; the builders answer a Change to submit
pub fn layout(canvas: &mut Canvas);
/// Where layout would move each node, those already there left out.
pub fn arrange(canvas: &Canvas, held: Option<&str>) -> Vec<(String, (i64, i64))>;
pub fn child(canvas: &Canvas, parent: &str, node: Node) -> Option<Vec<Change>>;
pub fn sibling(canvas: &Canvas, of: &str, node: Node) -> Option<Vec<Change>>;
pub fn root(canvas: &Canvas, node: Node, at: (i64, i64)) -> Change;
pub fn remove(canvas: &Canvas, id: &str) -> Change;
pub fn reparent(canvas: &Canvas, id: &str, parent: &str) -> Option<Vec<Change>>;
pub fn detach(canvas: &Canvas, id: &str) -> Option<Change>;
pub fn carry(canvas: &Canvas, id: &str, to: (i64, i64), pin: bool) -> Vec<Change>;

// canvas::change — pure
pub fn apply(canvas: &mut Canvas, change: &Change);
/// The first node a batch adds.
pub fn added(changes: &[Change]) -> Option<&str>;
```

The source is at `apps/gallery/src/patterns/canvas.rs`. Copy the file.
