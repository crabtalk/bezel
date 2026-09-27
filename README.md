# bezel

[![crates.io](https://img.shields.io/crates/v/bezel.svg?style=flat-square)](https://crates.io/crates/bezel)
[![license](https://img.shields.io/crates/l/bezel.svg?style=flat-square)](LICENSE)
[![gpui](https://img.shields.io/crates/v/bezel-gpui.svg?style=flat-square&label=gpui)](https://crates.io/crates/bezel-gpui)
[![discord](https://img.shields.io/discord/1481168707391852659?style=flat-square&label=discord&logo=discord&logoColor=white&color=5865F2)](https://discord.com/invite/yGZDYnwbx6)

> [!NOTE]
> Bezel is developing fast, with frequent releases and version bumps.
> APIs are still evolving, so expect changes as the library takes shape.

A gpui component library, SwiftUI-lean: style flows through the environment,
never through parameters.

https://github.com/user-attachments/assets/34861f29-004f-47f0-89e6-42cc8772749f

```rust
use bezel::ui::widgets::{ButtonStyle, Buttons};
theme.button("Save", ButtonStyle::Prominent, None)
```

## Built with Bezel

[Cydonia](https://github.com/crabtalk/cydonia) is a desktop workspace for working
with coding agents in a project directory. It's a real app built with Bezel —
explore it to see the library in use beyond the component gallery.

## Install

```toml
[dependencies]
bezel = "0.2"
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

Building on Windows needs the MSVC toolchain, and on Linux the fontconfig and
wayland/x11 headers — see [CONTRIBUTING](CONTRIBUTING.md#working-on-it).

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
shipped one was verified at. The gallery's **Theme** page is this struct with
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

## Agents

Every doc page is served as markdown — append `.md` to any docs URL, or take the
whole corpus from [llms.txt](https://bezel.gallery/llms.txt) and
[llms-full.txt](https://bezel.gallery/llms-full.txt).

[`skills/`](skills) holds two agent skills, plain `SKILL.md` files any agent that
reads the format can load. `skills/bezel` teaches an agent to *use* the library —
the bootstrap, the laws, and the traps that only bite at runtime.
`skills/bezel-contributing` is for working on it.

## Provenance

| What                   | From                               | License     |
| ---------------------- | ---------------------------------- | ----------- |
| Initial components     | [comet]                            | MIT         |
| Thinking orbs          | [gpui-thinking-orbs]               | MIT         |
| Blob avatars           | [blobatar]                         | MIT         |
| Syntax highlighting    | tree-sitter core and grammars      | MIT         |
| Wasm grammar fixtures  | [tree-sitter-json], [tree-sitter-css] | MIT      |
| TypeScript/TSX queries | [nvim-treesitter]                  | Apache-2.0  |
| Icons                  | [Lucide], ported from a release    | ISC         |
| Fonts                  | Geist and Geist Mono © Vercel Inc. | SIL OFL 1.1 |

[gpui]: https://github.com/zed-industries/gpui
[comet]: https://github.com/zeronsh/comet
[gpui-thinking-orbs]: https://github.com/FrancoEscob/gpui-thinking-orbs
[blobatar]: https://github.com/Alain00/blobatar
[nvim-treesitter]: https://github.com/nvim-treesitter/nvim-treesitter
[tree-sitter-json]: https://github.com/tree-sitter/tree-sitter-json
[tree-sitter-css]: https://github.com/tree-sitter/tree-sitter-css
[lucide]: https://lucide.dev
