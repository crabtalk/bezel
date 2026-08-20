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
crates/markdown  block document model     (markdown in, markdown out; painted)
crates/editor    the editing surface       (keys, IME, undo, menus)
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

**It carries only the layers every app paints with.** `markdown`, `syntax` and
`terminal` are peer crates a consumer names itself, and layer gating never
needed to arrive because each is an *implementation* behind a seam already open:
`markdown::set_highlighter` takes any `fn(&str, &str) -> Option<Vec<(Range,
HighlightKind)>>`, so tree-sitter is one answer to highlighting and not the
answer. Re-exporting `syntax` here made every consumer compile seven C grammars
to get a button, and made the facade fail outright for
`wasm32-unknown-unknown` — where that C has no libc, which is the very failure
`markdown` names no highlighter to avoid. A gate would have hidden that behind a
default; not carrying the crate removes it.

Its only feature flags are one per bundled face — `geist-sans`, `geist-mono`,
`geist-weights` — all on by default and forwarded to `ui`: an app that registers
its own typeface points `Theme::font_sans` at it and drops the Geist it no
longer paints. `ui` is also the one dependency spelled out rather than
inherited, because cargo will not let a member clear a workspace dependency's
default features. Depending on a single layer directly stays supported: the
token system alone is useful to anyone writing their own gpui components.

## The markdown model

`markdown` is a **flat list of blocks with an indent level**, not a
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

A position is `(block, part, offset)` — `select::Cursor`. The **part** is a
coordinate rather than a path, which is what keeps the model flat while a caret
still reaches inside a code block or a table cell: a block has one kind of part
and never a mix, so the three fields order lexicographically and a selection is
just two of them with `min`/`max` deciding which end is which. Every editable
region is a `Text`, code included, so one accessor and one edit path cover the
document.

That ordering is what makes `Doc::replace(Selection, Text)` **the** mutation.
Typing, backspace, delete, cut and paste are all that call with a different
argument, so none of them has to know whether the selection was empty, spanned
two paragraphs, or swallowed a table on the way past. The property test runs it
over generated documents: an editor that can reach a state its own serializer
cannot express corrupts the file on save, and no amount of UI polish recovers.

`doc`, `parse`, `serialize`, `select` and `edit` are pure — no gpui, no
painting. `render` is the gpui layer over them: a flat block list means nesting
is left padding rather than nested containers, and the gap between two blocks is
decided by the pair, so items of one list sit tight while a new list gets air.

## The editing surface

`editor` is a focus handle, key bindings, the platform input handler, undo, and
the menus. The boundary is *what an edit does* against *which key does it*, and
the dependency graph draws the same line: `ui` is the slash menu's
`popover::Filter` and `menu_at`, and it stops at this crate — a consumer that
only paints markdown compiles no popovers.

Its chords are `ui::TextField`'s, because a document is not the place to invent
a second set, and its rules are that field's too: vertical motion is geometry
through the painted layouts rather than arithmetic on line numbers, and undo
coalesces by *adjacency rather than by a pause*, so there is no timing threshold
to invent. `history` holds whole-document snapshots — a transaction log is the
machinery collaborative editing needs and nothing here asks for it.

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
   the group (`use ui::widgets::Scaffolding;`) and reach it as
   `theme.group_box()`. Stateful components are struct entities (`Table`,
   `TextField`, `Orb`). Builder methods configure *content* — an orb's
   state, a table's columns — never style. A closed variant enum
   (`ButtonStyle`) selects between shipped looks; free-form values (radius,
   color, padding) never become parameters.
3. **Motion is named.** Every animation comes from the `MotionSpec` catalog
   in `motion`; pure phase math lives in `motion::phase` and is
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

A new crate appears in `crates/` only when the component is real, and heavier
layers (markdown, syntax, terminal) arrive as their own crates rather than
swelling `ui` — so a consumer never compiles a tree-sitter grammar to get a
button.
