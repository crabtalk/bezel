---
title: Color
description: The token set — two designed palettes read from a gpui global at paint time, with light designed rather than inverted.
prompt: Add a Light / Dark / System setting that persists, and make our error red a little warmer in both modes.
---

```rust
use theme::{AppExt as _, Theme, appearance::{self, AppearanceMode}};

cx.set_palette(|appearance| {                    // optional: your own tokens
    let mut theme = Theme::for_appearance(appearance);
    theme.danger = my_red(appearance);
    theme
});
appearance::init(saved_mode, cx);                // once, before the first window

let theme = Theme::of(cx);
div().bg(theme.surface).text_color(theme.text_muted)
```

- `appearance::init` runs once at boot, before any window opens; later, the first frame paints the wrong palette.
- Change tokens through `set_palette`. A light/dark switch rebuilds from the registered builder, so a palette installed with `Theme::install_custom` lasts only until then. Registered after `init`, follow it with `appearance::apply(cx)`.
- Under `AppearanceMode::System`, each window needs `appearance::observe_window(window, cx).detach()` to follow the OS.
- `AppearanceMode` is serde-serializable; persisting it is the app's job. `cx.set_appearance_mode(mode)` changes it at runtime.
