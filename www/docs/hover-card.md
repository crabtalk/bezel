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

```rust
impl HoverCard {
    /// Heading and a line or two of prose.
    pub fn summary(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView;

    /// Adds avatar initials beside the name and a meta line under the body.
    pub fn person(
        initials: impl Into<SharedString>,
        name: impl Into<SharedString>,
        body: impl Into<SharedString>,
        meta: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyView;
}
```
