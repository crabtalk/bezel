---
title: Alert strips
description: Two inline notices — red for what failed, amber for what wants attention — each a leading icon and a line of copy.
---

```rust
use bezel_ui::widgets;

widgets::error_strip(&theme, "Something went wrong.")
widgets::warning_strip(&theme, "Heads up, check this.")
```

Both return a plain `Div`, so a dismiss control is a child you add and a click handler you attach. The message aligns to the top of the icon rather than centring on it, which is what keeps a two-line message from pushing the triangle into the middle of the strip.

Two tones, not a level enum. A strip is either the thing that failed or the thing to watch, and the palette's `danger` and `warning` families are already paired with muted variants for the copy on top.
