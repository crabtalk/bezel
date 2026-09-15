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
    Change::Remove { id } if is_running(id, cx) => None,   // refuse
    _ => Some(change),                                     // or rewrite it
})
```

Every edit — a key, a drop, typing in a node — is a `Change`: `Add`, `Move`, `Reparent`, `Detach`, `Remove`, `Unpin` or `Update`. The filter answers what lands, and `CanvasEvent::Changed` carries what did. The view's own `Resize` (a measured height) and `Layout` are announced too but never filtered, so saving on `Changed` misses nothing. It runs inside the view's update, so reach the view through `cx.defer`; `view.apply(change, cx)` lands one of the app's own past the filter.

## Dragging a node

```rust
CanvasView::new(doc, cx).with_drag(canvas::drag::reparent);

fn my_drag(canvas: &Canvas, drag: &Drag) -> Vec<Change> {
    // drag.id, drag.to(), drag.over, drag.phase (Move…, then one Drop)
}
```

On a move, the handler's `Move`s are applied as they come and the rest are drawn as what the drop would do: a ring on the new parent, the connector it would make, faded connectors it would cut. The drop's changes go through the filter, with the node put back first, so a refused drop leaves it where it was. Nodes a layout moves glide there. `drag::pin` (the default) leaves the node where it lands and marks it `"pinned": true`; `drag::reparent` hangs it under the node it is dropped on; `drag::detach` cuts its edges in.

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
    pub fn submit(&mut self, change: Change, cx: &mut Context<Self>) -> bool;
    /// Past the filter.
    pub fn apply(&mut self, change: Change, cx: &mut Context<Self>);
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
pub fn child(canvas: &Canvas, parent: &str, node: Node) -> Option<Change>;
pub fn sibling(canvas: &Canvas, of: &str, node: Node) -> Option<Change>;
pub fn root(canvas: &Canvas, node: Node, at: (i64, i64)) -> Change;

// canvas::change — pure
pub fn apply(canvas: &mut Canvas, change: &Change);
```

The source is at `apps/gallery/src/patterns/canvas.rs`. Copy the file.
