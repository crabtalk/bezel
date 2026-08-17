---
title: Hover card
description: A preview the pointer can travel into — same mounting as a tooltip, one word different.
---

```rust
use bezel_ui::hover_card::HoverCard;

div()
    .id("clearloop")
    .hoverable_tooltip(|window, cx| {
        HoverCard::summary("clearloop", "Builds desktop software in Rust.", window, cx)
    })
    .child("@clearloop")
```

`hoverable_tooltip` rather than `tooltip` is the whole difference, and it means there is no open/close state machine here: gpui owns the delay and keeps the card alive while the pointer is inside it, which is what lets a preview hold a link you can click.

Two constructors. `summary` is a heading and a line or two of prose; `person` adds avatar initials beside the name and a meta line under the body — a role, a path, a timestamp.

The card is wider and airier than a tooltip's because it holds prose rather than a label.
