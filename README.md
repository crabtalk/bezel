# bezel

[![crates.io](https://img.shields.io/crates/v/bezel.svg?style=flat-square)](https://crates.io/crates/bezel)
[![license](https://img.shields.io/crates/l/bezel.svg?style=flat-square)](LICENSE)
[![gpui](https://img.shields.io/crates/v/bezel-gpui.svg?style=flat-square&label=gpui)](https://crates.io/crates/bezel-gpui)

A gpui component library, SwiftUI-lean: style flows through the environment,
never through parameters.

https://github.com/user-attachments/assets/ba8a8946-edd9-4bc9-9fd2-a39fa249f495

```rust
use bezel::ui::widgets::{ButtonStyle, Buttons};
theme.button("Save", ButtonStyle::Prominent, None)
```

## Install

```toml
[dependencies]
bezel = "0.1"
```

An app also names the two crates the facade cannot cover — `gpui` because
`actions!` expands to literal `gpui::` paths, and `gpui_platform` because the
facade re-exports gpui but not the platform. Both are our fork of gpui,
published under `bezel-gpui*`; the `package` key keeps the `gpui::` paths the
macros and gpui's own docs expect:

```toml
gpui = { package = "bezel-gpui", version = "0.3" }
gpui_platform = { package = "bezel-gpui-platform", version = "0.3", features = ["font-kit"] }
```

## Theme

Most apps want the shipped palette in their own hues. That is a `Brand` — one
hue for the greys, one for the accent, one base radius:

```rust
use bezel::theme::{self, Brand, Tint};

// before appearance::init
theme::set_brand(
    Brand {
        tint: Tint::new(257.417, 0.046),
        accent: Tint::new(276.935, 0.182),
        radius: 8.0,
    },
    cx,
);
```

Lightness is not a knob, so a branded palette keeps the contrast ratios the
shipped one was verified at. The gallery's **Create** page is this struct with
sliders on it, and prints the call back.

For colors a hue rotation cannot reach, register a palette builder instead — it
runs first, and a brand rotates whatever it returns:

```rust
use bezel::theme::{Theme, set_palette};

theme::set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.danger = my_red(appearance);
    theme
}, cx);
```

## Build an app

`apps/hello` is the smallest consumer — one window, a button, a toggle.
Every gpui *type* path goes through `bezel::gpui`, so a second gpui cannot
creep into the graph. The bootstrap, which is the part no snippet can skip:

```rust
use bezel::gpui::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};
use bezel::theme::{self, Theme};
use bezel::ui;

fn main() {
    gpui_platform::application()
        .run(|cx: &mut App| {
            if let Err(err) = ui::register_fonts(cx) {
                eprintln!("FONT REGISTRATION FAILED: {err:?}");
            }
            theme::appearance::init(theme::appearance::AppearanceMode::System, cx);
            let bounds = Bounds::centered(None, size(px(520.0), px(360.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| {
                    theme::appearance::observe_window(window, cx).detach();
                    cx.new(Hello::new)
                },
            )
            .unwrap();
            cx.activate(true);
        });
}
```

`Hello` is a struct implementing `Render`; its `render` reads the theme with
`Theme::of(cx)` and builds elements through the `widgets` traits. Run
`cargo run -p hello` and read `apps/hello/src/main.rs` for the rest.

## Provenance

| What                   | From                               | License     |
| ---------------------- | ---------------------------------- | ----------- |
| Initial components     | [comet]                            | MIT         |
| Thinking orbs          | [gpui-thinking-orbs]               | MIT         |
| Blob avatars           | [blobatar]                         | MIT         |
| Syntax highlighting    | tree-sitter core and grammars      | MIT         |
| TypeScript/TSX queries | [nvim-treesitter]                  | Apache-2.0  |
| Icons                  | Solar Icons by 480 Design          | CC BY 4.0   |
| Fonts                  | Geist and Geist Mono © Vercel Inc. | SIL OFL 1.1 |

[gpui]: https://github.com/zed-industries/gpui
[comet]: https://github.com/zeronsh/comet
[gpui-thinking-orbs]: https://github.com/FrancoEscob/gpui-thinking-orbs
[blobatar]: https://github.com/Alain00/blobatar
[nvim-treesitter]: https://github.com/nvim-treesitter/nvim-treesitter
