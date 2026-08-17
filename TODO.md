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

Focus traversal landed the same way and is in the same state — it compiles, it
is unrun:

- [ ] `tab` reaching checkbox, radio, toggle, segment, slider and tab, in the
      order they are painted
- [ ] `space`/`enter` on a focused control doing exactly what its click does
- [ ] `←`/`→` moving the slider, and the drag it now takes
- [ ] The ring appearing on all six. It lands on a border each control paints
      transparent, so a control that loses that border loses its ring silently —
      the failure looks like nothing at all

The date picker is in the same state, and its *arithmetic* is not — that part is
eleven pure tests deep. What no test can reach is the card:

- [ ] The grid painting where the maths says it does — the month opening on the
      right weekday, and today, the selection and the cursor each reading as
      themselves
- [ ] Walking off the end of a month carrying the grid with it
- [ ] `pageup`/`pagedown`, `escape`, and `enter` opening a closed picker

The menubar's whole point is a pointer behaviour, so it is the least verified
thing here:

- [ ] **Hover-switch** — one menu down, the pointer crossing a sibling title
      opening that one; and hovering with *nothing* down opening nothing
- [ ] Clicking a second title switching rather than being swallowed by the
      dismissal that same press causes — `note_trigger_press_matching`'s reason
      to exist, never once exercised
- [ ] `left`/`right` crossing menus with the keyboard, and a disabled row being
      neither clickable nor landable

The scrollbar's geometry is tested in both directions; everything you do to it
with a pointer is not:

- [ ] Dragging the thumb — including that it keeps the grab point rather than
      snapping its middle to the pointer, which is the whole reason
      `ScrollbarState` exists
- [ ] The wheel still reaching the content *through* the bar (both hitboxes sit
      under the pointer, so it should — the bar deliberately does not occlude)
- [ ] Dragging one bar while three are mounted moving only that one
- [ ] The bar appearing on the frame after a page's content first overflows,
      and vanishing entirely when it fits

The table's reducer is tested and its cell-count guard has teeth; what nothing
checks is that the columns actually land where the model says:

- [ ] A heading click sorting, and the arrow landing on the clicked column
- [ ] The header holding still while the body scrolls under it
- [ ] Header and body cells lining up — the drift the shared column slice is
      supposed to make impossible, never once looked at

The tree's reducers are six tests deep and its paint is not:

- [ ] Clicking a folder opening it, and a file reading as chosen
- [ ] The arrows walking, opening and collapsing — `tab` reaching the tree at
      all, since it is one stop and the rows are not
- [ ] Indent guides landing under their parent's chevron rather than beside it,
      which is the one number (`INDENT` against the chevron slot) that nothing
      checks

The virtualized list is the best-measured page here — 9 rows built of 10,000,
read off a temporary probe rather than assumed — and still unseen:

- [ ] Scrolling it, and the built count staying flat while it moves (the count
      is on the page, so this one checks itself)
- [ ] The thumb tracking a 10,000-row document, which is the same bar over a
      handle it has never been pointed at before

The paginator's window is six tests deep; the row it draws is not:

- [ ] Clicking a page, and the prev/next steps going inert at the ends rather
      than vanishing
- [ ] The row holding its width as you walk to either end — the point of the
      sliding window, and the one thing a unit test can only assert about slots
      rather than pixels

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

- [x] **No keyboard focus traversal anywhere.** Answered by `crates/ui/src/focus.rs`
      — see Done.
- [ ] `Shape::Grow` shapes its text twice per frame — once in the measure
      closure to get the row count, once in `prepaint` to paint. Correct but
      wasteful; wants a cache keyed on (text, wrap width).

## Components — remaining

Nothing. Every row in the gallery's catalog has a source, and `PLANNED_BODIES`
is empty — the first time that has been true. `planned()`/`todo()` are kept
(unused, and marked as such) because the convention they encode is what the next
unbuilt component gets declared with, not because anything needs them today.

## Next round (deferred by decision)

(Data surfaces are done: scroll area, table, tree view, virtualized list.)

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
shortcuts) · **textarea** · select · combobox · command palette · date picker ·
menubar · checkbox · radio · toggle ·
badge ×2 · avatar · progress · slider · tabs · toggle group · disclosure +
collapsible header · breadcrumb · tag · status dot · empty state · tooltip ·
hover card · context menu · popover/menu/dialog/sheet mounts · resizable
split · scroll area · table · tree view · virtualized list · pagination · group box + rows · separator · skeleton rows · alert strips ·
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

Pagination (`pagination.rs`) — built against this file's own advice, which said
to skip it, so the reason is on the record. That advice stands for the case it
was written about: a long list is answered by the scroll area and the virtualized
list, and paginating one on a desktop is worse than scrolling it. What earns the
component is the *other* case — data that arrives in pages and cannot be held
whole, where the page number is the query rather than a scrolling affordance.
The gallery page says which case it is for, so the rail does not read as an
endorsement of the first.

The module is one function and some paint. `window` turns `(current, total)`
into the row, and the two rules that make it more than an iterator are the ones
under test: a gap hiding exactly *one* page shows the page instead (an ellipsis
standing for a single number takes the same room and says less), and the window
slides at the ends rather than shrinking, so walking to the last page does not
narrow the control under the pointer — the same refusal to reflow as the ring
slot and the calendar's six rows. Pages are 1-based, against the rest of the
crate, because a page number is a label rather than an index.

It also emptied `PLANNED_BODIES`, which turned `planned()` and `todo()` into
dead code — kept, marked, and documented rather than deleted: the convention is
what the next unbuilt component will be declared with, and the guard test now
asserts both sides are empty rather than nothing at all.

Virtualized list (`list.rs`) — and the entry it replaces was wrong about which
primitive. gpui has two virtualizers and only one can carry a scrollbar.
`uniform_list`'s handle wraps a real `ScrollHandle` (`base_handle`, public, and
registered as the list's tracked handle — gpui's own `is_scrolled_to_end` reads
pixel geometry off it), so `scroll::scrollbar` reports on a virtualized list
with no second implementation behind a trait. `list()`, the variable-height one,
speaks `ListOffset { item_ix, offset_in_item }` — logical, no maximum offset, no
viewport — and a proportional thumb needs a total height that a variable-height
list cannot know without measuring every row, which is the work virtualization
exists to skip.

So the module is thin on purpose, and earns its keep on the two things a caller
would otherwise have to know: the bridge to that inner handle, and applying the
row height to every row, since `uniform_list` sizes them all from the *first*
one it renders and silently overlaps the rest otherwise.

A third thing came out of building it, and only because the gallery counts the
rows it actually builds. The first run built **one** row of ten thousand: the
list had no height of its own, so it collapsed, measured item zero and stopped —
an empty box with no error and nothing to grep for. `virtual_list` now fills its
parent by default (a virtualized list is bounded by definition; a caller wanting
otherwise sets a size after, and the later call wins). With that, the same probe
reads 9 of 10,000 for a 220px frame at 26px a row, which is the arithmetic.

Tree view (`tree.rs`) — the design is one observation: **a depth-annotated flat
list is a complete navigation model.** bezel cannot walk an app's tree (it has
no idea what a node is, and a trait to find out would be a data model this
library does not want), so the app flattens the open parts into visible rows —
which it must do to render them anyway. Everything then falls out of that list
with no parent pointers and no walk: up and down are neighbouring indices, a
first child is the next row, and a parent is the nearest row above of smaller
depth. `parent_of` is the only piece with any subtlety, because the row directly
above is usually a sibling or the last leaf of a sibling's whole subtree.

`step` reports an intent — `To`/`Expand`/`Collapse` — rather than mutating,
since only the app can open a node; it owns the set, and a file tree's open
folders often outlive the window. Keys follow `focus.rs`: bezel names the four
chords everyone agrees on and publishes the actions, the app handles them. That
also drew the line between this and the menubar, which *can* handle its own keys
because it owns everything they touch.

Neither end wraps, deliberately. A menu wraps because it is a ring of choices; a
tree is a document, and arriving back at the top because you pressed down once
too often loses your place. Indent guides are per-row segments — one bordered
div per ancestor level — so the line runs continuously down the page without any
element spanning rows or knowing its neighbours, and leaves keep an empty
chevron slot so their labels line up with their siblings'.

One test caught a wrong assumption in its own test rather than in the code:
`left` on an open root branch collapses it, and only a row with nothing left to
close goes looking for a parent.

Table (`table.rs`) — and the first question it has to answer is when *not* to
use it. A list of records reads better as `group_box` + `card_row` + `row_title`
+ `meta_line`, which this library already had; a table earns its place only when
rows are tuples and reading *down* a column is the point.

Which names its own failure: a header and a body that size their own cells drift
apart the moment either changes, and nothing catches it because both halves look
right alone. So columns are declared once and shared, and `row` zips its cells
onto them rather than letting a caller size a cell where it writes it. A row
short of cells is a caller bug, not a shape to render — the `debug_assert` has a
`#[should_panic]` test behind it, because a guard nothing exercises is
decoration.

Sorting is a reducer and nothing else: `next_sort` says what a click on a
heading meant, the app sorts its own rows, and the table paints the arrow. A new
column starts ascending rather than inheriting the last one's direction — carry
it over and clicking a fresh heading can sort it descending, which reads as the
click having been ignored. `Align` has no `Center`, deliberately. Column resize
is out until something needs it (`widgets::axis_fraction` is the path, plus
per-column state), and virtualization stays its own entry.

The gallery page is where the last two commits meet: a sticky header outside a
scrolling body, with the body's own scrollbar — the table staying out of the
scrolling business is what lets that compose at all.

Scroll area (`scroll.rs`) — the bar only, not a wrapper: the caller keeps its
own `overflow_y_scroll` container, because something that swallowed the content
would have to re-implement layout for it. The geometry is zed's `thumb_ranges`
transcribed (fifteen lines of its 1722; the rest is a settings system, three
reveal policies and four handle types). Both of gpui's conventions here invert
easily — `max_offset` is the overflow rather than the content, and `offset` is
negative going down — so the round trip between the thumb and the offset that
drew it is a test, in both directions and at both ends.

It carries its own gesture, which is the part worth keeping: `ScrollHandle` and
the `Rc<Cell>` holding the grab both mutate through `&self`, so the bar wires
plain closures over two clones and calls `window.refresh()` — no `cx.listener`,
no view state beyond two fields, one line at the call site instead of the
fifteen `split_handle` asks for. The grab is what stops the thumb jumping its
middle to the pointer on every press. The drag payload carries the bar's id
because `on_drag_move` filters by type alone and an app has several bars at
once.

A render pass can only see the handle as the last frame left it, so a bar would
otherwise appear one frame late — or never, if nothing else repaints, which is
how the hover fades failed. A canvas compares its laid-out height against the
geometry the render used and asks for one more frame when they disagree; idle
CPU measured 0.0%, so it converges rather than spinning.

Overlay, never a gutter: a bar arriving or leaving must not reflow what it
reports on. Track clicks do nothing yet — the wheel and the thumb are the two
real gestures, and click-to-page is a platform preference before it is a
feature.

Menubar (`menubar.rs`) — the *in-window* bar. The native one is `cx.set_menus`
and four lines in an app's `main`, which is where it stays; this is the bar an
app with a custom titlebar draws itself. The page's old note ("app chrome more
than a component; comet's is app-coupled") held right up to the design: nothing
was ported, and what makes it a component rather than a row of dropdowns is that
one menu being open changes what the others do — the pointer crossing a sibling
title switches to it with no click, and `left`/`right` cross menus without
leaving the keyboard.

One `Popup<usize>` for the whole bar rather than one per title, because exactly
one menu can be down and saying so in the type makes switching a single
assignment. `note_trigger_press_matching` already existed for it, having been
written for the shared-popup case: the press that dismisses menu A is the same
press that opens menu B, and only a press on the *owning* title counts as a
dismissal. Menus are data the app hands over, shaped like gpui's `Menu`/
`MenuItem` so an app drawing both bars writes them alike — but not those types,
which carry a boxed action; bezel reports an index and leaves dispatch alone.
Accelerators are printed, never bound: a menu showing a keystroke it did not own
would be documenting a lie.

The keyboard is why disabled rows and separators are in scope at all —
`next_selectable` steps over both, wraps, and answers `None` for a menu with
nothing to land on, which is the one shape that would otherwise walk the ring
forever. Six pure tests, that one included. No submenus, no checkmarks, no `alt`
to focus the bar: each is where a menubar turns into a menu *system*, and
nothing needs one.

Date picker (`date.rs`) — an entity, not a function, and the line between the
two is now clear enough to state: a select owns nothing the caller does not
already own, so it stays a trigger plus a mount; a calendar owns a month on
screen and a keyboard cursor, which the app has no opinion about. It reports the
one thing the app does care about and nothing else.

The cursor *is* the view month — one field, not two, so walking off the end of a
month and paging to the next are the same operation and cannot disagree about
where you are. The grid is always six rows of real dates, including the
neighbouring months' edges: a card that resized as you paged would move the day
you were about to click, and blanks would need a second test that real dates do
not (`cell.month() != view.month()`).

`Date` is bezel's own. chrono is already under gpui, so taking it would compile
for free — and would make it *public* API, so a consumer with its own chrono
would have two, which is the split-graph failure `bezel::gpui` exists to
prevent. A picker needs no timezones, no parsing and no formatting: it needs the
civil calendar, which is Hinnant's two conversions transcribed and everything
else as a round trip through them. Eleven pure tests, checked against the system
calendar rather than against themselves. bezel carries no clock either — `today`
is a constructor argument, since the app is the only thing that knows its
timezone; the gallery does that conversion in three lines, which is the whole
integration story for an app that already speaks chrono.

Keyboard focus traversal (`focus.rs`), and every control wired to it. gpui had
the whole machinery and none of it on: `tab_stop` starts false and no keys are
bound. `focus::init` binds `tab`/`shift-tab` window-wide and `enter`/`space` in
a `Control` key context that only a focused control claims, so it wins `enter`
over the palette's list and a field's newline and gives it straight back.
Traversal order is paint order — every `tab_index` stays 0, so inserting a
control into the middle of a form renumbers nothing, which is the failure that
makes HTML `tabindex` a liability.

The handle stays with the *caller*. Giving a checkbox one of its own would mean
giving it an identity and a lifetime — the entity machinery `TextField` needs
and a checkbox does not — so `focus::focusable` takes the app's handle, and the
gallery holds them beside the state each control already paints from. `Activate`
is dispatched rather than folded into `on_click` for the same reason: only the
caller knows what a press means. That leaves two call sites per control and two
call sites are where a keyboard route quietly diverges from the mouse one, so
the gallery takes the behaviour once (`pressable`) and wires both from it. A
slider holds a value rather than a press, so it also answers
`Decrement`/`Increment` on `←`/`→`; the step is the caller's, since a library
picking one would pick it for a percentage and a font size alike.

The ring lands on the control's own border, which is why every widget now keeps
one even where it paints nothing (`RING_SLOT`): gpui sizes border-box, so a
border that appeared only on focus would shift the content under it by a pixel.
The toggle's knob and the tab's underline each pay a pixel back, absolute insets
being resolved against the padding box. A ring wrapped *around* each control
instead would have cost every one of them a radius parameter and prised a
focused tab off the hairline its underline has to overlap.

Configuration is gpui's globals, not a config object: `theme::set_palette`
registers how an app builds its palette so brand colours survive a light/dark
switch, `Theme::install_custom` installs a one-shot palette, and both move the
process-wide appearance mirror that `cx.set_global` alone leaves stale.
`input::init` is optional — the actions are public, so an app that wants its
own keymap skips it. Documented in the README.

No environment variables anywhere, as of the `motion::set_speed` change: the
speed knob used to read `BEZEL_MOTION_SCALE` through a `OnceLock`, which made it
the one piece of configuration in the library that no app could reach and no
app's own settings could carry. It is now an atomic mirror behind
`set_speed`/`speed_scale`, sited for the same reason as the theme's appearance
mirror — the timelines are read from free functions with no `cx` in scope, so a
gpui global cannot serve them. An app wanting it on a theme or a setting wires
that from the layer that owns both; motion sits below theme and stays there.
Every test that measures a timeline now holds `lock_speed()`, the same
arrangement `theme::lock_appearance` uses, or moving the knob in one test would
have quietly changed the arithmetic under another.

The gallery installs a menu bar. Without one a gpui app has no key equivalents
at all: `cmd-q` did not quit and `ctrl-cmd-f` did not toggle full screen, and
neither comes for free — the standard items live in a nib, which a gpui app
does not have, so full screen is an action the app binds itself.

The hover fades were never actually running. `motion::hover_listener` hands off
to a once-per-frame tail — `hover_fades_active()` + `request_animation_frame` in
the app's root render — and nothing in the tree had ever called it, so every
wash painted its first frame at rest and then held until an unrelated repaint
came along. It read as a hover that lagged and then jumped. The fade system was
right; the app was missing the two lines it is written against, which is now the
one required call in the README rather than a contract a reader had to infer.

Infrastructure: gpui sourced from our fork via `[patch.crates-io]` ·
`Window::paint_backdrop_blur` ported onto the fork (`e0b415b4bc`) and verified
rendering · `font-kit` feature (without it every glyph is notdef) · 138 tests.
