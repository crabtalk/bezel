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

- `Document::reload` loads external changes; `DocumentChange::Reloaded` distinguishes them from edits that should be saved.
- Multiple panes share an `Entity<Document>` through `CanvasView::for_document`; selection and viewport stay with each view.
- With `default-features = false`, the crate provides the model, changes, layouts and JSON Canvas import/export without GPUI.
- The document is [JSON Canvas 1.0](https://jsoncanvas.org/spec/1.0/). Fields the spec does not name survive a save.
- `Options::read_only` blocks user edits and history commands while keeping navigation and copying available. `CanvasEditor` owns grouping, selection colors, connection ends, and adding supplied nodes.
- Hosts use `canvas_point`, `item_at`, `edit_selected`, and `CanvasEvent::ContextMenu` for their own controls. Enable `Options::external_drops` for `FilesDropped` events.
- With `Options::choose_connection_node`, a loose connector emits `ConnectionRequested`; complete it with `CanvasEditor::extend_connection`.
- `CanvasEditor::arrange` aligns, distributes, lays out rows/columns/grids, or justifies a selection in one undo step.
- Use `handle::frame` for resizing from every border and corner; `handle::sides_and_corner` keeps the compact mindmap handles.
- Drag with Shift to constrain movement or preserve resize proportions, Space to bypass snapping, and Alt to duplicate. Space-drag pans over cards.
- Use `kind::text().sizing(kind::Sizing::Fixed)` for saved card dimensions and scrolling while selected; the default grows with its content.
- Every canvas names a layout; there is no default.
- gpui cannot transform an element, so a kind applies `look.card.zoom` to what it paints. A node of fixed size clips it.
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

Group backgrounds support JSON Canvas `cover`, `ratio`, and `repeat`. Repeated
images retain their logical tile size as the canvas zooms.

`CanvasView::export_image(export::Area::Canvas, scale, cx)` returns a task with
an RGBA image; `Area::Viewport` keeps the current pan and zoom. Custom node
renderers opt in with `Kind::exportable()` when they paint entirely through
GPUI. Native content is refused. The host owns encoding and its save dialog.

`presentation::typography(cx, zoom)` supplies canvas document sizes. Content
scales with zoom without switching to summaries or hiding connection labels.
`Look::card.render` lays out a card's header and content. Fixed cards scroll
when selected; growing cards measure their content. Short fixed cards reduce
vertical padding.

Custom renderers can use `Look::card.measure.wrap` to report their natural
content height, including padding in canvas units. `CanvasView::fit_to_content`
applies measured heights to selected nodes as one undo step, preserving width.

Node drags follow the pointer between grid lines and settle to the grid on
release. Object guides remain active during the drag. Selections and group
contents move together without layout animation while the gesture is held.
