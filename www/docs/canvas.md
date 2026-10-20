---
title: Canvas
description: A mindmap over JSON Canvas — an app's own node kinds, and every edit handed to the app as a change it can refuse or rewrite.
prompt: Add a mindmap canvas that saves to JSON Canvas on every change, with my own "session" node kind that shows a session's title.
---

```rust
editor::init(cx);
canvas::init(cx);                                            // after editor::init

let view = cx.new(|cx| {
    CanvasView::new(Canvas::parse(json)?, canvas::layout::MINDMAP, cx)
        .with_kinds(Kinds::new().with("session", session(store)))  // optional
});
cx.subscribe(&view, |_, view, event, cx| {
    if let CanvasEvent::Changed(_) = event {
        save(view.read(cx).editor().canvas().to_json());
    }
}).detach();
```

- The document is [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/). Fields the spec does not name survive a save.
- Every canvas names a layout; there is no default.
- gpui cannot transform an element, so a kind applies `look.zoom` to what it paints. A node of fixed size clips it.
- `CanvasEditor` is the canvas without a window. Headless, call `editor.take_events()` regularly.

## Make it yours

| call | what it decides |
| --- | --- |
| `with_kinds`, `Kind` | how each node `type` paints, sizes, edits and opens, and whether it holds others |
| `with_edge_kinds`, `EdgeKind` | how an edge runs, paints, is picked and labelled |
| `layout::{MINDMAP, BALANCED, DOWN, FREE}`, `Layout` | where nodes go, and what arrows, `tab` and `backspace` mean |
| `with_changes` | refusing or rewriting any edit before it lands |
| `with_drag`, `drag::*` | what dropping a node does |
| `with_tools`, `Tool` | the gestures, in the order a press is offered to them |
| `with_snap` | a grid and alignment guides |
| `with_options`, `with_style`, `with_overlays` | tuning, and everything the canvas paints beside the kinds |
| `Capabilities` | what a node or edge lets the reader do |
| `canvas::minimap` | a map of the whole canvas, placed by the app |

## Keys

| key | does |
| --- | --- |
| `tab` / `enter` | add a child / a sibling |
| `backspace` | remove the selection (a branch, under a tree) |
| `f2`, double-click | edit a node or an edge label |
| arrows / `shift`-arrows | move the selection / nudge it |
| `cmd-z` / `cmd-shift-z` | undo / redo |
| `cmd-c`, `cmd-v`, `cmd-d`, `cmd-a` | copy, paste, duplicate, select all |
| `cmd-=` / `cmd--` / `cmd-0` | zoom in / out / reset |
| `shift-1` / `shift-2` | fit everything / the selection |
| `escape` | leave a node, drop the gesture in hand |
