---
title: Toggle
description: An 18×32 switch whose knob slides and whose track fills with the text tone when on — state stays with the row that owns it.
prompt: Add a switch to the settings row that turns notifications on and off.
---

```rust
use ui::{focus, widgets::Controls};

focus::focusable(&theme, &self.switch, theme.toggle(self.enabled))
    .id("notifications")
    .on_click(cx.listener(|view, _, _, cx| view.flip(cx)))
    .on_action(cx.listener(|view, _: &focus::Activate, _, cx| view.flip(cx)))
```

- Display-only: the caller holds the bool and wires the click and `focus::Activate`.
