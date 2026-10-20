---
title: Hover card
description: A preview the pointer can travel into — same mounting as a tooltip, one word different.
prompt: Show a profile preview with initials, bio and location when I hover an @mention.
---

```rust
use ui::hover_card::HoverCard;

div()
    .id("clearloop")
    .hoverable_tooltip(|window, cx| {
        HoverCard::person("CL", "clearloop", "Builds desktop software in Rust.", "Shanghai", window, cx)
    })
    .child("@clearloop")
```

- Mount with `hoverable_tooltip`, not `tooltip`, or the card closes as the pointer moves onto it.
- `HoverCard::summary` is a heading and prose, without the avatar or meta line.
- The delay is gpui's: `.tooltip_show_delay(..)` on the element.
