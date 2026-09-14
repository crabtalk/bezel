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

An entity rather than a plain function: gpui's `.tooltip(..)` takes a builder returning an `AnyView`, since the tooltip is mounted in its own layer after the hover delay.

## With a shortcut

```rust
Tooltip::with_keystroke("Copy path", "⌘C", window, cx)
```

## API

```rust
impl Tooltip {
    /// The label.
    pub fn text(text: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> AnyView;

    /// Shortcut right-aligned in the same card.
    pub fn with_keystroke(
        text: impl Into<SharedString>,
        keystroke: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView;

    // ...
}
```

The delay is gpui's — `.tooltip_show_delay(..)` on the element changes it.
