---
title: Avatar
description: A circular initials tile — the fallback every avatar needs, and often the whole thing on a monochrome surface.
---

```rust
use ui::widgets::Content;

theme.avatar("TC")
theme.avatar("K")
```

No image variant: that is `div().rounded_full().overflow_hidden()` around a gpui `img`, and where the image comes from belongs to the app.

## API

| | |
| --- | --- |
| `avatar(initials)` | One or two initials, 28px. |
