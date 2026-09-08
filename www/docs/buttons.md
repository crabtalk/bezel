---
title: Buttons
description: Buttons for gpui — ghost, prominent and destructive, each returning a plain gpui::Div you attach your own click handler to.
---

Three buttons, and the difference between them is only ever emphasis. Each returns a `gpui::Div`, so you wrap it in whatever handles the click — bezel does not own your interaction.

```rust
use ui::widgets::{ButtonStyle, Buttons};

theme.button("Cancel", ButtonStyle::Ghost, None)
theme.button("Save", ButtonStyle::Prominent, None)
theme.button("Discard", ButtonStyle::Destructive, None)
```

One function and a closed enum, not three functions: the three are the same control at three emphases, and what differs is an argument.

The last one is the hover fade, and it matters only for `Ghost` — the style whose wash animates per instance. `Some(Fade::new(Painter::of(cx), "dialog-no"))` names the view that paints it and a key that must be stable across frames; two buttons sharing a key trade one another's animation state. `None` is the plain ghost, with the hover left to the caller.

The click handler is yours to attach:

```rust
div()
    .id("dialog-confirm")
    .on_click(cx.listener(|view, _, _, cx| view.close(cx)))
    .child(theme.button("Save", ButtonStyle::Prominent, None))
```
