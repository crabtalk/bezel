---
title: Stats
description: The meter — how many frames this window is drawing, what the process costs and what the GPU spent, while you watch it.
prompt: Add a draggable FPS and CPU meter in the top-right corner of the window in debug builds, so I can see when something keeps redrawing.
---

```rust
use ui::{floating::{self, Floating}, stats::{self, Stats}};

// in the view's constructor
meter: cx.new(Stats::new),
meter_at: Floating::new(Painter::of(cx)),

// in render, inside a relative() container
let home = point(viewport.width - px(stats::WIDTH + 16.0), px(16.0));
floating::panel("meter", &self.meter_at, home, self.meter.clone())
```

- A window at rest reads `0` FPS. The meter's own two draws a second are not counted; its CPU cost is.
- One meter per window.
- CPU, GPU and MEM read `—` where the platform does not report them, the web build included.
- Placement is the caller's; [`floating::panel`](/docs/floating) makes it draggable.
