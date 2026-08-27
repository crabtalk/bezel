---
title: Floating panel
description: Content that floats over a page and is dragged around it — a meter, an inspector, a detached preview.
---

```rust
use ui::floating::{self, Floating};

// The host holds the state, like a scrollbar's.
meter_at: Floating,   // Floating::new(Painter::of(cx))

div().relative().size_full()
    .child(page)
    .child(floating::panel("meter", &self.meter_at, home, child))
```

The panel lays a full-size layer over its container and places the box inside it, because the drag has to be heard somewhere larger than the thing being dragged. A pointer that outruns a frame is outside the box for most of the gesture, and a listener mounted on the box would go quiet and leave it stranded behind the cursor. `scroll` hangs its thumb drag off the track for the same reason.

**It does not go through gpui's `on_drag`.** That refreshes the entire window on every mouse-move event, and a pointer reports far faster than a window can paint — dragging a panel that way cost a full core. This claims frames from the shared clock instead, at a 60fps ceiling, so the rate belongs to the library rather than to the mouse. A claim is not a redraw: the pointer sample records where the panel now is and schedules, and the clock paints whatever the latest sample said.

Movement is carried as a delta from the last sample rather than as an offset from the box's corner. A delta reads the same from the window's origin or the container's, so the panel needs no element bounds to place itself — and a plain mouse listener is not offered any.

`home` is where it opens, and it is passed every render rather than stored, so a host can read it off the viewport and a window that grows never strands the panel out of reach. Once dragged, the panel holds a position of its own.

Two states, because they answer different questions. `held` is the pointer pressed on it, travelling or not — that is the closed hand, which closes on the press the way every other grabbable surface does. `dragging` is the panel actually moving, past the two pixels that separate a drag from a click — that is the lift, the shadow, whatever a host shows for a thing in flight.

It clamps nothing, snaps to nothing and remembers nothing across launches. A panel dragged half off the window stays there, and the point it was grabbed by is under the pointer, so it can always be dragged back. `at()` and `move_to()` are there for a host that wants to persist a position across sessions.
