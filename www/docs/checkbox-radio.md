---
title: Checkbox & radio
description: Display-only checkbox and radio marks — the app owns which are on, and one call puts them in the tab order.
---

Both are `fn(&Theme, bool) -> Div`. They paint the mark and nothing else; whether a box is checked lives in the app:

```rust
use ui::{focus, widgets};

widgets::checkbox(&theme, self.checked[index])
widgets::radio_button(&theme, self.radio == index)
```

Radios are a *set*, so the caller owns which index is on — passing `self.radio == index` is the whole of it. Nothing here groups them, because a group would need to own the answer.

Keyboard support is one wrapper. `focus::focusable` puts the control in the tab order, paints the focus ring, and lets `enter`/`space` press it:

```rust
focus::focusable(&theme, &self.checkboxes[index], widgets::checkbox(&theme, checked))
    .id("checkbox-0")
    .on_click(cx.listener(..))
    .on_action(cx.listener(|view, _: &focus::Activate, _, cx| ..))
```

The click and the key press are handled separately on purpose. A control pressed by mouse and by key is doing the same thing, but only the caller knows what that is, and a keyboard affordance that silently diverges from the click is worse than none.

Every control here carries a 1px border even where it paints nothing in it. gpui sizes border-box, so a border that appeared only on focus would move the tick under it by a pixel as you tab onto it.
