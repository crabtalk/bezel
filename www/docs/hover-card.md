---
title: Hover card
description: A preview the pointer can travel into — same mounting as a tooltip, one word different.
---

```rust
use ui::hover_card::HoverCard;

div()
    .id("clearloop")
    .hoverable_tooltip(|window, cx| {
        HoverCard::summary("clearloop", "Builds desktop software in Rust.", window, cx)
    })
    .child("@clearloop")
```

`hoverable_tooltip` rather than `tooltip` is the whole difference: gpui owns the delay and keeps the card alive while the pointer is inside it, so there is no open/close state machine here.

## For a person

```rust
HoverCard::person("CL", "clearloop", "Builds desktop software in Rust.", "Shanghai", window, cx)
```

## API

| | |
| --- | --- |
| `HoverCard::summary(title, body, window, cx)` | Heading and a line or two of prose. |
| `HoverCard::person(initials, name, body, meta, window, cx)` | Adds avatar initials beside the name and a meta line under the body. |
