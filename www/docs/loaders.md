---
title: Loaders
description: The orb cluster and the older cell grids — four shapes over one period, with every position pure arithmetic.
---

```rust
use motion::Painter;
use ui::loaders::{self, Orb};

loaders::orb(Orb::Cluster, "thinking", 44.0, &theme, Painter::of(cx), cx)
```

One function with a shape parameter rather than four functions: they are the same operation, and what differs is an argument.

## Shapes

```rust
pub enum Orb {
    /// Blobs whose sizes swing so the count you perceive changes.
    Cluster,
    /// Dots on a circle, brightness chasing round.
    Ring,
    /// Dots gathering to a point and opening back out.
    Converge,
    /// Rings leaving the centre — the only one that travels outward, which
    /// reads as a signal rather than a wait.
    Bloom,
}
```

## Cell grids

```rust
loaders::pulse_loader("busy", &theme, 6.0, view, cx)          // a row
loaders::gradient_spinner("busy", &theme, 6.0, view, cx)      // 3×3
loaders::mini_gradient_spinner("busy", &theme, 5.0, view, cx) // 2×3
loaders::loading_word(&theme)                                 // the L O A D I N G caption
```

## API

```rust
// ui::loaders

/// One tint, from the theme's accent.
pub fn orb(
    shape: Orb,
    key: impl Into<SharedString>,
    size_px: f32,
    theme: &Theme,
    painter: Painter,
    cx: &mut App,
) -> impl IntoElement;

/// The older grids. Cells animate inside fixed slots, so nothing reflows.
pub fn pulse_loader(
    id: &'static str,
    theme: &Theme,
    cell_px: f32,
    painter: Painter,
    cx: &mut App,
) -> impl IntoElement;

pub fn loading_word(theme: &Theme) -> impl IntoElement;

// ...
```

Every loader takes the calling view's `Painter` and drives off the shared 30fps pulse clock, so instances stay phase-locked and the clock parks when the last one unmounts. Reduced motion snaps every cell to its rest state.
