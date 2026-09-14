---
title: Theme
description: Pick a hue, a chroma and a radius, watch every component repaint, and copy the code that reproduces it.
---

```rust
use bezel::theme::{self, Brand, Tint};

theme::set_brand(
    Brand {
        tint: Tint::new(257.417, 0.046),
        accent: Tint::new(276.935, 0.182),
        radius: 8.0,
        ..Brand::default()
    },
    cx,
); // before appearance::init
```

A `Brand` is what an app changes about the shipped palette without redesigning it: one hue for the greys, one for the accent, one base radius.

## API

```rust
// theme

/// Before `appearance::init`.
pub fn set_brand(brand: Brand, cx: &mut App);

pub struct Brand {
    /// The hue every grey in the palette carries. A token that already carries
    /// one — `danger`, `warning`, `success` — is semantic and keeps it, and
    /// translucent ink is left alone.
    pub tint: Tint,
    /// The emphasis hue. Moves `accent_strong` (the plate) and `on_accent`,
    /// which is measured, so a yellow plate takes a dark label.
    pub accent: Tint,
    /// The button corner. Every other is a ratio: bubble 2x, surfaces 1.5x,
    /// panels 1.25x, small controls 0.75x.
    pub radius: f32,
    pub vibrancy_alpha: f32,
    pub vibrancy: Vibrancy,
    pub glass: bool,
}

impl Tint {
    /// An oklch hue and how much of it. At `chroma: 0.0` it is the neutral the
    /// library ships, so `Brand::default()` reproduces the built-in palette
    /// byte for byte.
    pub const fn new(hue: f32, chroma: f32) -> Self;
}

/// For colours a hue rotation cannot reach. It runs first, and a brand rotates
/// whatever it returns.
pub fn set_palette(build: fn(Appearance) -> Theme, cx: &mut App);

// ...
```

Lightness is never a knob: every tone was tuned against a measured contrast ratio, so a brand rotates hue and leaves those ratios where they were — `text` on `bg` is 16.09:1 unbranded and stays within a tenth of that at any hue.
