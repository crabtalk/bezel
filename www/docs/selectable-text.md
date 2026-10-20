---
title: Selectable text
description: Prose a reader can drag over and copy, without an editor behind it — the gesture, and nothing else.
prompt: Make the chat messages selectable, so a reader can drag across a message and copy the text with cmd-C.
---

```rust
use markdown::{Selection, render::selectable::{self, Pointer}};

selectable::render("message", &self.doc, &self.layouts, self.selection, self.dragging, window, cx,
    |view, pointer, cx| {
        match pointer {
            Pointer::Down(cursor, _) => (view.selection, view.dragging) = (Some(Selection::at(cursor)), true),
            Pointer::Move(cursor) => view.selection = view.selection.map(|s| s.extend_to(cursor)),
            Pointer::Up => view.dragging = false,
        }
        cx.notify();
    },
)
```

- The selection, `dragging` and the `BlockLayouts` are yours; keep the layouts across frames, since the next press resolves against them. `dragging` may be `true` for at most one rendered document at a time.
- `Pointer::Down` carries the unit the click count asks for. `cursor.span(unit, &doc)` with `ui::input::drag_selection` selects by word and line.
- `selectable::copied(&doc, selection)` is plain text, with a newline between blocks. `selectable::surface(&focus, &doc, selection, cx)` wraps the text so a click focuses it and the platform copy chord copies; keep `focus` stable.
