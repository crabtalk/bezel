---
title: Textarea
description: The same text field in a taller box — fixed rows or growing between a min and a max, with enter and pasted newlines behaving accordingly.
prompt: Add a message composer that grows from 3 to 12 lines, where enter sends and shift-enter breaks the line.
---

```rust
use ui::input::{self, Shape, TextField};

const COMPOSER: &str = "Composer";

cx.bind_keys([
    KeyBinding::new("enter", Send, Some(COMPOSER)),
    KeyBinding::new("shift-enter", input::InsertNewline, Some(COMPOSER)),
]);
let composer = cx.new(|cx| {
    TextField::new(cx)
        .with_shape(Shape::Grow { min: 3, max: 12 })
        .with_key_context(COMPOSER)
});
```

- `Shape::Fill` is as tall as its parent, which must give it a definite height.
- Rebind `enter` on a context of the field's own (`with_key_context`). Rebinding `input::MULTILINE_KEY_CONTEXT` takes the newline from every textarea in the app.
