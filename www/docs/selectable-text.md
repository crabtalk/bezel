---
title: Selectable text
description: Prose a reader can drag over and copy, without an editor behind it — the gesture, and nothing else.
---

```rust
use markdown::selectable::{self, Pointer};

selectable::render(
    "message",
    &doc,
    &self.layouts,
    self.selection,
    self.dragging,
    window,
    cx,
    |view, pointer, cx| match pointer {
        Pointer::Down(cursor) => { view.selection = Some(Selection::at(cursor)); view.dragging = true }
        Pointer::Move(cursor) => view.selection = view.selection.map(|s| s.extend_to(cursor)),
        Pointer::Up => view.dragging = false,
    },
)
```

`dragging` is yours because a move must only extend a selection a press started — otherwise the pointer drags one just by crossing the text.

## Copying

```rust
markdown::selectable::copied(&doc, selection)
```

Plain text, with a newline where each block ended. Not the editor's copy, which is `slice` then `serialize` and keeps the markup: a reader who selected the word inside `**bold**` selected the word.

## API

```rust
// markdown::selectable

/// The release is answered twice, on the text and off it — a drag ending past
/// the edge of a paragraph is the ordinary way to select to the end of one.
pub fn render<V: 'static>(
    id: impl Into<ElementId>,
    doc: &Doc,
    layouts: &BlockLayouts,
    selection: Option<Selection>,
    dragging: bool,
    window: &mut Window,
    cx: &mut Context<V>,
    on_pointer: impl Fn(&mut V, Pointer, &mut Context<V>) + 'static,
) -> AnyElement;

pub enum Pointer { Down(Cursor), Move(Cursor), Up }

/// Plain text, with a newline where each block ended. `Doc::spans` answers in
/// parts, and joining them puts a multi-block selection back together.
pub fn copied(doc: &Doc, selection: Selection) -> String;

// ...
```

A screen with several documents on it keeps the selection keyed by item and hands each its own slice of the state — which item a press landed in is not something one block of text can know. `caret_on` is forced off, or a collapsed selection would blink an insertion point in text nobody can type into.

It lives in `markdown` rather than `ui`: it names `Doc`, `Selection`, `Cursor` and `BlockLayouts`, and `ui` depends on `theme` and `motion` alone.
