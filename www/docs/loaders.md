---
title: Loaders
description: The orb cluster and the older cell grids — four shapes over one period, with every position pure arithmetic.
---

The orbs are bezel's own, and what a thinking surface should reach for:

```rust
use bezel_ui::loaders::{self, Orb};

loaders::orb(Orb::Cluster, "thinking", 44.0, &theme, cx.entity_id(), cx)
```

One function with a shape parameter rather than four functions: they are the same operation, and the thing that differs is an argument. `Cluster` is blobs whose sizes swing so the count you perceive changes; `Ring` is dots on a circle with the brightness chasing round; `Converge` gathers them to a point and opens back out; `Bloom` is rings leaving the centre and fading before the edge — the only one that travels outward, which is what makes it read as a signal rather than a wait.

Everything is circles, because that is the vocabulary gpui gives at the pinned rev: no rotation transform, no conic gradient, no blur filter on an element. So the glow is a `BoxShadow`, the ring is eight positioned dots rather than a swept arc, and every position is arithmetic — all of it pure and unit-tested in `bezel_motion::phase`.

One tint, from the theme's accent. In three hues this would be the gradient spinner wearing a different shape.

The older three are grids of cells: `pulse_loader` (a row), `gradient_spinner` (3×3) and `mini_gradient_spinner` (2×3). `loading_word` is the spaced "L O A D I N G" caption that goes under one.

They all take the calling view's `EntityId` and drive off the shared 30fps pulse clock rather than a per-element repeating animation, so instances stay phase-locked and the clock parks when the last one unmounts. Cells animate inside fixed-size slots — opacity and inner size are paint-local and never move the layout around them. Reduced motion snaps every cell to its rest state.
