---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
---

Both surfaces come off one method, `Surfaced::surface`, so any element carrying a corner radius has them. The radius is read from the element's own style, so the blur and the lens are cut to the corners the caller just asked for:

```rust
use theme::{Glass, Material, SurfaceStyle};
use ui::surface::{self, Surfaced as _};

card.rounded(px(Theme::surface_radius()))
    .surface(&theme, SurfaceStyle::Material(Material::Regular))
card.rounded(px(Theme::surface_radius()))
    .surface(&theme, SurfaceStyle::Glass(Glass::Regular))
```

Material and glass are different things — a material has thickness, a glass has a variant — and `SurfaceStyle` is where they meet, at the numbers they both resolve to.

`surface::of(radius, style, child)` is the free-function form, for a child that is not itself `Styled`. `surface::popover(radius, child)` is the same on `Theme::popover_surface`, the token the menu, dialog, sheet and tooltip layers mount on — they carry no theme of their own, so an app moves every one of them between frost and glass by moving one field.

## Frost

A surface wraps a floating card so its entire subtree paints inside one gpui scene layer, with the backdrop blur painted first, structurally under the content.

`Material` is SwiftUI's five thicknesses, and measured 2026-08-31 they are one material at five opacities — the tone implied by `tint / (1 - gain)` holds to within 9% across the scale and the sigma does not move at all (23.1/20.2/21.0pt). So the theme carries one `MaterialSpec` and the thickness picks its coverage: a knob, not five looks.

One layer is the point. With per-primitive bounds-tree ordering a hover repaint elsewhere can reassign the card's quads relative to its siblings; inside a single layer the card's stacking is structural. The cost is that everything in the card shares one draw order, and equal orders render grouped by primitive kind — quads, then icons, then images — so a close button's circle painted "after" a thumbnail still lands under it. `surface::layered` opens a nested layer to restore the intended stacking:

```rust
surface::layered(close_button)
```

## Liquid glass

> **Where the primitive runs.** The lens is a backdrop-blur primitive from bezel's gpui fork: Metal reads it off the scene, and so does wgpu, which covers the web build. On Linux and Windows the surface falls back to the flat backdrop tint — the card and its shape, without the refraction at the rim. `surface::lensed(&theme)` answers it at runtime, capability and the theme's own `glass` flag together.

A `Glass` style paints the card as a refracting surface: a bevel at the rim that displaces what is behind it, a per-channel fringe across that displacement, and a transfer line applied to the interior.

It takes the theme and a **closed variant** — the two shipped looks, named for SwiftUI's own:

```rust
card.surface(&theme, SurfaceStyle::Glass(Glass::Regular))
card.surface(&theme, SurfaceStyle::Glass(Glass::Clear)).tint(theme.accent.opacity(0.3))
```

There is no blur parameter, and no gain, bevel or magnify parameter. Apple exposes none either — `Glass` is `.regular`, `.clear`, `.identity`, plus `.tint` and `.interactive` — so blur belongs to the look rather than to the caller. `tint` stands in for the look's own tone instead of adding to it, so a heavy alpha reads as paint and a light one as glass.

The numbers live on `Theme` as `glass_regular` and `glass_clear`, both `SurfaceSpec { gain, saturation, tint, blur, rim, … }`, alongside the shared `glass_magnify` and `glass_dispersion`; the frost's are `theme.material`, the `MaterialSpec` a thickness resolves against. A caller who wants different glass hands over a different theme; nothing is a knob on the component.

Measured 2026-08-30 on macOS 26.3 off a real `NSGlassEffectView` — a nine-step grey staircase read through the flat interior for the line, 48pt bands for the sigma:

| appearance | look | interior | blur |
| --- | --- | --- | --- |
| dark | `Clear` | `0.712 * backdrop + 25/255` | none |
| dark | `Regular` | `0.139 * backdrop + 41/255` | 3.5pt |
| light | `Clear` | `1.041 * backdrop + 19/255` | none |
| light | `Regular` | `0.142 * backdrop + 212/255` | 6.0pt |

The transfer line is fit in **sRGB**, not linear light: refitting in linear space is 30x worse on residual (rms 3–9 levels against 0.14–0.34), so the material composites in gamma space. Note light `Clear`'s slope is above 1, which no alpha composite can produce — it brightens and slightly expands contrast rather than dimming, which is why the field is `gain` and not `dim`.

The material is not a tone-flip of itself. `Regular` keeps its opacity across both — 86% — and swaps a 19% grey base for a 97% white one, which is why in light it reads as ordinary frost: a near-white panel over a blur is what frost is. `Clear` changes character instead: in dark it compresses toward its tint, lifting black to 25 and dropping white to 207 with a crossover at backdrop 87; in light it stops compressing and is very nearly a pure lift.

The rim is measured off a real `NSGlassEffectView` over a position-coded backdrop — green ramping once across it, red sawtoothing every 32pt — so a pixel under the glass names the backdrop position it came from and the displacement is read rather than inferred. It falls from ~47pt at the outermost pixel to nothing by 19pt, and it is the same curve on a 96pt box and a 320pt one, at r24 and at r84 — so `SurfaceSpec::rim` is a length, not a share of the box. Blur is uniform across the surface in both looks; neither sharpens toward the rim.

`surface` clears the card's own `bg`, because the surface paints the fill. Painting both buries the lens, and a caller who has to remember that is a caller who will forget.

## Where it runs

Both need `Window::paint_backdrop_blur` from bezel's gpui fork — any macOS version, and the browser, since the lens is our own shader and not `NSGlassEffectView`.

Where it cannot run — Linux and Windows, or any theme with `glass` off — the fill is painted as a plain quad in the look's own tint, at the card's own corners. The card keeps its shape and the page still shows through; what is lost is the blur and the refraction at the rim, not the surface. A `tint` stands in there too, so a glass control that carries colour still carries it.

Gate glass-only recipes on `theme.glass`, never on the platform and never on the window's frost. `theme.vibrancy` is the separate question of whether the window itself composites translucent — what `Theme::window_background_appearance` answers — and an app moves the two independently: `Brand { vibrancy: false, glass: true }` is an opaque window still carrying layered chrome, which is what a Reduce-transparency setting asks for. Both start from `Theme::VIBRANCY_ALPHA`, so both are on for macOS and the web build and off elsewhere until a brand says otherwise.
