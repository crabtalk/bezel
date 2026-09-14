---
title: Buttons
description: Buttons with shared pointer and keyboard activation, stable focus, semantic roles, and label or icon content.
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

Purpose and emphasis are independent: a destructive action can be ghost or prominent.

## Paint catalog

For custom interaction. Both forms resolve their appearance the same way.

```rust
use ui::widgets::{ButtonStyle, Buttons};

theme.button("Save", ButtonStyle::Prominent, None)
theme.icon_button(ui::icons::glyph::Trash, ButtonStyle::Destructive, None)
```

## API

| | |
| --- | --- |
| `Button::new(id, label)` | Identity retains focus across renders; click and `enter`/`space` call the same handler. |
| `.enabled(false)` | Blocks both paths and leaves the tab order. A button with no handler is disabled too. |
| `.label_hidden()` | Hides an icon button's label, keeping its accessible name. |
| `.hover_fade(fade)` | The named hover animation on ghost buttons. |
| `theme.button(label, style, fade)` | The last argument controls ghost hover fading; the key must be stable across frames. |
| `focus::pressable(theme, handle, el, enabled, callback)` | One callback for other painted controls. |

Call `ui::focus::init(cx)` at startup and wrap the window root with `ui::focus::traversal`. Standard gpui modifiers apply last — `.text_color(..)` reaches both the label and the glyph.
