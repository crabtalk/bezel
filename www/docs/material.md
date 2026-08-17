---
title: Materials
description: The glass float — a backdrop-blurred card painted in one scene layer, with a nested-layer escape hatch for overlays inside it.
---

`material` wraps a floating card so its entire subtree paints inside one gpui scene layer, with the backdrop blur painted first, structurally under the content:

```rust
use bezel_ui::material;

material::material(Theme::SURFACE_RADIUS, material::MENU_BLUR, card)
```

The corner radius must match the card's own rounding — the blur is cut to the radius you pass, and a mismatch frosts square corners outside a round border.

One layer is the point. With per-primitive bounds-tree ordering a hover repaint elsewhere can reassign the card's quads relative to its siblings; inside a single layer the card's stacking is structural. The cost is that everything in the card shares one draw order, and equal orders render grouped by primitive kind — quads, then icons, then images — so a close button's circle painted "after" a thumbnail still lands under it. `material::layered` opens a nested layer to restore the intended stacking:

```rust
material::layered(close_button)
```

The blur needs `Window::paint_backdrop_blur` from bezel's gpui fork, and that is macOS Metal only. Elsewhere the primitive is ignored and the glass falls back to the theme's translucent tint over the OS window blur; on an opaque appearance `material` is a pass-through. Gate glass-only recipes on `theme.is_glass()`, never on the platform: the frost alpha is per-appearance, and light chrome is opaque by design.
