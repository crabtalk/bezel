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

## Provenance & licenses

The initial components were extracted from
[zeronsh/comet](https://github.com/zeronsh/comet) (MIT). Bundled assets:
Solar Icons by 480 Design (CC BY 4.0), Geist and Geist Mono © Vercel Inc.
(SIL Open Font License 1.1).
