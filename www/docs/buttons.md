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

```rust
impl Button {
    /// Identity retains focus across renders; click and `enter`/`space` call
    /// the same handler.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self;

    pub fn button_style(self, style: ButtonStyle) -> Self;
    pub fn role(self, role: ButtonRole) -> Self;
    pub fn icon(self, icon: impl Into<Icon>) -> Self;

    /// Blocks both paths and leaves the tab order. A button with no handler is
    /// disabled too.
    pub fn enabled(self, enabled: bool) -> Self;

    /// Hides an icon button's label, keeping its accessible name.
    pub fn label_hidden(self) -> Self;

    /// The named hover animation, on ghost buttons.
    pub fn hover_fade(self, fade: Fade) -> Self;

    pub fn on_press(self, activate: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self;

    // ...
}
```

```rust
pub trait Buttons: ThemeExt {
    /// `fade` controls ghost hover fading; its key must be stable across frames.
    fn button(
        &self,
        label: impl Into<SharedString>,
        style: ButtonStyle,
        fade: Option<Fade>,
    ) -> Div;

    fn icon_button(&self, icon: impl Into<Icon>, style: ButtonStyle, fade: Option<Fade>) -> Div;

    /// The bare frame — padding and children are the caller's.
    fn ghost(&self, id: impl Into<ElementId>) -> Stateful<Div>;

    /// Gathers the buttons beside it onto one background.
    fn control_group(&self) -> Div;
}

/// One callback for other painted controls.
pub fn pressable(
    theme: &Theme,
    handle: &FocusHandle,
    el: gpui::Stateful<Div>,
    enabled: bool,
    activate: impl Fn(&(), &mut Window, &mut App) + 'static,
) -> gpui::Stateful<Div>;
```

Call `ui::focus::init(cx)` at startup and wrap the window root with `ui::focus::traversal`. Standard gpui modifiers apply last — `.text_color(..)` reaches both the label and the glyph.
