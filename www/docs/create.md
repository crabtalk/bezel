---
title: Create
description: Pick a hue, a chroma and a radius, watch every component repaint, and copy the code that reproduces it.
---

A `Brand` is what an app changes about the shipped palette without redesigning it — one hue for the greys, one for the accent, one base radius:

```rust
use bezel::theme::{self, Brand, Tint};

theme::set_brand(
    Brand {
        tint: Tint::new(257.417, 0.046),
        accent: Tint::new(276.935, 0.182),
        radius: 8.0,
    },
    cx,
); // before appearance::init
```

`Tint` is an oklch hue and how much of it. At `chroma: 0.0` it is the neutral the library ships, so `Brand::default()` reproduces the built-in palette exactly — the same bytes, not a close match.

Lightness is never a knob. Every tone in `Theme::dark` and `Theme::light` was tuned against a measured contrast ratio, so a brand rotates hue and leaves those ratios where they were: `text` on `bg` is 16.09:1 unbranded and stays within a tenth of that at any hue. The page prints the four pairings a brand can break, in both appearances, so you watch the numbers rather than trust them.

One rule decides which tokens take the tint: a token that is already grey takes the hue, and one that already carries a hue — `danger`, `warning`, `success` — is semantic and keeps it. Translucent ink is left alone, because it paints over a surface that is tinted already.

The base colours are Tailwind's five neutral families at the chroma each carries mid-ramp. Chroma is constant across the ramp here rather than tapered per step, and falls off only where sRGB runs out — near black and near white the gamut is a needle, and asking for a mid-ramp chroma there would shift the hue rather than the saturation.

An accent moves two tokens beyond `accent` itself. `accent_strong` is the plate, at the lightness the palette's existing chromatic plate uses, and `on_accent` is whichever of the palette's two extremes that plate can actually hold — measured, so a yellow plate takes a dark label and a blue one takes a light label without either being written down.

`radius` is the button corner, and every other corner is a ratio of it: the bubble at 2×, floating surfaces at 1.5×, panels at 1.25×, small controls at 0.75×. Moving one number moves the set together and keeps the concentric relationships intact.

The page keeps no palette of its own. Knobs write the theme global, the readouts build from `Theme::branded`, and the snippet prints the same `Brand` both of those used — so the code you copy is the thing on screen. It emits three files: the `set_brand` call, a `Cargo.toml`, and a `main.rs` that opens a window with your palette already installed.

For colours a hue rotation cannot reach, register a palette builder with `set_palette`. It runs first and a brand rotates whatever it returns.
