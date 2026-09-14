---
title: Composer
description: A growing field on a frosted card, a send button that knows when there is nothing to send, and a `#` picker anchored at the caret.
---

```rust
TextField::new(cx)
    .with_shape(Shape::Grow { min: 3, max: 12 })
    .with_key_context(COMPOSER_CONTEXT)
    .with_placeholder("Ask anything, or # to attach a file")
```

One library call on a card of `Theme::card_glass_bg`. `enter` sends and `shift-enter` breaks a line, through a [key context](/docs/textarea) of the field's own.

## The mention picker

```rust
let anchor = self.field.read(cx).offset_bounds(hash, window)?;

popover::menu_at(
    "composer-mentions",
    gpui::point(anchor.left(), anchor.bottom() + px(4.0)),
    card,
    None,
)
```

The only thing this pattern had to invent — and it is `popover::Filter`, the combobox's own state, mounted at a caret instead of under a trigger.

Its trigger is a *read* of the text rather than a key handler: the `#` nearest behind the caret, if nothing since it has been whitespace. Typing, pasting, arrowing back into a word and deleting the `#` then all agree without being special-cased.

`offset_bounds` is the same measurement the IME candidate panel anchors to, so the menu follows the caret down as the box grows. What `#` offers is a `Vec<SharedString>` — the app searches its own store.

The source is at `apps/gallery/src/patterns/agent.rs`.
