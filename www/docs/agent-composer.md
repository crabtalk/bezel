---
title: Composer
description: A growing field on a frosted card, a send button that knows when there is nothing to send, and a `#` picker anchored at the caret.
prompt: Build a chat composer that grows from 3 to 12 rows, sends on enter, breaks a line on shift-enter, and opens a file picker under the caret when I type #.
---

```rust
cx.bind_keys([
    KeyBinding::new("enter", Send, Some(COMPOSER)),
    KeyBinding::new("shift-enter", input::InsertNewline, Some(COMPOSER)),
]);
let field = cx.new(|cx| TextField::new(cx)
    .with_shape(Shape::Grow { min: 3, max: 12 })
    .with_key_context(COMPOSER)
    .with_placeholder("Ask anything, or # to attach a file"));

// the # picker: popover::Filter state, mounted at the caret
let at = self.field.read(cx).offset_bounds(hash)?;
popover::menu_at("mentions", point(at.left(), at.bottom() + px(4.0)), card, None)
```

- Bind `enter` in a key context of the field's own; a container around the field cannot take `enter` from it.
- `offset_bounds` is `None` until the field has painted once, and is in window coordinates, which `menu_at` takes.
- The card is a `div` on `theme.card_glass_bg()`. What `#` offers is the app's list; the demo is `apps/gallery/src/pages/agent/activity.rs`.
