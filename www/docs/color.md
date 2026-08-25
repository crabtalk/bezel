---
title: Color
description: The token set — two designed palettes read from a gpui global at paint time, with light designed rather than inverted.
---

`Theme` is a plain struct of `gpui::Hsla` fields installed as a gpui `Global`. Components read it at paint time and never take a color parameter:

```rust
use theme::Theme;

let theme = Theme::of(cx);
div().bg(theme.surface).text_color(theme.text_muted)
```

Install it once at boot, before the first window opens — later than that and the first frame paints in the wrong palette:

```rust
use theme::appearance::{self, AppearanceMode};

appearance::init(AppearanceMode::System, cx);
```

`AppearanceMode` is `System`, `Light` or `Dark`, and it is serde-serializable so you persist it wherever your settings live. `appearance::observe_window(window, cx)` subscribes to the OS notification; `appearance::set_mode` changes the preference and repaints.

Light is designed, not inverted. Mirroring lightness gets three things backwards: surface order (dark's content panel is the *darkest* plane, light's is white and the chrome goes grey), elevation (a faint white wash means "raised" on dark and "recessed" on light), and accents (the 400-level tones fall below 4.5:1 on white, so light uses the 600-level siblings at the same hue). Each light text token lands within ~0.5 of its dark counterpart's contrast ratio, and a test in the crate asserts it.

`accent` is neutral by default. A library that ships a hue puts that hue in every app that installs it, so the default is the accent's lightness with the chroma at zero. Branding it is a [Brand](/docs/create), which rotates hue without moving any of the lightnesses above.

`set_palette` is the way in for colours a hue rotation cannot reach — a retuned `danger`, a wholesale replacement. Register the builder rather than installing one theme: `appearance::apply` rebuilds the palette from scratch on every light/dark switch, and a theme installed on its own lasts only until then.

```rust
theme::set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.danger = my_red(appearance);
    theme
}, cx); // before appearance::init
```
