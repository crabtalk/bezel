---
title: Tag
description: A removable chip — a token in a filter bar or a recipient field, with the ✕ wired by the caller.
---

```rust
use bezel_ui::widgets;

widgets::tag(&theme, "rust")
```

The chip paints its own ✕; the click handler for it is yours, because only the caller knows what removing a token means for the list behind it.

It sets `self_start`, so dropping one into a column does not stretch it to the column's width.
