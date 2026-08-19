# bezel

A gpui component library, SwiftUI-lean: style flows through the environment,
never through parameters.

https://github.com/user-attachments/assets/6b5f16d0-9f58-48d6-8398-09acb1afa402

```rust
use bezel::ui::widgets::{ButtonStyle, Buttons};
theme.button("Save", ButtonStyle::Prominent, None)
```

## Theme

To ship your own colors, register a custom palette:

```rust
use bezel::theme::{Theme, set_palette};

// before appearance::init
theme::set_palette(|appearance| {
    let mut theme = Theme::for_appearance(appearance);
    theme.accent = my_brand_accent(appearance);
    theme
}, cx);
```

One thing is not optional if you use hover fades — ask for frames in your root
render, or fades paint once and stick:

```rust
if bezel::motion::hover_fades_active() {
    window.request_animation_frame();
}
```

## Build an app

`apps/hello` is the smallest consumer — one window, a button, a toggle.
Every gpui *type* path goes through `bezel::gpui`, so a crates.io gpui cannot
creep into the graph; the two dependencies beyond that are both from the same
fork rev: `gpui` (for `actions!`, which expands to literal `gpui::` paths)
and `gpui_platform` (the facade re-exports gpui but not the platform, and
this gpui has no `Application::new()`). The bootstrap, which is the part no
snippet can skip:

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

The initial components were extracted from [comet][comet](MIT).
The thinking orbs were ported from [gpui-thinking-orbs][gpui-thinking-orbs] (MIT).
The blob avatars were ported from [blobatar][blobatar] (MIT).
Syntax highlighting: tree-sitter core and grammars (MIT), TypeScript/TSX queries from [nvim-treesitter][nvim-treesitter] (Apache-2.0).
Bundled assets: Solar Icons by480 Design (CC BY 4.0), Geist and Geist Mono © Vercel Inc. (SIL OFL 1.1).

[gpui]: https://github.com/zed-industries/gpui
[comet]: https://github.com/zeronsh/comet
[gpui-thinking-orbs]: https://github.com/FrancoEscob/gpui-thinking-orbs
[blobatar]: https://github.com/Alain00/blobatar
[nvim-treesitter]: https://github.com/nvim-treesitter/nvim-treesitter
