---
title: Buttons
description: Buttons with shared pointer and keyboard activation, stable focus, semantic roles, and label or icon content.
prompt: Add a prominent Save button that calls save(), and a destructive Delete button with a trash icon beside it.
---

```rust
use ui::widgets::{Button, ButtonRole, ButtonStyle};

Button::new("save", "Save")
    .button_style(ButtonStyle::Prominent)
    .on_press(cx.listener(|view, _, _, cx| view.save(cx)))

Button::new("delete", "Delete")
    .role(ButtonRole::Destructive)
    .icon(ui::icons::glyph::Trash)
    .on_press(cx.listener(|view, _, _, cx| view.delete(cx)))
```

- Keyboard activation needs `ui::focus::init(cx)` at startup and the window root wrapped in `ui::focus::traversal`.
- The id holds focus across renders; keep it stable. A button with no `on_press` is disabled.
- `theme.button(label, style, fade)` paints the same look with no behaviour, for a control of your own.
