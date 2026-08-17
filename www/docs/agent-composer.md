---
title: Composer
description: A growing field on a frosted card, a send button that knows when there is nothing to send, and a `#` picker anchored at the caret.
---

The prompt box is one library call — `Shape::Grow { min, max }`, the field that grows with what you type and scrolls past its maximum — on a card of `Theme::card_glass_bg` with a row of controls under it. `enter` sends and `shift-enter` breaks a line, which is a key context of the field's own:

```rust
TextField::new(cx)
    .with_shape(Shape::Grow { min: 3, max: 12 })
    .with_key_context(COMPOSER_CONTEXT)
    .with_placeholder("Ask anything, or # to attach a file")
```

So the only thing this pattern had to invent is the mention picker, and that is `popover::Filter` — the combobox's own state — mounted at a caret instead of under a trigger.

Its trigger is a *read* of the text rather than a key handler: the `#` nearest behind the caret, if nothing since it has been whitespace. Typing, pasting, arrowing back into a word and deleting the `#` then all agree without any of them being special-cased, and the picker closes on a backspace over the `#` without anything having to tell it.

It hangs under the `#` itself:

```rust
let anchor = self.field.read(cx).offset_bounds(hash, window)?;

popover::menu_at(
    "composer-mentions",
    gpui::point(anchor.left(), anchor.bottom() + px(4.0)),
    card,
    None,
)
```

`offset_bounds` is the same measurement the IME candidate panel anchors to, so the menu follows the caret down as the box grows a row at a time.

What `#` offers is a `Vec<SharedString>`. The app searches its own store; bezel takes a list of strings, which is the whole difference between a library and an app.

The source is at `apps/gallery/src/patterns/agent.rs`.
