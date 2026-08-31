# Contributing

bezel is a gpui component library with SwiftUI's architecture and shadcn's
spirit: layered crates you build real apps on, extracted from working
application code — never invented ahead of need.

## Layers

```
crates/bezel     the facade               (one dependency, peer namespaces)
crates/theme     tokens + appearance      (the @Environment layer)
crates/motion    animation vocabulary     (the Animation/transition layer)
crates/ui        components               (the View layer)
crates/markdown  block document model     (markdown in, markdown out; painted)
crates/blocks    painted fenced blocks    (one feature per block)
crates/editor    the editing surface      (keys, IME, undo, anchors, menus)
apps/gallery     the documentation — a rail of every component, live
```

Libraries in `crates/`, binaries in `apps/`, each depending only downward:
`ui → motion + theme`, `theme → gpui alone`. They are separate crates because
each has consumers alone — the token system is useful to anyone writing their
own gpui components.

The facade re-exports every layer as a peer namespace plus `bezel::gpui`, which
is the reason it exists early: a consumer naming its own `gpui` can end up with
a second copy in the graph, and two gpuis are two type universes — at worst a
window that paints shapes but no text, one text system holding the fonts while
the other draws the frame. A test there type-annotates values from every layer
as `bezel::gpui` types, so a split graph stops compiling.

It carries only the layers every app paints with. `markdown`, `syntax`,
`blocks` and `terminal` are peers a consumer names itself: re-exporting `syntax`
made every consumer compile seven C grammars to get a button and broke
`wasm32-unknown-unknown` outright, where that C has no libc. Highlighting is a
seam (`markdown::set_highlighter`), so tree-sitter is one answer to it rather
than the answer; painting a fence is the same shape
(`markdown::set_block_renderer`, answered by `blocks`). Neither is a feature on
the seam crate, because cargo unions features across a graph: one a dependency
turns on is one no consumer can turn off. `gpui_platform` is carried under a
`platform` feature, off by default — a wasm consumer reaching for the browser
backend and a library opening no window both turn it off. The `geist-*` features
are one per bundled face, on by default and forwarded to `ui`.

## Laws

1. **Style flows through the environment.** Components read `Theme::of(cx)` (a
   gpui `Global`) at paint time — SwiftUI's `@Environment`. No color, font or
   size parameters. The environment supplies *defaults*: a caller overrides any
   of it by chaining gpui modifiers on the returned element
   (`group_box(&theme).rounded(px(4.0))`), so never bake style into a child the
   caller cannot reach. The rare function needing caller-supplied style takes an
   optional `StyleRefinement` and merges it last (`ghost_hover`).
2. **SwiftUI vocabulary.** Widgets are named for their SwiftUI analog —
   `toggle`, `divider`, `group_box`, `material`. Stateless paint is a catalog
   trait on `Theme`: import the group (`use ui::widgets::Scaffolding;`) and
   reach it as `theme.group_box()`. Stateful components are struct entities
   (`Table`, `TextField`, `Orb`), whose builders configure *content* — an orb's
   state, a table's columns. A closed enum (`ButtonStyle`) selects between
   shipped looks; free-form radius, color and padding never become parameters.
3. **Motion is named.** Every animation comes from the `MotionSpec` catalog in
   `motion`; pure phase math lives in `motion::phase` and is unit-tested. No
   inline durations or curves.
4. **Numbers drive layout, colors are paint.** No layout depends on which color
   is painted. A number reaches `Theme` when the platform names it; a
   component's own metric stays with the component, and a document's rhythm
   stays with the document.
5. **Measured, and dated.** Every number on `Theme` records where it was read
   and when — the type ladder from `NSFont.preferredFont(forTextStyle:)`, the
   8pt sibling gap from `NSStackView().spacing`, both macOS 26 on 2026-08-31.
   Where the platform names one value we ship one and no more: inventing the
   rest puts made-up numbers in the file and every later decision rests on them.
   A default carries what it can, so an ordinary call site writes no number at
   all — `ui::stack::row()` is the system gap, and `.gap(px(12.0))` is a
   deviation the way `VStack(spacing: 12)` is one.

## The markdown model

A **flat list of blocks with an indent level** — Notion's shape rather than
CommonMark's tree. The reason is editing: on a flat list Enter splits, Backspace
merges and Tab indents, all list operations, where on a tree every edit is a
restructure. Inline formatting is marks over byte ranges, because an editor has
to *map* marks through insertions and because flags lose nesting order (under
them `**_x_**` and `_**x**_` are one value).

Markdown is the wire form, so `parse` and `serialize` are inverses up to a
**fixed point**: parse, serialize, parse again, unchanged — enforced over a
canonical corpus and 20,000 generated documents. Byte-identical round tripping
is deliberately not promised, because a flat model cannot hold arbitrarily
nested CommonMark and neither can Notion.

A position is `(block, part, offset)` — `select::Cursor`. The **part** is a
coordinate rather than a path, which keeps the model flat while a caret still
reaches inside a fence or a table cell: a block has one kind of part and never a
mix, so the three fields order lexicographically and a selection is two of them.
That ordering is what makes `Doc::replace(Selection, Text)` **the** mutation —
typing, backspace, delete, cut and paste are all it with a different argument.

`doc`, `parse`, `serialize`, `select` and `edit` are pure — no gpui, no
painting. `render` is the gpui layer over them.

## The editing surface

`editor` is a focus handle, key bindings, the platform input handler, undo and
the menus — *which key does it* against `markdown`'s *what an edit does*. Its
chords are `ui::TextField`'s, and so are its rules: vertical motion is geometry
through the painted layouts rather than arithmetic on line numbers, and undo
coalesces by adjacency rather than by a pause, so there is no timing threshold
to invent. `history` holds whole-document snapshots, which is why comment
anchors sit beside them: an undo restores a whole document, leaving no delta an
app could map its own copy of a range through.

## Dependencies

gpui comes from crates.io as `bezel-gpui`, published from our fork
[crabtalk/zed](https://github.com/crabtalk/zed) with the whole crate closure
renamed. The workspace renames it back —
`gpui = { package = "bezel-gpui", version = "0.3" }` — so the `gpui::` paths the
macros expect still resolve and a consumer writes plain registry dependencies.
`gpui`, `gpui_platform` and `gpui_web` must resolve to the **same** version.

Everything else is inherited from `[workspace.dependencies]`; a member writes
`x.workspace = true` and never a version of its own.

## Working on it

```sh
cargo nextest run                 # the suite
cargo clippy --workspace --all-targets
cargo run -p gallery              # every component, live
```

Imports are grouped per crate, which stable rustfmt cannot enforce
([rustfmt#4991](https://github.com/rust-lang/rustfmt/issues/4991)) — after
touching them run `rustfmt +nightly --edition 2024 --config
imports_granularity=Crate <files>`, then `cargo fmt --all`.

A component lands with its gallery page: the rail is the documentation, and a
test asserts every row names a source file that exists.
