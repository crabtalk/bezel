---
title: Color
description: The token set — two designed palettes read from a gpui global at paint time, with light designed rather than inverted.
---

```rust
use theme::Theme;

let theme = Theme::of(cx);
div().bg(theme.surface).text_color(theme.text_muted)
```

`Theme` is a plain struct of `gpui::Hsla` fields installed as a gpui `Global`. Components read it at paint time and never take a colour parameter.

## Installing

```rust
use theme::appearance::{self, AppearanceMode};

appearance::init(AppearanceMode::System, cx);
```

Once at boot, before the first window opens — later than that and the first frame paints in the wrong palette.

## Replacing a token

```rust
theme::set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.danger = my_red(appearance);
    theme
}, cx); // before appearance::init
```

Register the builder rather than installing one theme: `appearance::apply` rebuilds the palette from scratch on every light/dark switch, and a theme installed on its own lasts only until then.

## API

```rust
impl Theme {
    /// The installed palette.
    pub fn of(cx: &App) -> &Theme;

    pub fn for_appearance(appearance: Appearance) -> Self;

    // ...
}
```

```rust
// theme::appearance

/// Once at boot, before the first window opens.
pub fn init(mode: AppearanceMode, cx: &mut App);

/// Subscribes to the OS notification.
pub fn observe_window(window: &mut Window, cx: &mut App) -> Subscription;

/// Changes the preference and repaints.
pub fn set_mode(mode: AppearanceMode, cx: &mut App);

/// Serde-serializable, so you persist it wherever your settings live.
pub enum AppearanceMode { System, Light, Dark }

// ...
```

Light is designed, not inverted — mirroring lightness reverses surface order, elevation and accent contrast. Each light text token lands within ~0.5 of its dark counterpart's ratio, and a test asserts it. `accent` is neutral by default: a library that ships a hue puts that hue in every app that installs it.
