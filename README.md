# bezel

A gpui component library with a SwiftUI-shaped architecture: style flows
through the environment, never through parameters.

- **`crates/theme`** — the design tokens. One flat `Theme` struct installed as
  a gpui `Global` (the `@Environment` analog), read via `Theme::of(cx)` at
  paint time. Designed light + dark palettes with WCAG-verified contrast
  pairing, oklch color math, and system appearance switching
  (`theme::appearance`).
- **`crates/motion`** — the animation vocabulary. Exact CSS `cubic-bezier`
  evaluation, a named `MotionSpec` catalog (`FADE_IN`, `MENU_IN`, `PULSE`, …),
  a shared 30fps pulse clock for loaders, and a hover-fade system. The pure
  phase math lives in `motion::phase`.
- **`crates/ui`** — the components: popover/menu/dialog mounts, buttons,
  `toggle`, `divider`, `group_box`, spinners, embedded SVG icons, and the
  `material` float (SwiftUI's `.ultraThinMaterial` analog).
- **`apps/gallery`** — every component rendered in a real window
  (`cargo run -p gallery`).

gpui comes from [crabtalk/zed](https://github.com/crabtalk/zed), our fork of
zed (gpui's home repo), pinned by rev. The crates.io gpui release is months
behind the API this code targets, so a git pin is required; the fork also
hosts any gpui patches we carry.

## Configuration

There is no config object and no registry: bezel is configured through gpui's
own globals and by choosing which `init` functions to call. Every `init` below
is optional. The one thing that is **not** optional is two lines in your root
render:

```rust
if bezel_motion::hover_fades_active() {
    window.request_animation_frame();
}
```

Hover washes are colours computed at paint time, not animation elements that
drive themselves — so the frames a fade needs are the app's to ask for. Skip it
and every hover wash paints one frame at rest and then sticks until something
else repaints.

**Theme.** `Theme` is a plain struct with public fields, installed as a gpui
`Global`. To ship your own colours, say how a palette is built and register it
before `appearance::init`:

```rust
fn palette(appearance: Appearance) -> Theme {
    let mut theme = Theme::for_appearance(appearance);
    theme.accent = my_brand_accent(appearance);
    theme
}
theme::set_palette(palette, cx);
```

Registering the *builder* rather than a palette is what makes the colours
survive a light/dark switch — the appearance switch rebuilds the palette, and it
now rebuilds yours. For a one-shot palette that does not need to survive that,
`Theme::install_custom(theme, cx)` installs one directly.

Use either of those rather than `cx.set_global(theme)`. The context-free paint
helpers (`ink`, `hairline`, `wash`) read a process-wide appearance mirror rather
than the global — they are called from element builders with no `cx` in scope —
and setting the global alone leaves them painting for the wrong appearance.

**Key bindings.** `bezel_ui::input::init(cx)` installs a default keymap scoped
to the `TextField` key context. It is a convenience — every action is a public
type, so an app that wants a different keymap skips `init` and binds the actions
itself. bezel deliberately claims few chords: a component library that binds a
keystroke it is not sure about takes it away from every app downstream.

`bezel_ui::combobox::init(cx)`, `bezel_ui::date::init(cx)` and
`bezel_ui::menubar::init(cx)` do the same for the components that own a keyboard
of their own — list navigation, the calendar's day/week/month arrows, and the
bar's rows and menus.

`bezel_ui::focus::init(cx)` is the other one: `tab`/`shift-tab` to walk the
controls, `enter`/`space` to press the focused one, `←`/`→` to move one that
holds a value. Traversal needs two more lines — `focus::traversal` on the root
element, because moving focus needs a `Window` an app-level handler never sees,
and `focus::focusable(&theme, &handle, control)` around each control. The handle
is the app's: bezel's widgets are `fn(&Theme, ..) -> Div` with nowhere to keep
one, and the state saying whether a checkbox is checked already lives with the
caller.

**Motion.** `bezel_motion::set_speed(scale)` stretches every timeline in the
catalog — `10.0` slows the 200ms pane tweens to 2s, which is how you sample an
animation frame by frame. `set_reduced_motion(cx, true)` is the other switch,
and gpui honours it in every animated element for free.

There are no environment variables anywhere in bezel, deliberately. A knob only
the shell can reach is a knob the app cannot put behind its own setting.

## Provenance & licenses

The initial components were extracted from
[zeronsh/comet](https://github.com/zeronsh/comet) (MIT). Bundled assets:
Solar Icons by 480 Design (CC BY 4.0), Geist and Geist Mono © Vercel Inc.
(SIL Open Font License 1.1).
