---
title: Loaders
description: The orb cluster and the older cell grids — four shapes over one period, with every position pure arithmetic.
prompt: Show a thinking indicator next to the assistant's name while the reply is streaming.
---

```rust
use motion::Painter;
use ui::loaders::{self, Orb};

loaders::orb(Orb::Cluster, "thinking", 44.0, &theme, Painter::of(cx), cx)
```

- Pass the calling view's `Painter`; that view re-renders while the loader is mounted. Every instance shares one clock and stays phase-locked.
- Reduced motion shows every loader at rest.

## Make it yours

| call | what it decides |
| --- | --- |
| `Orb::{Cluster, Ring, Converge, Bloom}` | the orb's shape; one tint, from the theme's accent |
| `pulse_loader(id, &theme, cell_px, painter, cx)` | a row of pulsing cells |
| `gradient_spinner(id, &theme, cell_px, painter, cx)` | a 3×3 grid, tinted per row |
| `mini_gradient_spinner(key, cell_px, painter, cx)` | a 2×3 grid sized for a status-dot slot |
| `loading_word(&theme)` | the L O A D I N G caption |
