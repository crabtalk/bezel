---
title: Select
description: The closed face of a select — a trigger shaped like a text field, opened onto a menu the caller assembles.
prompt: Add a theme dropdown to the settings form that shows the current theme and opens a menu of Light, Dark and System.
---

```rust
use ui::{popover, widgets::Controls};

div()
    .id("theme-select")
    .on_click(cx.listener(|view, _, _, cx| view.toggle_menu(cx)))
    .child(theme.select_trigger(CHOICES[self.choice]))
    .when(self.open, |trigger| {
        let rows = CHOICES.iter().enumerate().map(|(index, label)| {
            popover::menu_row(&theme, index == self.choice, None).child(*label)
        });
        let card = popover::popover_card(&theme).w(px(200.0)).children(rows);
        trigger.child(popover::anchored_menu_below("theme-menu", card.into_any_element(), None))
    })
```

- There is no `Select` component; the caller owns the open state and the selection.
- Close the menu yourself: `popover::dismiss_on_out` or `.on_mouse_down_out` on the card.
- Pass `None` as a row's fade when the menu moves its own active index; `Some(fade)` only for a menu with no cursor.
