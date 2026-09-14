---
title: Stats
description: The meter — how many frames this window is drawing, what the process costs and what the GPU spent, while you watch it.
---

```rust
use ui::{
    floating::{self, Floating},
    stats::Stats,
};

// Two fields on the view that mounts it: the meter, and where it floats.
meter: Entity<Stats>,
meter_at: Floating,

// In render, inside a `relative()` container.
floating::panel("meter", &self.meter_at, home, self.meter.clone())
```

**A window at rest reads `0`.** Anything above it while nothing is moving means something is asking for frames — one mounted spinner at 120Hz cost 36% of a core on the machine this library was built on.

## Readings

| | |
| --- | --- |
| FPS | This view's own renders, which is the window's draw count. The meter's own two draws a second are included. |
| CPU | The whole process, user plus system, as a percentage of one core — `getrusage`, so `—` on the web build. |
| GPU | Time spent on this window's frames. `—` on a renderer that does not report it. |
| MEM | Resident bytes, sampled at the tick rather than differenced across one. |

## API

| | |
| --- | --- |
| `Stats::new(cx)` | One per window — two mounted meters each count the other's frames. |
| `stats::WIDTH` | The box's width, for a host placing it by its trailing edge. |

Placement is the caller's. A [floating panel](/docs/floating) makes it draggable; a corner works just as well.
