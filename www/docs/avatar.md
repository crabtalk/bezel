---
title: Avatar
description: A circular initials tile — the fallback every avatar needs, and often the whole thing on a monochrome surface.
---

```rust
use bezel_ui::widgets;

widgets::avatar(&theme, "TC")
widgets::avatar(&theme, "K")
```

One or two initials. There is no image variant: an avatar with a picture in it is `div().rounded_full().overflow_hidden()` around a gpui `img`, and the interesting part — where the image comes from, what happens while it loads, what happens when it fails — belongs to the app. This is the part that is always the same.
