---
title: Floating panel
description: Content that floats over a page and is dragged around it — a meter, an inspector, a detached preview.
prompt: Show a small FPS meter floating over the page that I can drag anywhere.
---

```rust
use ui::floating::{self, Floating};

// self.meter: Floating, built with Floating::new(Painter::of(cx))
div().relative().size_full()
    .child(page)
    .child(floating::panel("meter", &self.meter, home, meter))
```

- Mount it in a `relative()` container; it lays a layer over that container's whole box.
- Pass `home` every render. The panel sits there until first dragged.
- Nothing is clamped or persisted. Read `at()` and hand it back with `move_to` to keep a position across launches.
- `held()` and `dragging()` tell a press from a move, for a grab cursor or a lift.
