---
title: Checkbox & radio
description: Display-only checkbox and radio marks — the app owns which are on, and one call puts them in the tab order.
prompt: Add a "Remember me" checkbox and a pair of radio buttons for monthly or yearly billing, both reachable by keyboard.
---

```rust
use ui::{focus, widgets::Controls};

focus::focusable(&theme, &self.remember_focus, theme.checkbox(self.remember))
    .id("remember")
    .on_click(cx.listener(|view, _, _, cx| view.toggle_remember(cx)))
    .on_action(cx.listener(|view, _: &focus::Activate, _, cx| view.toggle_remember(cx)))

theme.radio_button(self.billing == Billing::Yearly)
```

- The marks are display-only: the caller holds the bools and the radio index, and nothing groups radios.
- Keyboard use needs `ui::focus::init(cx)` and the window root in `focus::traversal`. Click and `focus::Activate` are wired separately.
