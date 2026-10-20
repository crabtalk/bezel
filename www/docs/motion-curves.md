---
title: Curves
description: Six CSS cubic-bezier timing functions evaluated exactly, each usable as a gpui easing closure.
prompt: Slide the drawer in with the same cubic-bezier(0.32, 0.72, 0, 1) our web app uses.
---

```rust
use gpui::{Animation, AnimationExt as _};
use motion::CubicBezier;

const DRAWER: CubicBezier = CubicBezier::new(0.32, 0.72, 0.0, 1.0);

div().with_animation(
    "drawer",
    Animation::new(Duration::from_millis(300)).with_easing(DRAWER.easing()),
    |el, t| el.opacity(t),
)
```

- A CSS `cubic-bezier()` ports with the same four numbers. The shipped curves are `EASE`, `EASE_OUT`, `EASE_IN_OUT`, `EASE_OUT_EXPO`, `EASE_RESORT` and `EASE_TAILWIND`.
- `eval` clamps its output to `[0,1]`. A hand-written easing must too: gpui's animation element aborts on a delta outside it.
