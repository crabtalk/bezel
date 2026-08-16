# Architecture

bezel is a gpui component library with SwiftUI's architecture and shadcn's
spirit: a small set of layered crates you build real apps on, extracted from
working application code — never invented ahead of need.

## Layout

```
crates/bezel     the facade               (one dependency, peer namespaces)
crates/theme     tokens + appearance      (the @Environment layer)
crates/motion    animation vocabulary     (the Animation/transition layer)
crates/ui        components               (the View layer)
apps/gallery     the documentation — a rail of every component, live
```

Library crates live in `crates/`, binaries in `apps/`. Each crate depends
only downward: `ui → motion + theme`, `motion → (theme in tests)`, `theme →
gpui alone`.

The layers are separate crates because there are real consumers of each
alone: the token system is useful to anyone writing their own gpui
components, and the coming `markdown`/`syntax`/`terminal` crates need tokens
without pulling in popovers.

**The `bezel` facade re-exports each layer as a peer namespace** —
`bezel::theme`, `bezel::motion`, `bezel::ui` — plus `bezel::gpui`. That last
one is why it exists early: a consumer declaring its own `gpui = "0.2.2"`
beside bezel can end up with a second copy in the graph, and going through
`bezel::gpui` makes that impossible. A test in the facade pins the guarantee —
it type-annotates values from every layer as `bezel::gpui` types, so a split
graph stops compiling instead of producing a window that paints shapes but no
text.

It carries **no feature flags yet**. Features were the original reason to
build it: gating `markdown` (pulldown-cmark), `syntax` (28 tree-sitter
grammars) and `terminal` (alacritty) so nobody compiles a grammar to get a
button. Those crates do not exist, and a feature that gates nothing is
machinery for its own sake — they arrive together. Depending on a single layer
directly stays supported: the token system alone is useful to anyone writing
their own gpui components.

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

`TODO.md` holds the checklist. The policy it follows: a new crate appears in
`crates/` only when the component is real, and heavier layers (markdown,
syntax, terminal) arrive as their own crates rather than swelling `ui` — so a
consumer never compiles a tree-sitter grammar to get a button.
