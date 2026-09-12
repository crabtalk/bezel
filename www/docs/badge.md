---
title: Badge
description: A small right-anchored pill for a count or a state, in a quiet variant and an active one.
---

```rust
use ui::widgets::Content;

theme.badge("badge")
theme.badge_active("active")
```

The plain badge is a hairline pill in the muted text tone; `badge_active` is the emerald "connected / running / on" pill.

Both are plain `Div`s, so a badge with an icon in it is a child you add.
