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

Every test in the tree is a pure function. The whole of layout, paint, hit
testing and scrolling has no automated coverage at all — the multi-line work
landed on `cargo run -p gallery` and nothing else. Multi-line wrapping, `enter`
and backspace at the growth ceiling were exercised by hand on 2026-08-17; these
were not:

- [ ] Undo coalescing end to end — the grouping rule is unit-tested, the stack
      and its bound are not
- [ ] Vertical motion holding its goal column across a short row
- [ ] Selection painted across a soft wrap (one quad per row)
- [ ] Scroll following the caret, and the wheel *not* being dragged back to it
- [ ] Horizontal scroll in a `Shape::Line` field
- [ ] IME composition on a multi-line field — `bounds_for_range` anchoring the
      candidate panel to the composing row rather than the box

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

## Found while building the textarea

- [ ] **No keyboard focus traversal anywhere.** gpui has the machinery —
      `FocusHandle::tab_index`/`tab_stop`, `Window::focus_next`/`focus_prev` —
      and bezel uses none of it: `tab_stop` defaults to false and no component
      sets either. So `tab` reaches nothing in any bezel app.

      Not a mechanical fix. Only `TextField`, `Combobox` and `CommandPalette`
      own a focus handle at all; every button, checkbox, toggle, radio, tab and
      slider is a stateless `fn(&Theme, ..) -> Div` with nowhere to put one.
      Making them focusable means either giving them state — against the grain
      of the whole widget layer — or leaving focus to the caller and accepting
      that bezel ships no keyboard story. That is a design conversation before
      it is a task.
- [ ] `Shape::Grow` shapes its text twice per frame — once in the measure
      closure to get the row count, once in `prepaint` to paint. Correct but
      wasteful; wants a cache keyed on (text, wrap width).

## Components — remaining

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
- [ ] README example that is not the gallery — the Configuration section now
      carries real snippets (`set_palette`, binding an action), but there is
      still no worked example of building an app against bezel
- [ ] Migrate `bezel-theme`'s `sync_ns_appearance` from `objc 0.2` to `objc2`,
      dropping our last pre-`objc2` dependency. Match gpui's version carefully:
      its tree already holds two objc2 generations.
- [ ] Remove the `.cargo/config.toml` future-incompat mute once gpui drops `cocoa`
- [ ] Real crates.io release once zed publishes a gpui matching this API
      (0.0.1 stubs reserve `bezel`, `bezel-theme`, `bezel-motion`, `bezel-ui`;
      the tree is on 0.0.2 and unpublished)
- [ ] Feature-gate the facade when `markdown`/`syntax`/`terminal` land — that
      is what features are for, and there is nothing to gate until then

## Done

Layers: `bezel-theme` (tokens as a gpui `Global`, designed light+dark, oklch
math, appearance switching) · `bezel-motion` (bezier catalog, pulse clock,
hover fades, pure phase math) · `bezel-ui` · `bezel` (the facade: peer
namespaces plus `bezel::gpui`, so a consumer cannot end up with two copies of
gpui) · `apps/gallery`.

The gallery is the documentation: a top nav for the kind of thing
(Foundations, Components), a rail grouped by what each is *for*, one page in
the pane with its title and source path. `TABS` in `apps/gallery/src/lib.rs`
is the catalog — adding a component means one row there and one arm in
`section_body`, and three tests check that rail keys are unique, that every
source path resolves, and that the rows with no source match the TODO pages
exactly.

The rail also lists what does *not* exist: `planned()` rows carry no source
path, render a page stating what the remaining work is, and recede in the rail.
Building one means giving it a source and dropping it from `PLANNED_BODIES` —
and the test fails until both happen, so a component cannot ship while still
documenting itself as missing.

Foundations: colour tokens with contrast ratios · typography (families,
weights, the sizes actually in use) · layout constants drawn at size · the
bezier curves and the whole motion catalog, plotted from their own pure
functions · materials · all 58 icons.

A third tab, Patterns — composed examples rather than primitives, shadcn's
"blocks" — is the obvious next one, once there are compositions worth porting
out of comet.

Components: button ×3 · text field (IME, selection, clipboard, native
shortcuts) · **textarea** · select · combobox · command palette · checkbox ·
radio · toggle ·
badge ×2 · avatar · progress · slider · tabs · toggle group · disclosure +
collapsible header · breadcrumb · tag · status dot · empty state · tooltip ·
hover card · context menu · popover/menu/dialog/sheet mounts · resizable
split · group box + rows · separator · skeleton rows · alert strips ·
spinners · icons (58 SVG) · material glass.

Textarea is one `TextField` under a `Shape`, not a second component: `Line`,
`Rows(n)`, `Grow { min, max }`. Every action already worked on the content and
a byte range and none touched layout, so only three things branch — whether
`enter` inserts, whether a pasted newline survives, and how tall the box is.
`shape_text` gives one `WrappedLine` per hard newline; selection is one quad
per visual row; vertical motion is a geometric query holding a goal column, so
soft wraps and hard newlines are the same case. `ctrl-a`/`ctrl-e`/`ctrl-k` are
*logical* lines (emacs' reading, not macOS' visual one); vertical motion is
visual. `normalize` folds CRLF and buys the invariant that a `Line` field's
content never holds a newline. One scroll point serves both axes with no test
for shape: wrapped lines cannot overflow sideways, one row cannot overflow
downwards, so the clamp decides which axis is live.

Vertical motion and `enter` bind to a second key context, `TextArea`, claimed
only by multi-line fields — the palette and combobox already bind `up`, `down`,
`ctrl-n`, `ctrl-p` and `enter` for their lists, and their query fields sit
deeper in the focus path, so binding those on every field would have silently
broken both.

Undo/redo (`cmd-z`, `cmd-shift-z`) on every field — nothing below provides it,
gpui has none and `gpui_macos` does not wire `NSUndoManager`, and no app could
build it from outside since the history and the selection to restore with it
are private to the field. Whole snapshots rather than diffs, because a field
holds a sentence and `SharedString` clones are a refcount bump. A run of typing
coalesces into one step by *adjacency* — same edit kind, landing where the last
left the caret — so there is no timing threshold to invent. Pushed from
`replace_text_in_range` and never from `replace_and_mark_text_in_range`, or
every keystroke of IME composition would be its own step. Bounded (default 10 *steps*,
not keystrokes, `with_undo_limit` per field); `set_content` clears both stacks,
being a programmatic reset rather than something the user did.

Configuration is gpui's globals, not a config object: `theme::set_palette`
registers how an app builds its palette so brand colours survive a light/dark
switch, `Theme::install_custom` installs a one-shot palette, and both move the
process-wide appearance mirror that `cx.set_global` alone leaves stale.
`input::init` is optional — the actions are public, so an app that wants its
own keymap skips it. Documented in the README.

The gallery installs a menu bar. Without one a gpui app has no key equivalents
at all: `cmd-q` did not quit and `ctrl-cmd-f` did not toggle full screen, and
neither comes for free — the standard items live in a nib, which a gpui app
does not have, so full screen is an action the app binds itself.

Infrastructure: gpui sourced from our fork via `[patch.crates-io]` ·
`Window::paint_backdrop_blur` ported onto the fork (`e0b415b4bc`) and verified
rendering · `font-kit` feature (without it every glyph is notdef) · 95 tests.
