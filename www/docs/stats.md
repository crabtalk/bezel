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

**A window at rest reads `0`.** That is the number the whole thing exists for. Anything above it while nothing on screen is moving means something is asking for frames, and a single element can hold a whole window at the display's rate — one mounted spinner drawn at 120Hz cost 36% of a core on the machine this library was built on.

The count is this view's own renders, which is the same number as the window's draws: gpui re-renders every uncached view once per draw. The one render it does not count is the one its own tick provoked, and the clock says which that was rather than anything here inferring it from a stopwatch. Those two draws a second are what the meter itself costs, and the CPU figure includes them.

**CPU** is the whole process — user plus system, every thread — as a percentage of one core, which is the figure Activity Monitor prints. It comes from `getrusage`, so it reads `—` where the platform has no such call, including the web build.

**GPU** is the time the GPU spent on this window's frames over the same interval. It reads `—` on a renderer that does not report it.

Placement is the caller's, as it is for the control bar. Mounting it in a [floating panel](/docs/floating) is what makes it draggable; a corner works just as well if you would rather it stayed put.
