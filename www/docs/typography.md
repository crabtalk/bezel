---
title: Typography
description: Geist and Geist Mono bundled behind a feature gate, registered with the gpui text system in one call, named on the theme as two families — and how to put your own type in their place.
---

```rust
ui::register_fonts(cx).ok();

div().font_family(theme.font_mono.clone())
```

Failure is non-fatal: `font_sans_fallback` / `font_mono_fallback` name the system faces, so text still paints.

## Your own type

```rust
use std::borrow::Cow;

static INTER: &[u8] = include_bytes!("../assets/Inter.ttf");

cx.text_system().add_fonts(vec![Cow::Borrowed(INTER)]).ok();
theme::set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.font_sans = "Inter".into();
    theme
}, cx);
```

The string is the family name the file declares, not a path. Go through `set_palette` — a light/dark switch rebuilds the palette, and only the registered builder is rerun.

## Feature gates

```toml
bezel = "0.1"
bezel = { version = "0.1", features = ["geist-mono"] }
```

| | |
| --- | --- |
| `geist-sans` | Variable Geist, 165 KB. |
| `geist-mono` | Variable Geist Mono, 168 KB. |
| `geist-weights` | Static Medium, SemiBold and Bold, 375 KB. Implies `geist-sans`. |

All three are off by default, and unasked `register_fonts` registers nothing and still returns `Ok`. The statics exist because gpui's cosmic-text path rasterizes a variable font at its default instance only — on Linux every weight above 400 would otherwise paint at 400. CoreText applies the axis natively and never falls through to them.

Sizes are not a scale on the theme: the library paints between 10px and 16px, and each site names the size it wants.
