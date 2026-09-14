---
title: Floating panel
description: Content that floats over a page and is dragged around it — a meter, an inspector, a detached preview.
---

```rust
use ui::floating::{self, Floating};

// The host holds the position, the way it holds a scrollbar's state.
meter_at: Floating,   // Floating::new(Painter::of(cx))

div().relative().size_full()
    .child(page)
    .child(floating::panel("meter", &self.meter_at, home, child))
```

It does not go through gpui's `on_drag`, which refreshes the whole window on every mouse-move: this claims frames from the shared clock at a 60fps ceiling, so the rate belongs to the library rather than to the mouse.

## API

| | |
| --- | --- |
| `panel(id, at, home, child)` | Lays a full-size layer over its container, because a pointer that outruns a frame is outside the box for most of the gesture. |
| `home` | Where it opens, passed every render rather than stored, so a window that grows never strands it. |
| `held` | The pointer pressed on it, travelling or not — the closed hand. |
| `dragging` | Actually moving, past the two pixels that separate a drag from a click — the lift. |
| `at()` / `move_to()` | For a host persisting a position across sessions. |

It clamps nothing and snaps to nothing. A panel dragged half off the window stays there, with the point it was grabbed by under the pointer, so it can always be dragged back.
