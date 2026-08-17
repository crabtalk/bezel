---
title: Typography
description: Geist and Geist Mono embedded in the crate, registered with the gpui text system in one call, named on the theme as two families.
---

Both families are bundled in `bezel-ui` and registered with the text system at boot:

```rust
bezel_ui::register_fonts(cx).ok();
```

Failure is non-fatal — the theme's `font_sans_fallback` / `font_mono_fallback` name the system faces, so text still paints if registration fails. Everything else reads the family off the theme:

```rust
div().font_family(theme.font_mono.clone())
```

Five files ship, not two: the variable Geist and Geist Mono, plus static Medium, SemiBold and Bold. gpui's cosmic-text path rasterizes a variable font at its default instance only and never applies `wght`, so on Linux every weight above 400 would silently paint at 400 with just the variable file registered. CoreText applies the axis natively and never falls through to the statics.

Sizes are not a scale on the theme. The library paints between 10px and 16px — mostly in half-point steps — and each site names the size it wants, rather than reaching a number it already knows through a token.
