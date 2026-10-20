---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
prompt: Put the floating inspector on liquid glass with an accent tint, and keep its close button above the thumbnail.
---

```rust
use theme::{Glass, Material, SurfaceStyle, Theme};
use ui::surface::{self, Surfaced as _};

div()
    .rounded(px(Theme::surface_radius()))
    .surface(&theme, SurfaceStyle::Material(Material::Regular))

div()
    .rounded(px(Theme::surface_radius()))
    .child(thumbnail)
    .child(surface::layered(close_button))
    .surface(&theme, SurfaceStyle::Glass(Glass::Clear))
    .tint(theme.accent.opacity(0.3))
```

- Round the element before `.surface`: the blur and the lens are cut to its own corner radius. `.surface` clears the element's `bg`.
- A surface is one scene layer, where equal draw orders render grouped by primitive kind (quads, then icons, then images). Wrap an overlay in `surface::layered` to keep it on top.
- The lens needs `Window::paint_backdrop_blur` from bezel's gpui fork (Metal and wgpu). Elsewhere, or with `theme.glass` off, the surface paints a flat tint. Gate glass-only code on `surface::lensed(&theme)`.
- Menus, dialogs, sheets and tooltips mount on `theme.popover_surface`.
