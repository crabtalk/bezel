---
title: Command palette
description: A filtered command list over a text field, reporting selections as events so it never learns what your commands are.
---

```rust
use ui::palette::{self, CommandPalette, PaletteEvent};

palette::init(cx);   // once, at startup, alongside input::init

let palette = cx.new(|cx| CommandPalette::new(COMMANDS.to_vec(), cx));

cx.subscribe(&palette, |_, _, event, _| match event {
    PaletteEvent::Selected(index) => { /* run command `index` */ }
    PaletteEvent::Dismissed => { /* unmount */ }
})
.detach();
```

Indices are into the **original** item list, never into the filtered view — match on a filtered index and the first query you type runs the wrong command.

## API

| | |
| --- | --- |
| `palette::init(cx)` | Once at startup, alongside `input::init`. |
| `CommandPalette::new(items, cx)` | Owns a query, a filtered view and an active row. |
| `PaletteEvent` | `Selected(index)` or `Dismissed` — the host decides what a selection means. |
| keys | `up`/`down`, `ctrl-p`/`ctrl-n`, `enter`, `escape`, scoped to the palette's context, which wraps the query field's so typing still reaches it. |
| `popover::Filter` | The ranking underneath, shared with `Combobox`: prefix matches first, then substring, stable within each rank. |

Mounting is the caller's — usually centred over a scrim, for which `popover::modal_glass` is the frame. Only text changes re-rank; moving the query caret preserves the highlighted command.
