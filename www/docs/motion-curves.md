---
title: Curves
description: Six CSS cubic-bezier timing functions evaluated exactly, each usable as a gpui easing closure.
---

`CubicBezier` is a CSS `cubic-bezier(x1, y1, x2, y2)` with the endpoints fixed at (0,0) and (1,1). `eval` solves x(t) = input by Newton iteration with a bisection fallback — the standard UnitBezier approach — so a curve copied out of a stylesheet plays the same shape here:

```rust
use motion::{CubicBezier, EASE_OUT_EXPO};

EASE_OUT_EXPO.eval(0.5);        // eased progress
element.with_animation(id, animation.with_easing(EASE_OUT_EXPO.easing()), ..)
```

The named curves are `EASE`, `EASE_OUT`, `EASE_IN_OUT`, `EASE_OUT_EXPO` (the signature entrance, `cubic-bezier(0.16, 1, 0.3, 1)`), `EASE_RESORT` for list reordering, and `EASE_TAILWIND` — `cubic-bezier(0.4, 0, 0.2, 1)`, the curve every `transition-colors` hover wash rides in the reference app.

`eval` clamps its output hard. f32 rounding can push the sample a hair past 1.0 — 1.000000119 was observed near the end of a menu animation — and gpui's animation element asserts its delta is in [0,1] and aborts.

The plots on this page are drawn from each curve's own `eval`, and the ones on the catalog page from `MotionSpec::progress`. Both are pure functions of a float, unit-tested without a window.
