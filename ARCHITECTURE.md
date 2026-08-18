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
crates/markdown  block document model     (markdown in, markdown out; +editor)
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

## The markdown model

`bezel-markdown` is a **flat list of blocks with an indent level**, not a
nested tree — Notion's shape rather than CommonMark's. The reason is editing:
on a flat list, Enter splits, Backspace merges and Tab indents, all list
operations; on a tree, "the previous block" is a traversal and every edit is a
restructure. Inline formatting is a list of marks over byte ranges rather than
flags on a run, because an editor has to *map* marks through insertions, and
because flags lose nesting order — under flags `**_x_**` and `_**x**_` are the
same value.

Markdown is the wire form, so `parse` and `serialize` are inverses up to a
**fixed point**: parse, serialize, parse again, and the document is unchanged.
That is what an edit/save cycle needs and it is what the tests enforce, over a
canonical corpus and 20,000 generated documents. Byte-identical round tripping
is deliberately *not* promised — a flat model cannot represent arbitrarily
nested CommonMark, and neither can Notion.

`doc`, `parse`, `serialize` and `edit` are pure — no gpui, no painting. `render`
is the gpui layer over them: a flat block list means nesting is left padding
rather than nested containers, and the gap between two blocks is decided by the
pair, so items of one list sit tight while a new list gets air.

`editor` is the editing **surface** — a focus handle, key bindings, the platform
input handler, and turning a click into an offset — behind the `editor` feature,
off by default. It was a `bezel-editor` crate until its manifest said otherwise:
its dependencies were a strict subset of this crate's, so the boundary isolated
nothing while cutting one feature in half, with *what an edit does* on one side
and *which key does it* on the other. The feature gates compile time and API
surface; it does not lighten the graph, because there is no dependency here that
a reader does not already carry.

## Laws

1. **Style flows through the environment.** Components read `Theme::of(cx)`
   (a gpui `Global`) at paint time — SwiftUI's `@Environment`. No color,
   font, or size parameters on component functions. The environment supplies
   *defaults*, not verdicts: a caller overrides any style by chaining the
   standard gpui modifiers on the returned element
   (`widgets::group_box(&theme).rounded(px(4.0))`). Components must never bake
   style into a child the caller cannot reach; the rare function that needs
   caller-supplied styling takes an optional `StyleRefinement` and merges it
   last (see `ghost_hover`).
2. **SwiftUI vocabulary.** Widgets are named for their SwiftUI analog:
   `toggle`, `divider`, `group_box`, `material`, `button_prominent`,
   `redacted_rows`. Stateless paint is a catalog trait on `Theme` — import
   the group (`use bezel_ui::widgets::Scaffolding;`) and reach it as
   `theme.group_box()`. Stateful components are struct entities (`Table`,
   `TextField`, `Orb`). Builder methods configure *content* — an orb's
   state, a table's columns — never style. A closed variant enum
   (`ButtonStyle`) selects between shipped looks; free-form values (radius,
   color, padding) never become parameters.
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
