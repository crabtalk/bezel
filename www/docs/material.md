---
title: Materials
description: Two backdrop surfaces — a frosted card that blurs what it covers, and liquid glass that refracts it at the rim.
prompt: Put the floating inspector on liquid glass with an accent tint, and keep its close button above the thumbnail.
---

```rust
use theme::{Glass, Material, SurfaceStyle, Theme};
use ui::surface;

surface::of("card", SurfaceStyle::Material(Material::Regular), |theme, _, _| {
    div()
        .rounded(px(Theme::surface_radius()))
        .text_color(theme.text)
        .child("Inspector")
})

surface::of("preview", SurfaceStyle::Glass(Glass::Clear), move |_, _, _| {
    div()
        .rounded(px(Theme::surface_radius()))
        .child(thumbnail)
        .child(surface::layered(close_button))
})
.tint(Theme::of(cx).accent.opacity(0.3))
```

- Give each surface a stable element id. Build its content inside the callback, using the supplied theme for foreground colors.
- Round the styled content; the surface takes its corner radius and clears its background.
- macOS uses Liquid Glass on macOS 26 and native popover materials on older systems. Each foreground lives in its material's content view. System appearance and accessibility settings control native effects.
- Other platforms use their renderer's material capability, with an opaque fill where effects are unavailable. `Surface::optics` is available on renderer backends.
- `surface::group` groups related glass surfaces without merging their shapes. `surface::layered` gives a GPU overlay a separate draw order.
- Menus, dialogs, sheets and tooltips use `theme.popover_surface`.
