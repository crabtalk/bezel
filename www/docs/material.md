---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
---

Both surfaces come off the `Glass` trait, so any element carrying a corner radius has them. The radius is read from the element's own style, so the blur and the lens are cut to the corners the caller just asked for:

```rust
use ui::material::{self, Glass as _};

card.rounded(px(Theme::surface_radius())).material(material::MENU_BLUR)
card.rounded(px(Theme::surface_radius())).glass_effect()
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

`glass_effect` paints the card as a refracting surface: a bevel at the rim that displaces what is behind it, a per-channel fringe across that displacement, and an additive lift over an interior that passes the backdrop straight through.

The profile is measured off SwiftUI's `.glassEffect(.clear)` (2026-08). On a 460x120 capsule the ruler behind it is displaced over the outer 27pt and is exactly unperturbed below that — a bezel with a flat interior, which is the 0.225 share of the shape's smaller side that `Theme::glass_bevel` returns. The interior measures `1.042 * backdrop + 19/255` across five luminances, so `Theme::glass_clear` composites additively and brightens whatever sits behind it at every level.

`glass_effect` clears the card's own `bg`, because the lens paints the fill. Painting both buries the lens, and a caller who has to remember that is a caller who will forget.

Glass is clear by default, and two builders move it:

```rust
card.glass_effect().tint(theme.accent.opacity(0.3))
card.glass_effect().blurred(material::MENU_BLUR)
```

`tint` stands in for `glass_clear`'s neutral lift instead of adding to it, so a heavy alpha reads as paint and a light one as glass. `blurred` frosts the backdrop under the lens, for a surface that has to carry dense content.

The rim width and the displacement amplitude are process-wide, for the same reason the base radius is — the element builders that paint a lens have no `cx` in scope:

```rust
theme::set_glass_bevel(0.225); // share of the shape's smaller side
theme::set_glass_magnify(0.34); // signed: negative inverts the lens
```

## Where it runs

Both need `Window::paint_backdrop_blur` from bezel's gpui fork, which is macOS Metal only — any macOS version, since the lens is our own shader and not `NSGlassEffectView`.

Off macOS the frost alpha is 1.0, so `theme.is_glass()` is false and neither surface composites. `material` becomes a pass-through and the caller's own fill shows through unchanged. `glass_effect` has moved the fill inside the lens, so where the lens cannot run it paints `Theme::glass_overlay` — the same tint a floating card carries over a blur, without the blur under it. The card keeps its shape and the page still shows through; what is lost is the refraction at the rim, not the surface. A `tint` is painted over that backing, so a glass control that carries colour still carries it.

Gate glass-only recipes on `theme.is_glass()`, never on the platform: the frost alpha is per-appearance, and light chrome is opaque by design.
