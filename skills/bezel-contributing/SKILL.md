---
name: bezel-contributing
description: Work inside the bezel repo — where code goes across the crate layers, the five laws a change has to obey, and everything a component has to land with (gallery row, docs page, tests, the nightly rustfmt import pass). Use for any edit under crates/, apps/ or www/ in this repository, and before opening a PR against crabtalk/bezel.
---

# Working on bezel

`CONTRIBUTING.md` is the full text and wins on any conflict. This is the part an
agent gets wrong.

## Where code goes

```
crates/theme     tokens + appearance      (the @Environment layer)
crates/motion    animation vocabulary
crates/ui        components               (the View layer)
crates/icons     the icon set             (declared from Lucide, no SVGs in git)
crates/markdown  block document model     (pure: doc, parse, serialize, select, edit)
crates/blocks    painted fenced blocks
crates/editor    the editing surface      (keys, IME, undo, anchors, menus)
crates/syntax    tree-sitter highlighting (one answer to a seam, never a feature on it)
crates/bezel     the facade
apps/gallery     the documentation, live
```

Dependencies point downward only — `ui → motion + theme`, `theme → gpui alone`.
A member inherits every dependency (`x.workspace = true`) and never names a
version of its own. Nothing new lands in the facade that every app does not paint
with; `markdown`, `syntax`, `blocks` and `terminal` stay peers a consumer names.

## The five laws, compressed

1. **Style flows through the environment.** No color, font or size parameters.
   The environment supplies defaults; a caller overrides by chaining gpui
   modifiers on what is returned — so never bake style into a child the caller
   cannot reach. The rare exception takes an optional `StyleRefinement` merged last.
2. **SwiftUI vocabulary.** Name a widget for its SwiftUI analog. Stateless paint
   is a catalog trait on `Theme`; stateful is a struct entity whose builders
   configure *content*. A closed enum picks between shipped looks — free-form
   radius, color and padding never become parameters.
3. **Motion is named.** From the `MotionSpec` catalog; phase math lives in
   `motion::phase` and is unit-tested. No inline durations or curves.
4. **Numbers drive layout, colors are paint.** No layout value depends on which
   color is painted. A number reaches `Theme` only when the platform names it;
   a component's own metric stays with the component.
5. **Measured, and dated.** Every number on `Theme` records where it was read and
   when. Where the platform names one value, ship one and no more — inventing the
   rest puts made-up numbers in the file and every later decision rests on them.

Extracted from working application code, never invented ahead of need. If there
is no consumer, it does not land.

## What a change lands with

- **A gallery row.** `apps/gallery/src/lib.rs` `TABS` — key, title, and the source
  path it prints as documentation. `apps/gallery/tests/rail.rs` asserts keys are
  unique and every path exists.
- **A docs page.** `www/docs/<key>.md`, frontmatter `title` + a one-line unique
  `description`. The site indexes prose by rail key, so a row without a page is a
  broken build, and the page is what `llms.txt` and `llms-full.txt` serve to agents.
- **Tests.** Pure logic gets unit tests (`motion::phase`, `markdown` round-trips
  to a fixed point). Prefer a test that proves the artifact works over one that
  proves it looks like markup.
- **Terse docs.** Doc comments say what the thing is and why it is that way. No
  essays.

## Commands

```sh
cargo nextest run                       # the suite — never cargo test
cargo clippy --workspace --all-targets  # CI runs with -D warnings
cargo run -p gallery                    # every component, live
cargo build -p web --lib --target wasm32-unknown-unknown   # CI's third leg
```

After touching imports, stable rustfmt cannot group them per crate
([rustfmt#4991](https://github.com/rust-lang/rustfmt/issues/4991)):

```sh
rustfmt +nightly --edition 2024 --config imports_granularity=Crate <files>
cargo fmt --all
```

CI is fmt + clippy on Linux, `nextest --no-fail-fast` on macOS *and* Linux, and a
wasm build. Linux is the platform with no lens that must still pass; wasm is where
`std::time::Instant` panics and a missing renderer feature first shows up.

## Pull requests

One-line Conventional Commits subject (`feat(editor): tab indents and zoom
support`) — no body, no trailers, no AI attribution. PR body is a few lines or
short bullets. Branch, never push to `main`.
