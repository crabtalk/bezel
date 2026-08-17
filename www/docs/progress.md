---
title: Progress
description: A determinate bar — a clamped fraction over a track that keeps its full width.
---

```rust
use bezel_ui::widgets;

widgets::progress_bar(&theme, 0.35)
```

The fraction is clamped to `0..=1`, and the track keeps its full width whatever the value, so a row never reflows as progress moves.

There is no indeterminate mode here — that is what the loaders are for.
