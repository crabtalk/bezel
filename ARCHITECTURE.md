# Architecture

bezel is a gpui component library with SwiftUI's architecture and shadcn's
spirit: a small set of layered crates you build real apps on, extracted from
working application code — never invented ahead of need.

## Layout

```
crates/theme     tokens + appearance      (the @Environment layer)
crates/motion    animation vocabulary     (the Animation/transition layer)
crates/ui        components               (the View layer)
apps/gallery     the dev surface — every component rendered in a real window
```

Library crates live in `crates/`, binaries in `apps/`. Each crate depends
only downward: `ui → motion + theme`, `motion → (theme in tests)`, `theme →
gpui alone`.

The layers are separate crates because there are real consumers of each
alone: the token system is useful to anyone writing their own gpui
components, and the coming `markdown`/`syntax`/`terminal` crates need tokens
without pulling in popovers.

**A `bezel` facade crate is deferred, not rejected.** The name is reserved on
crates.io. It becomes worth building when the heavy layers land — `markdown`
pulls in pulldown-cmark, `syntax` 28 tree-sitter grammars, `terminal`
alacritty — because then one crate can gate them behind features and nobody
compiles a grammar to get a button. It would re-export each layer as a peer
namespace (`bezel::theme`, `bezel::ui`, …) plus `bezel::gpui`, so consumers
cannot end up with a second copy of gpui in the graph. Until then the three
layer crates are used directly.

## Laws

1. **Style flows through the environment.** Components read `Theme::of(cx)`
   (a gpui `Global`) at paint time — SwiftUI's `@Environment`. No color,
   font, or size parameters on component functions.
2. **SwiftUI vocabulary.** Widgets are named for their SwiftUI analog:
   `toggle`, `divider`, `group_box`, `material`, `button_prominent`,
   `redacted_rows`. Components are plain functions returning gpui elements —
   no component structs, no builder knobs, no style traits. Customization is
   editing the source.
3. **Motion is named.** Every animation comes from the `MotionSpec` catalog
   in `bezel-motion`; pure phase math lives in `motion::phase` and is
   unit-tested. No inline durations or curves in components.
4. **Numbers drive layout, colors are paint.** Layout constants are plain
   numbers on `Theme`; no layout ever depends on which color is painted.

## Dependencies

Crates declare `gpui = "0.2.2"` — a version requirement, because crates.io
rejects a bare git dependency — and `[patch.crates-io]` supplies the real
source. The registry release trails the API we build against by months, so
the patch is what actually compiles.

That patch currently points at a **sibling `../zed` checkout**: our fork of
gpui's home repo, [crabtalk/zed](https://github.com/crabtalk/zed), where the
gpui patches we carry live (first up: `Window::paint_backdrop_blur`, which
`ui::material` needs). A local path is the honest source while those commits
are unpushed — the trade is that a fresh clone needs that sibling checkout.
Once the branch is pushed, the patch becomes `{ git = …, rev = …, version =
"=0.2.2" }` and nothing else changes.

`gpui_platform` (unpublished, gallery only) must resolve to the *same*
checkout. Two copies of gpui in one graph are two incompatible type
universes; the failure is a trait-bound error at best and, at worst, a window
that paints shapes but no text — one text system holding the fonts while the
other draws the frame.

## Roadmap

Next extractions from comet, in order of value: markdown renderer
(streaming, block-incremental), tree-sitter syntax crate + highlight cache,
terminal grid view. Each arrives as its own crate in `crates/` only when the
component is real.
