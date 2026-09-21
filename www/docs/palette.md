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

```rust
// ui::palette

/// Once at startup, alongside `input::init`.
pub fn init(cx: &mut App);

impl CommandPalette {
    /// Owns a query, a filtered view and an active row.
    pub fn new(items: Vec<SharedString>, cx: &mut Context<Self>) -> Self;

    // ...
}

/// The host decides what a selection means.
pub enum PaletteEvent { Selected(usize), Dismissed }
```

Keys are `up`/`down`, `ctrl-p`/`ctrl-n`, `enter` and `escape`, scoped to the palette's context — which wraps the query field's, so typing still reaches it. `popover::Filter` is the ranking underneath, shared with `Combobox`: prefix matches first, then substring, stable within each rank.

Mounting is the caller's — usually centred over a scrim, for which `popover::modal_glass` is the frame. Only text changes re-rank; moving the query caret preserves the highlighted command.

Twelve commands show at once and the rest scroll, so a palette over hundreds of them opens the same size as one over twenty. The query line stays above the scroller, the arrows keep the highlighted command in view, and a new query returns the list to the top.
