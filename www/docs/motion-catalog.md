---
title: Catalog
description: Every named motion spec — duration, delay and curve — plus the hover-fade store components blend their washes through.
prompt: Add a gentle breathing pulse to the recording dot that respects Reduce Motion, and fade the list rows' hover background.
---

```rust
use motion::{AppExt as _, Fade, MotionSpec, Painter};

motion::menu_in("menu", menu)                                // a catalog entrance

let fade = Fade::new(Painter::of(cx), "row-3");              // a hover wash
div()
    .on_hover(motion::hover_listener(fade.clone()))
    .bg(motion::hover_blend(&fade, theme.surface, theme.element_hover))

const BREATHE: MotionSpec = MotionSpec::new(1800, motion::EASE_IN_OUT);
let phase = motion::pulse_delta(&BREATHE, Painter::of(cx), cx); // in render

cx.set_reduced_motion(true);
```

- A repeating animation goes through `pulse_delta` or `Painter::lease`, renewed every render. `with_animation(..).repeat()` holds the whole window at the display's rate while mounted.
- A `Fade` key need only be unique within its view. Read the same `Fade` in `hover_listener` and `hover_blend`.
- `menu_out` takes its eased progress from the caller: `with_animation`'s clock restarts from 0 on remount.
- Animation stops while the app is not frontmost; `cx.set_pause_when_inactive(false)` keeps it running. Reduced motion snaps every element, and `pulse_delta` returns 0.
