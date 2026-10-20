---
title: Text field
description: A single-line gpui text field with IME, selection, clipboard and undo — an entity you hold, with its key bindings scoped to the field.
prompt: Add a search field above the list that filters it as I type, with "Search…" as its placeholder.
---

```rust
use ui::input::{self, FieldEvent, TextField};

input::init(cx); // once, at startup

let field = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
cx.subscribe(&field, |view, field, event, cx| {
    if let FieldEvent::Changed(_) = event {
        view.filter(field.read(cx).content().clone(), cx);
    }
}).detach();
// …then render it: .child(field.clone())
```

- A field is an entity the caller holds; render the same `Entity<TextField>` every frame.
- Subscribe to `FieldEvent`, not `cx.observe`: observe fires on every caret blink.
- Bindings are scoped to `input::KEY_CONTEXT`. To rebind, take `input::bindings()` and layer over it with `ui::keys`.
- `set_content` clears the undo history.
