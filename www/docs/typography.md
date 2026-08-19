---
title: Typography
description: Geist and Geist Mono bundled behind a feature gate, registered with the gpui text system in one call, named on the theme as two families — and how to put your own type in their place.
---

Both families are bundled in the `ui` crate and registered with the text system at boot:

```rust
ui::register_fonts(cx).ok();
```

Failure is non-fatal — the theme's `font_sans_fallback` / `font_mono_fallback` name the system faces, so text still paints if registration fails. Everything else reads the family off the theme:

```rust
div().font_family(theme.font_mono.clone())
```

Five files ship, not two: the variable Geist and Geist Mono, plus static Medium, SemiBold and Bold. gpui's cosmic-text path rasterizes a variable font at its default instance only and never applies `wght`, so on Linux every weight above 400 would silently paint at 400 with just the variable file registered. CoreText applies the axis natively and never falls through to the statics. The three cover the sans only — Geist Mono ships as its variable file alone, so bold monospace still paints at 400 off CoreText.

Each of the three is its own gate, so an app pays for the faces it paints and no more: `geist-sans` is the variable Geist at 165 KB, `geist-mono` the variable Geist Mono at 168 KB, `geist-weights` the three statics at 375 KB. All are on by default. `geist-weights` implies `geist-sans`, since it is that family's weights — a macOS-only build can drop it, and a terminal app that never paints proportional text can take `geist-mono` alone.

Your own type goes in through the same two seams, because there is nothing Geist-specific about either. Bytes go to the gpui text system, which takes any font; the theme names which family the components then paint with:

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

The string is the family name the file itself declares, not a path — the text system resolves it, and a name nothing registered falls through to the fallback. Go through `set_palette` rather than mutating the installed theme: a light/dark switch rebuilds the palette from scratch, and only the registered builder is rerun.

An app that brings its own type can then stop paying for ours, in whole or per family:

```toml
bezel = { version = "0.0.2", default-features = false }
bezel = { version = "0.0.2", default-features = false, features = ["geist-mono"] }
```

The facade forwards all three gates. With none of them `register_fonts` registers nothing, it still returns `Ok`, and the fallback families are all that paints.

Sizes are not a scale on the theme. The library paints between 10px and 16px — mostly in half-point steps — and each site names the size it wants, rather than reaching a number it already knows through a token.
