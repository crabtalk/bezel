---
title: Badge
description: A small right-anchored pill for a count or a state, in a quiet variant and an active one.
---

```rust
use ui::widgets::Content;

theme.badge("badge")
theme.badge_active("active")
```

Both return a `Div`, so a badge with an icon in it is a child you add.

## API

```rust
pub trait Content: ThemeExt {
    /// Hairline pill in the muted text tone.
    fn badge(&self, label: impl Into<SharedString>) -> Div;

    /// The emerald "connected / running / on" pill.
    fn badge_active(&self, label: impl Into<SharedString>) -> Div;

    // ...
}
```
