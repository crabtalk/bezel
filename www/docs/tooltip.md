---
title: Tooltip
description: The small hover label, built for gpui's own .tooltip builder — text, optionally with the shortcut beside it.
prompt: Give the copy button in my toolbar a "Copy path" tooltip that shows its ⌘C shortcut.
---

```rust
use ui::tooltip::Tooltip;

div()
    .id("copy")
    .tooltip(|window, cx| Tooltip::with_keystroke("Copy path", "⌘C", window, cx))
    .child(copy_icon)
```

- The element needs an `.id(..)`.
- `Tooltip::text` is the label alone. `for_action` reads the chord off the keymap and drops it when nothing is bound.
- The delay is gpui's: `.tooltip_show_delay(..)` on the element.
