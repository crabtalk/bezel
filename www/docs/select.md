---
title: Select
description: The closed face of a select — a trigger shaped like a text field, opened onto a menu the caller assembles.
---

```rust
use ui::{popover, widgets::Controls};

div()
    .id("theme-select")
    .on_click(cx.listener(|view, _, _, cx| view.toggle_menu(cx)))
    .child(theme.select_trigger(SELECT_CHOICES[self.choice]))
    .when(open, |trigger| {
        trigger.child(popover::anchored_menu_below(
            "theme-select-menu",
            popover::popover_card(&theme)
                .w(px(200.0))
                .children(SELECT_CHOICES.iter().enumerate().map(|(index, label)| {
                    popover::menu_row(&theme, index == self.choice, None).child(label)
                }))
                .into_any_element(),
            None,
        ))
    })
```

There is no `Select` component: a select *is* a trigger plus an anchored menu, and the caller already owns the open state and the selection.

## API

```rust
pub trait Controls: ThemeExt {
    /// Shaped and toned like a `TextField`, so a form of fields and selects
    /// reads as one system. The chevron is part of the face.
    fn select_trigger(&self, label: impl Into<SharedString>) -> Div;

    // ...
}
```

The menu rows take `None` for their fade because this menu owns its active index; `Some(fade)` is for a menu with no cursor of its own. `popover::dismiss_on_out` on the card is what closes it — without it, clicking away leaves the menu open.
