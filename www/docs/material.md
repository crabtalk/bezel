---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
---

```rust
use theme::{Glass, Material, SurfaceStyle};
use ui::surface::{self, Surfaced as _};

card.rounded(px(Theme::surface_radius()))
    .surface(&theme, SurfaceStyle::Material(Material::Regular))
card.rounded(px(Theme::surface_radius()))
    .surface(&theme, SurfaceStyle::Glass(Glass::Regular))
```

Both come off one method, and the radius is read from the element's own style, so the blur and the lens are cut to the corners the caller just asked for.

## Glass

```rust
card.surface(&theme, SurfaceStyle::Glass(Glass::Clear)).tint(theme.accent.opacity(0.3))
```

No blur, gain, bevel or magnify parameter — Apple exposes none either. `tint` stands in for the look's own tone rather than adding to it, so a heavy alpha reads as paint and a light one as glass.

## Overlays inside a card

```rust
surface::layered(close_button)
```

A surface is one scene layer, and equal draw orders render grouped by primitive kind — quads, then icons, then images — so a close button's circle painted "after" a thumbnail still lands under it.

## API

| | |
| --- | --- |
| `Surfaced::surface(theme, style)` | On any element carrying a corner radius. Clears the card's own `bg`, since the surface paints the fill. |
| `surface::of(radius, style, child)` | The free function, for a child that is not `Styled`. |
| `surface::popover(radius, child)` | On `Theme::popover_surface` — the token menus, dialogs, sheets and tooltips mount on, so one field moves every one of them between frost and glass. |
| `surface::layered(child)` | A nested layer, to restore stacking inside a card. |
| `surface::lensed(theme)` | Whether the lens will actually refract here — capability and the theme's `glass` flag together. |
| `Material` | SwiftUI's five thicknesses. Measured, they are one material at five opacities, so the theme carries one `MaterialSpec` and the thickness picks its coverage. |
| `Glass::Regular` / `Clear` | A closed variant. The numbers are `theme.glass_regular` / `glass_clear`. |

> **Where the primitive runs.** The lens needs `Window::paint_backdrop_blur` from bezel's gpui fork: Metal and wgpu, which covers the web build. Elsewhere the surface falls back to the flat backdrop tint — the card and its shape, without the refraction at the rim.

## Measurements

Taken 2026-08-30 on macOS 26.3 off a real `NSGlassEffectView`.

| appearance | look | interior | blur |
| --- | --- | --- | --- |
| dark | `Clear` | `0.712 * backdrop + 25/255` | none |
| dark | `Regular` | `0.139 * backdrop + 41/255` | 3.5pt |
| light | `Clear` | `1.041 * backdrop + 19/255` | none |
| light | `Regular` | `0.142 * backdrop + 212/255` | 6.0pt |

The line is fit in **sRGB** — refitting in linear light is 30× worse on residual. Light `Clear`'s slope is above 1, which no alpha composite can produce, hence `gain` rather than `dim`. The rim falls from ~47pt at the outermost pixel to nothing by 19pt, the same curve on a 96pt box and a 320pt one, so `SurfaceSpec::rim` is a length rather than a share of the box.

Gate glass-only recipes on `theme.glass`, never on the platform — `theme.vibrancy` is the separate question of whether the window itself composites translucent.
