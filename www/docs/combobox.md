---
title: Combobox
description: A select you can type into — the closed face of a select over an anchored menu whose rows narrow as you search.
---

```rust
use ui::combobox::{self, Combobox, ComboboxEvent};

combobox::init(cx);   // once, at startup, alongside input::init

let language = cx.new(|cx| Combobox::new(LANGUAGES.to_vec(), "Language", cx));

cx.subscribe(&language, |_, _, event, _| match event {
    ComboboxEvent::Selected(index) => { /* item `index` */ }
})
.detach();
```

The reported index is into the **original** item list, never into the filtered view.

## API

```rust
// ui::combobox

/// Once at startup, alongside `input::init`.
pub fn init(cx: &mut App);

impl Combobox {
    /// An entity, because it owns a query `TextField`.
    pub fn new(
        items: Vec<SharedString>,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Self;

    // ...
}

pub enum ComboboxEvent { Selected(usize) }
```

The menu is matched to the trigger's width, measured off the last frame — an anchored layer sizes to its own content and would not otherwise line up with its face. Keys are `up`/`down`, `ctrl-p`/`ctrl-n`, `enter`, `escape`; enter or an arrow opens a focused closed box, and escape closes without changing the value and returns focus to the trigger.

It shares `popover::Filter` and its result rows with the [command palette](/docs/palette), and differs only in frame. Moving the query caret preserves the highlighted result; changing the query re-ranks.
