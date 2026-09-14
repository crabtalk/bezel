---
title: Curves
description: Six CSS cubic-bezier timing functions evaluated exactly, each usable as a gpui easing closure.
---

```rust
use motion::{CubicBezier, EASE_OUT_EXPO};

EASE_OUT_EXPO.eval(0.5);        // eased progress
element.with_animation(id, animation.with_easing(EASE_OUT_EXPO.easing()), ..)
```

A CSS `cubic-bezier(x1, y1, x2, y2)` with the endpoints fixed, solved by Newton iteration with a bisection fallback, so a curve copied out of a stylesheet plays the same shape here.

## API

| | |
| --- | --- |
| `EASE` / `EASE_OUT` / `EASE_IN_OUT` | The CSS defaults. |
| `EASE_OUT_EXPO` | The signature entrance, `cubic-bezier(0.16, 1, 0.3, 1)`. |
| `EASE_RESORT` | List reordering. |
| `EASE_TAILWIND` | `cubic-bezier(0.4, 0, 0.2, 1)` — every `transition-colors` hover wash. |
| `.eval(t)` | Clamps hard: f32 rounding can push a sample past 1.0, and gpui's animation element asserts its delta is in `[0,1]` and aborts. |
| `.easing()` | The gpui closure. |
