---
title: Buttons
description: Buttons with shared pointer and keyboard activation, stable focus, semantic roles, and label or icon content.
---

`Button` owns standard interaction while the caller owns the action:

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

Call `ui::focus::init(cx)` at startup and wrap the window root with
`ui::focus::traversal`. Identity retains focus across renders. Clicking or
pressing Enter/Space calls the same handler. `.enabled(false)` blocks both
paths and removes the button from tab traversal. A button without a handler
is disabled too.

Purpose and emphasis are independent: a destructive action can be ghost or
prominent. `.label_hidden()` hides the visual label of an icon button while
keeping its accessible name. Standard gpui style modifiers apply last;
`.text_color(...)` reaches both the label and the glyph. `.hover_fade(fade)`
uses the named hover animation on ghost buttons.

For custom interaction, the paint catalog remains available. Both forms use
the same appearance resolver:

```rust
use ui::widgets::{ButtonStyle, Buttons};

theme.button("Save", ButtonStyle::Prominent, None)
theme.icon_button(ui::icons::glyph::Trash, ButtonStyle::Destructive, None)
```

The last argument controls ghost hover fading. `Some(Fade::new(Painter::of(cx),
"cancel"))` must have a key stable across frames; `None` leaves hover to the
caller. `ButtonStyle::Destructive` retains the existing filled destructive
appearance; new semantic buttons can express the role separately.

Other painted controls can share one callback with
`focus::pressable(theme, handle, element.id("toggle"), enabled, callback)`.
