# TODO

Status snapshot and checklist. Policy lives in `ARCHITECTURE.md`; this is what
is actually left to do.

## Needs a pointer to verify

Seen working once, on 2026-08-17, driven offscreen through gpui's
`VisualTestAppContext` and checked against the captured frame. The harness that
did it was deleted afterwards (the gallery is the documentation now), so
nothing re-checks them — treat these as "worked then", not "works":

- [x] Tooltip appears on hover
- [x] Hover card opens and survives the pointer moving into it
- [x] Context menu opens at the click point
- [x] Select opens on click
- [x] Combobox opens, filters as you type, keeps its selected row distinct from
      the keyboard cursor
- [x] Sheet slides in from the right
- [x] Split divider drags, clamps, and lights up

Never verified:

- [ ] Text field typing, word motion (`opt-←/→`), line kill (`cmd-backspace`)
- [ ] Palette on `⌘K`
- [ ] Sheet and menu *exit* animations

If this becomes a standing need rather than a one-off, the way back is a small
`bezel-shot` crate: `open_offscreen_window` + `Window::render_to_image` +
`simulate_*` + a pixel diff, ~120 lines, and useful to any gpui app — but
macOS-only, so it can never gate CI.

## Found while verifying the above

- [ ] A `div().id(..)` wrapped around a button takes clicks over a box far
      narrower than the label it paints: the gallery's "Open sheet" trigger
      only responds near x=30 while its text runs to x=104. Hit-testing and
      layout disagree — worth chasing in gpui before more of the gallery is
      wired this way.

## Components — remaining

- [ ] **Textarea** — multi-line editing. Deserves its own pass: line wrapping,
      vertical cursor movement, its own scrolling. Do not half-ship it.
      `TextField`'s offset math (word boundaries, UTF-16 mapping, grapheme
      stepping) carries over as-is; the layout does not — `shape_line` gives
      one line, this needs `shape_text`, per-line hit testing, a goal column
      for vertical motion, and a scroll offset that follows the caret.

Need a real use case before building:

- [ ] Menubar — app chrome more than a component; comet's is app-coupled
- [ ] Date picker / calendar — large surface
- [ ] Pagination — desktop apps rarely paginate; skip until something needs it

## Next round (deferred by decision)

Data surfaces:

- [ ] Scroll area (styled scrollbar over gpui scroll handles)
- [ ] Table
- [ ] Tree view
- [ ] Virtualized list wrapper over gpui `list()`

Heavy extractions from comet (ports, not new design):

- [ ] `bezel-markdown` — streaming, block-incremental renderer
- [ ] `bezel-syntax` — tree-sitter highlighting + bounded highlight cache
- [ ] `bezel-terminal` — alacritty grid view (leave the app-coupled panel behind)

## Consumability

Worth more than more widgets, for a library other projects depend on.

- [ ] Push `crabtalk/zed`, then swap the gpui patch from `path` to
      `{ git, rev, version = "=0.2.2" }`. Until then a fresh clone needs the
      sibling `../zed` checkout — and CI is impossible.
- [ ] CI: fmt, clippy, test, build (blocked on the above)
- [ ] README example that is not the gallery
- [ ] Migrate `bezel-theme`'s `sync_ns_appearance` from `objc 0.2` to `objc2`,
      dropping our last pre-`objc2` dependency. Match gpui's version carefully:
      its tree already holds two objc2 generations.
- [ ] Remove the `.cargo/config.toml` future-incompat mute once gpui drops `cocoa`
- [ ] Real crates.io release once zed publishes a gpui matching this API
      (0.0.1 stubs currently reserve `bezel`, `bezel-theme`, `bezel-motion`, `bezel-ui`)
- [ ] Build the `bezel` facade crate when the heavy layers land and features
      start earning their keep (see `ARCHITECTURE.md`)

## Done

Layers: `bezel-theme` (tokens as a gpui `Global`, designed light+dark, oklch
math, appearance switching) · `bezel-motion` (bezier catalog, pulse clock,
hover fades, pure phase math) · `bezel-ui` · `apps/gallery`.

The gallery is the documentation: a top nav for the kind of thing
(Foundations, Components), a rail grouped by what each is *for*, one page in
the pane with its title and source path. `TABS` in `apps/gallery/src/lib.rs`
is the catalog — adding a component means one row there and one arm in
`section_body`, and two tests check every row has a page and every source path
resolves.

Foundations: colour tokens with contrast ratios · typography (families,
weights, the sizes actually in use) · layout constants drawn at size · the
bezier curves and the whole motion catalog, plotted from their own pure
functions · materials · all 58 icons.

A third tab, Patterns — composed examples rather than primitives, shadcn's
"blocks" — is the obvious next one, once there are compositions worth porting
out of comet.

Components: button ×3 · text field (IME, selection, clipboard, native
shortcuts) · select · combobox · command palette · checkbox · radio · toggle ·
badge ×2 · avatar · progress · slider · tabs · toggle group · disclosure +
collapsible header · breadcrumb · tag · status dot · empty state · tooltip ·
hover card · context menu · popover/menu/dialog/sheet mounts · resizable
split · group box + rows · separator · skeleton rows · alert strips ·
spinners · icons (58 SVG) · material glass.

Infrastructure: gpui sourced from our fork via `[patch.crates-io]` ·
`Window::paint_backdrop_blur` ported onto the fork (`e0b415b4bc`) and verified
rendering · `font-kit` feature (without it every glyph is notdef) · 81 tests.
