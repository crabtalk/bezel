---
title: Tag
description: A removable chip — a token in a filter bar or a recipient field, with the ✕ wired by the caller.
---

```rust
use ui::widgets::Content;

theme.tag("rust")
```

The ✕ is painted, not wired: only the caller knows what removing a token means for the list behind it.

## API

```rust
pub trait Content: ThemeExt {
    /// The chip. Sets `self_start`, so a column does not stretch it.
    fn tag(&self, label: impl Into<SharedString>) -> Div;

    /// A name on a wash of `tint`, without the ✕ — a label on an entry, an
    /// option of a multi-select. A long name ends in `…` once the caller caps
    /// the width.
    fn chip(&self, label: impl Into<SharedString>, tint: Hsla) -> Div;

    // ...
}
```
