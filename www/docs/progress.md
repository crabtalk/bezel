---
title: Progress
description: A determinate bar — a clamped fraction over a track that keeps its full width.
---

```rust
use ui::widgets::Controls;

theme.progress_bar(0.35)
```

No indeterminate mode — that is what the [loaders](/docs/loaders) are for.

## API

```rust
pub trait Controls: ThemeExt {
    /// Clamped to `0..=1`; the track keeps its width, so a row never reflows
    /// as progress moves.
    fn progress_bar(&self, fraction: f32) -> Div;

    // ...
}
```
