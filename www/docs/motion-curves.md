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

```rust
// motion

/// The CSS defaults.
pub const EASE: CubicBezier;
pub const EASE_OUT: CubicBezier;
pub const EASE_IN_OUT: CubicBezier;

/// The signature entrance, `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_OUT_EXPO: CubicBezier;

/// List reordering.
pub const EASE_RESORT: CubicBezier;

/// `cubic-bezier(0.4, 0, 0.2, 1)` — every `transition-colors` hover wash.
pub const EASE_TAILWIND: CubicBezier;

impl CubicBezier {
    /// Clamps hard: f32 rounding can push a sample past 1.0, and gpui's
    /// animation element asserts its delta is in `[0,1]` and aborts.
    pub fn eval(&self, x: f32) -> f32;

    /// The gpui closure.
    pub fn easing(self) -> impl Fn(f32) -> f32;

    // ...
}
```
