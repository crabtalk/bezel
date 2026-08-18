---
title: Tooltip
description: The small hover label, built for gpui's own .tooltip builder — text, optionally with the shortcut beside it.
---

```rust
use ui::tooltip::Tooltip;

div()
    .id("copy")
    .tooltip(|window, cx| Tooltip::text("Copy path", window, cx))
    .child("⌘C")
```

An entity rather than a plain function, because gpui's `.tooltip(..)` takes a builder returning an `AnyView` — the tooltip is mounted in its own layer after the hover delay, so it cannot be an inline element.

`Tooltip::with_keystroke("Copy path", "⌘C", window, cx)` shows the shortcut right-aligned in the same card. That pairing is how a keyboard affordance stays discoverable without opening a menu.

The delay is gpui's, not bezel's — `.tooltip_show_delay(..)` on the element changes it.

The card is the popover surface with tighter padding and no menu rhythm: a tooltip holds a label, not rows.
