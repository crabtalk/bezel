# TODO

Status snapshot and checklist. Policy lives in `ARCHITECTURE.md`; this is what
is actually left to do.

## Needs a pointer to verify

Wired but never seen on screen — I can't inject pointer events, so these are
verified by construction only.

- [ ] Tooltip appears on hover (`bezel_ui::tooltip::Tooltip`)
- [ ] Context menu opens on right-click (uses `popover::menu_at`)
- [ ] Text field typing, word motion (`opt-←/→`), line kill (`cmd-backspace`)
- [ ] Select and palette respond to click / `⌘K` (mounted state verified, click path not)

## Components — remaining

- [ ] **Combobox** — searchable select; `select_trigger` + the palette's filtering
- [ ] **Sheet / drawer** — side panel, slides in
- [ ] **Hover card** — popover on hover instead of click
- [ ] **Resizable split** — draggable pane divider
- [ ] **Textarea** — multi-line editing. Deserves its own pass: line wrapping,
      vertical cursor movement, its own scrolling. Do not half-ship it.

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
hover fades, pure phase math) · `bezel-ui` · `apps/gallery` (two-column, every
component on screen).

Components: button ×3 · text field (IME, selection, clipboard, native
shortcuts) · select · command palette · checkbox · radio · toggle · badge ×2 ·
avatar · progress · slider · tabs · toggle group · disclosure + collapsible
header · breadcrumb · tag · status dot · empty state · tooltip · context menu ·
popover/menu/dialog mounts · group box + rows · separator · skeleton rows ·
alert strips · spinners · icons (58 SVG) · material glass.

Infrastructure: gpui sourced from our fork via `[patch.crates-io]` ·
`Window::paint_backdrop_blur` ported onto the fork (`e0b415b4bc`) and verified
rendering · `font-kit` feature (without it every glyph is notdef) · 76 tests.
