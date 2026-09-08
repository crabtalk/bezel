---
title: Selectable text
description: Prose a reader can drag over and copy, without an editor behind it — the gesture, and nothing else.
---

Selection used to need an `Editor`. The two hard halves were always in `markdown` — `render_with` paints a `Selection` it is handed, and `BlockLayouts::hit` turns a point back into a `Cursor` — but the *gesture* belonged to whoever owned the selection, and only an editor ever did.

`markdown::selectable` is that gesture:

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

`dragging` is yours because a move must only extend a selection a press started — otherwise the pointer drags one just by crossing the text. Which item the press landed in is not something one block of text can know, so a screen with several documents on it (a transcript's messages, a list of notes) keeps the selection keyed by item and hands each one its own slice of the state.

The release is answered twice, on the text and off it: a drag that ends past the edge of a paragraph is the ordinary way to select to the end of one.

`caret_on` is forced off. A collapsed selection is what every press starts, and without this it would blink an insertion point in text nobody can type into.

## Copying

```rust
markdown::selectable::copied(&doc, selection)
```

Plain text — the words a reader dragged over, with a newline where each block ended. `Doc::spans` answers in parts (a paragraph, a cell, a line of a fence) and joining them is what puts a multi-block selection back together.

This is **not** the editor's copy. That one is `slice` then `serialize`, and it keeps the markup, because what it puts back on paste is blocks. A reader who selected the word inside `**bold**` selected the word.

## Where it lives

In `markdown`, not `ui`: it names `Doc`, `Selection`, `Cursor` and `BlockLayouts`, and `ui` depends on `theme` and `motion` alone. A consumer names `markdown` the same way it already does for `render`.
