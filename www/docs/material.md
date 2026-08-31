---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
---

Both surfaces come off the `Glass` trait, so any element carrying a corner radius has them. The radius is read from the element's own style, so the blur and the lens are cut to the corners the caller just asked for:

```rust
use ui::material::{self, Glass as _, GlassStyle};

card.rounded(px(Theme::surface_radius())).material(material::MENU_BLUR)
card.rounded(px(Theme::surface_radius())).glass_effect(&theme, GlassStyle::Regular)
```

`material::material(radius, blur, child)` is the free-function form, for a child that is not itself `Styled`.

## Frost

`material` wraps a floating card so its entire subtree paints inside one gpui scene layer, with the backdrop blur painted first, structurally under the content.

One layer is the point. With per-primitive bounds-tree ordering a hover repaint elsewhere can reassign the card's quads relative to its siblings; inside a single layer the card's stacking is structural. The cost is that everything in the card shares one draw order, and equal orders render grouped by primitive kind — quads, then icons, then images — so a close button's circle painted "after" a thumbnail still lands under it. `material::layered` opens a nested layer to restore the intended stacking:

```rust
material::layered(close_button)
```

## Liquid glass

> **macOS only.** The lens is a Metal primitive from bezel's gpui fork. Everywhere else — web, Linux, Windows — `glass_effect` falls back to the backdrop tint: the card and its shape, without the refraction at the rim. `material::lensed(&theme)` answers it at runtime.

`glass_effect` paints the card as a refracting surface: a bevel at the rim that displaces what is behind it, a per-channel fringe across that displacement, and a transfer line applied to the interior.

It takes the theme and a **closed variant** — the two shipped looks, named for SwiftUI's own:

```rust
card.glass_effect(&theme, GlassStyle::Regular)
card.glass_effect(&theme, GlassStyle::Clear).tint(theme.accent.opacity(0.3))
```

There is no blur parameter, and no gain, bevel or magnify parameter. Apple exposes none either — `Glass` is `.regular`, `.clear`, `.identity`, plus `.tint` and `.interactive` — so blur belongs to the look rather than to the caller. `tint` stands in for the look's own tone instead of adding to it, so a heavy alpha reads as paint and a light one as glass.

The numbers live on `Theme` as `glass_regular` and `glass_clear`, both `GlassSpec { gain, tint, blur, rim }`, alongside the shared `glass_magnify` and `glass_dispersion`. A caller who wants different glass hands over a different theme; nothing is a knob on the component.

Measured 2026-08-30 on macOS 26.3 off a real `NSGlassEffectView` — a nine-step grey staircase read through the flat interior for the line, 48pt bands for the sigma:

| appearance | look | interior | blur |
| --- | --- | --- | --- |
| dark | `Clear` | `0.712 * backdrop + 25/255` | none |
| dark | `Regular` | `0.139 * backdrop + 41/255` | 3.5pt |
| light | `Clear` | `1.041 * backdrop + 19/255` | none |
| light | `Regular` | `0.142 * backdrop + 212/255` | 6.0pt |

The transfer line is fit in **sRGB**, not linear light: refitting in linear space is 30x worse on residual (rms 3–9 levels against 0.14–0.34), so the material composites in gamma space. Note light `Clear`'s slope is above 1, which no alpha composite can produce — it brightens and slightly expands contrast rather than dimming, which is why the field is `gain` and not `dim`.

The material is not a tone-flip of itself. `Regular` keeps its opacity across both — 86% — and swaps a 19% grey base for a 97% white one, which is why in light it reads as ordinary frost: a near-white panel over a blur is what frost is. `Clear` changes character instead: in dark it compresses toward its tint, lifting black to 25 and dropping white to 207 with a crossover at backdrop 87; in light it stops compressing and is very nearly a pure lift.

The rim is measured off a real `NSGlassEffectView` over a position-coded backdrop — green ramping once across it, red sawtoothing every 32pt — so a pixel under the glass names the backdrop position it came from and the displacement is read rather than inferred. It falls from ~47pt at the outermost pixel to nothing by 19pt, and it is the same curve on a 96pt box and a 320pt one, at r24 and at r84 — so `GlassSpec::rim` is a length, not a share of the box. Blur is uniform across the surface in both looks; neither sharpens toward the rim.

`glass_effect` clears the card's own `bg`, because the lens paints the fill. Painting both buries the lens, and a caller who has to remember that is a caller who will forget.

## Where it runs

Both need `Window::paint_backdrop_blur` from bezel's gpui fork, which is macOS Metal only — any macOS version, since the lens is our own shader and not `NSGlassEffectView`.

Off macOS the frost alpha is 1.0, so `theme.is_glass()` is false and neither surface composites. `material` becomes a pass-through and the caller's own fill shows through unchanged. `glass_effect` has moved the fill inside the lens, so where the lens cannot run it paints `Theme::glass_overlay` — the same tint a floating card carries over a blur, without the blur under it. The card keeps its shape and the page still shows through; what is lost is the refraction at the rim, not the surface. A `tint` is painted over that backing, so a glass control that carries colour still carries it.

Gate glass-only recipes on `theme.is_glass()`, never on the platform: the frost alpha is per-appearance, and light chrome is opaque by design.
