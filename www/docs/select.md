---
title: Select
description: The closed face of a select — a trigger shaped like a text field, opened onto a menu the caller assembles.
---

There is no `Select` component. A select *is* a trigger plus an anchored menu, and the caller already owns the open state and the selection:

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

Wrapping that in a struct would buy an abstraction and cost the caller its control over both halves.

`select_trigger` is shaped and toned like a `TextField`, so a form of fields and selects reads as one system. It takes the current label and nothing else: the chevron is part of the face, and which choice is showing is the caller's to say.

The rows take `None` for their fade because this menu owns an active index and paints it — `Some(fade)` is for a menu with no cursor of its own, letting the mouse light a row by itself.

Dismissal is the caller's. `popover::dismiss_on_out` on the card is what closes it; without it, clicking away leaves the menu open. `anchored_menu_below`'s last argument is the exit clock: `None` for a menu that simply disappears, and `self.menu.closing_since()` when a `popover::Popup` holds the state so the close can animate.
