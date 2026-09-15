---
title: Canvas
description: A mindmap over JSON Canvas — tab, enter and f2 build it, and an app's own node kinds paint through one renderer.
---

```rust
editor::init(cx);
canvas::init(cx);                                  // after editor::init
canvas::set_node_renderer(cx, my_nodes);           // optional

let view = cx.new(|cx| CanvasView::new(Canvas::parse(json)?, cx));
cx.subscribe(&view, |_, view, event, cx| {
    if *event == CanvasEvent::Changed {
        save(view.read(cx).canvas().to_json());
    }
}).detach();
```

The document is [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/). Fields the spec does not name are kept and written back, so a file another app wrote survives a save.

## Your own nodes

```rust
fn my_nodes(node: &Node, zoom: f32, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match node.kind.as_str() {
        "session" => Some(sessions(cx).view(&node.id)?.into_any_element()),
        _ => None,                                 // the built-in painting
    }
}
```

State stays with the app: look the view up by the node's id. gpui cannot transform an element, so `zoom` is a scale the content applies itself — `canvas::text_style(div(), TextStyle::Callout, zoom)` sets size, leading and weight together, and `Typography::scaled` does a whole document. A node of fixed size clips what its renderer paints.

## Keys

## Dragging a node

```rust
cx.new(|cx| CanvasView::new(doc, cx).with_drag(canvas::drag::reparent));

fn my_drop(canvas: &mut Canvas, drag: &canvas::drag::Drag) {
    // drag.id, drag.to(), drag.over, drag.phase (Move, then one Drop)
}
```

What a drop does is the app's, per view. `drag::pin` (the default) leaves the node where it lands and marks it `"pinned": true`; `drag::reparent` hangs it under the node it is dropped on; `drag::detach` cuts its edges in. While held, the node stays where the handler put it and its branch follows.

`tab` adds a child (under the first root when nothing is selected), `enter` a sibling, `backspace` removes a branch, `f2` or a double-click edits, arrows walk the tree, `escape` leaves a node. `cmd-=`, `cmd--` and `cmd-0` zoom; a pinch or a `cmd`-wheel zooms at the pointer, and a drag or a wheel pans.

## API

```rust
impl CanvasView {
    pub fn new(canvas: Canvas, cx: &mut Context<Self>) -> Self;
    /// `Arrange::Free` keeps positions as the document has them.
    pub fn with_arrange(self, arrange: Arrange) -> Self;
    pub fn canvas(&self) -> &Canvas;
    pub fn set_canvas(&mut self, canvas: Canvas, cx: &mut Context<Self>);
    pub fn selected(&self) -> Option<&str>;
    pub fn select(&mut self, id: Option<String>, cx: &mut Context<Self>);
    pub fn zoom(&self) -> f32;
    pub fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>);
}

// canvas::mindmap — pure, over a Canvas
pub fn layout(canvas: &mut Canvas);
pub fn add_child(canvas: &mut Canvas, parent: &str) -> Option<String>;
pub fn add_sibling(canvas: &mut Canvas, of: &str) -> Option<String>;
pub fn remove(canvas: &mut Canvas, id: &str) -> Option<String>;
```

The source is at `apps/gallery/src/patterns/canvas.rs`. Copy the file.
