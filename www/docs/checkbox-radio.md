---
title: Checkbox & radio
description: Display-only checkbox and radio marks — the app owns which are on, and one call puts them in the tab order.
---

```rust
use ui::{focus, widgets::Controls};

theme.checkbox(self.checked[index])
theme.radio_button(self.radio == index)
```

Radios are a *set*, so the caller owns which index is on. Nothing here groups them, because a group would have to own the answer.

## Keyboard

```rust
focus::focusable(&theme, &self.checkboxes[index], theme.checkbox(checked))
    .id("checkbox-0")
    .on_click(cx.listener(..))
    .on_action(cx.listener(|view, _: &focus::Activate, _, cx| ..))
```

## API

| | |
| --- | --- |
| `checkbox(checked)` | The mark. |
| `radio_button(selected)` | The mark. |
| `focus::focusable(theme, handle, el)` | Tab order, focus ring, and `enter`/`space`. |

The click and the key press are wired separately on purpose: only the caller knows what the press means. Every control keeps a 1px border even where it paints nothing in it — gpui sizes border-box, so a border appearing only on focus would move the tick by a pixel as you tab onto it.
