---
title: Typography
description: Geist and Geist Mono bundled behind a feature gate, registered with the gpui text system in one call, named on the theme as two families — and how to put your own type in their place.
prompt: Use Geist for the UI and bump the base text size to 14.
---

```rust
// bezel = { version = "0.2", features = ["geist-sans", "geist-mono"] }
use theme::{AppExt as _, TextStyle, Theme, Typeset as _};

ui::register_fonts(cx).ok();
cx.set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.font_sans = "Geist".into();
    theme
});
cx.set_base_text_size(14.0);

div().font_family(theme.font_mono.clone()).text_style(TextStyle::Callout)
```

- Registering a face does not select it. `font_sans`, `font_body` and `font_mono` name the family the file declares; until set, the platform's own faces paint.
- Your own font files go through `ui::fonts::add`. In a browser, a face given to `cx.text_system().add_fonts` directly paints text but is missing from SVG.
- `geist-weights` adds static 500/600/700 Geist faces for the cosmic-text (Linux) path, which paints a variable font at 400. Geist Mono has no statics.
- `set_base_text_size` moves every `TextStyle` role and the metrics keyed to it together.
