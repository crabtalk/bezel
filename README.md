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
if bezel_motion::hover_fades_active() {
    window.request_animation_frame();
}
```

## Provenance

The initial components were extracted from [comet][comet](MIT).
The thinking orbs were ported from [gpui-thinking-orbs][gpui-thinking-orbs] (MIT).
Bundled assets: Solar Icons by480 Design (CC BY 4.0), Geist and Geist Mono © Vercel Inc. (SIL OFL 1.1).

[gpui]: https://github.com/zed-industries/gpui
[comet]: https://github.com/zeronsh/comet
[gpui-thinking-orbs]: https://github.com/FrancoEscob/gpui-thinking-orbs
