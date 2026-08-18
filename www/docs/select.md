---
title: Select
description: The closed face of a select — a trigger shaped like a text field, opened onto a menu the caller assembles.
---

There is no `Select` component. A select *is* a trigger plus an anchored menu, and the caller already owns the open state and the selection:

```rust
use ui::{popover, widgets};

div()
    .id("theme-select")
    .on_click(cx.listener(|view, _, _, cx| view.toggle_menu(cx)))
    .child(widgets::select_trigger(&theme, SELECT_CHOICES[self.choice], open))
    .when(open, |trigger| {
        trigger.child(popover::anchored_menu_below(
            "theme-select-menu",
            popover::popover_card(&theme)
                .w(px(200.0))
                .children(SELECT_CHOICES.iter().enumerate().map(|(index, label)| {
                    popover::menu_row(&theme, index == self.choice, format!("row-{index}"))
                        .child(label)
                })),
        ))
    })
```

Wrapping that in a struct would buy an abstraction and cost the caller its control over both halves.

`select_trigger` is shaped and toned like a `TextField`, so a form of fields and selects reads as one system. It takes the current label and whether the menu is open — the chevron follows.

Dismissal is the caller's. `.on_mouse_down_out` on the card is what closes it; without one, clicking away leaves the menu open. Pair that with `popover::Popup` if you want the menu to animate out rather than vanish.
