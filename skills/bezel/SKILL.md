---
name: bezel
description: Build native GUI apps in Rust with bezel, the gpui component library (bezel, bezel-gpui, bezel-ui, bezel-theme, bezel-motion, bezel-icons, bezel-markdown, bezel-editor, bezel-terminal). Use when adding or styling UI in a gpui app, choosing a bezel component, theming with Brand/Tint, or when a bezel/gpui build fails to link or paints no text. Triggers on `bezel`, `gpui`, `Theme::of(cx)`, `theme.button`, `ui::widgets`, `bezel.gallery`.
---

# bezel

A gpui component library, SwiftUI-lean. Style flows through the environment —
components read `Theme::of(cx)` at paint time. There are no color, font or size
parameters, and that is the thing to internalise before writing a call.

## Fetch the page before writing the call

Every doc page is served as markdown. Signatures here are wrong to guess.

| want | fetch |
| --- | --- |
| the index of every page | `https://bezel.gallery/llms.txt` |
| one component | `https://bezel.gallery/docs/<slug>.md` |
| all of it, one file (~88KB) | `https://bezel.gallery/llms-full.txt` |

Slugs by group — fetch the one you need:

- **Style** theme, color, typography, layout, material
- **Motion** motion-curves, motion-catalog
- **Assets** icons
- **Input** buttons, text-field, textarea, select, combobox, checkbox-radio, toggle, toggle-group, slider, date-picker
- **Menus** menu, context-menu, palette, menubar
- **Presentation** dialog, sheet, tooltip, hover-card
- **Layout** group-box, tabs, nav-row, collapsible, split, titlebar, control-bar, floating
- **Data** scroll-area, follow, table, tree, virtual-list
- **Content** badge, tag, avatar, breadcrumb, pagination, empty-state, skeleton
- **Status** progress, status-dot, alerts, step-row, loaders, stats
- **Patterns** agent-activity, agent-tools, agent-composer, agent-transcript, agent-diff, agent-terminal, agent-orbs, agent-avatar, document, editor, syntax

With the sources checked out, `crates/ui/src/` is the truth and
`cargo run -p gallery` shows every component live.

## The dependency line

```toml
[dependencies]
bezel = "0.1"
gpui = { package = "bezel-gpui", version = "0.3" }
gpui_platform = { package = "bezel-gpui-platform", version = "0.3", features = ["font-kit"] }
```

Three, not one: `actions!` expands to literal `gpui::` paths, and the facade
re-exports gpui but not the platform backend. The `package` rename is what keeps
those paths resolving. `gpui`, `gpui_platform` and `gpui_web` must be the **same**
version. Never write a plain `gpui = "0.2"` beside bezel — see Traps.

`markdown`, `syntax`, `blocks` and `terminal` are peer crates an app names
itself (`bezel-markdown`, …); the facade deliberately does not carry them.

## Bootstrap

Four calls no snippet can skip, in this order:

```rust
gpui_platform::application().run(|cx: &mut App| {
    ui::register_fonts(cx).ok();                  // bundled Geist into the text system
    theme::appearance::init(AppearanceMode::System, cx);  // Theme::of(cx) panics without it
    focus::init(cx);                              // tab order, enter/space activation
    cx.set_menus(vec![Menu::new("app").items([MenuItem::action("Quit", Quit)])]);
    cx.open_window(opts, |window, cx| {
        theme::appearance::observe_window(window, cx).detach();  // repaint on light/dark flip
        cx.new(Root::new)
    }).unwrap();
    cx.activate(true);
});
```

`apps/hello` in the repo is this and nothing else — the smallest correct consumer.

## Write it the way the library is written

1. **No style parameters.** `theme.button("Save", ButtonStyle::Prominent, None)`
   takes no color and no size. Override by chaining gpui modifiers on what comes
   back: `theme.group_box().rounded(px(4.0))`. If you find yourself wanting a
   color argument, you want a chain modifier instead.
2. **Stateless paint is a catalog trait on `Theme`.** Import the group, reach the
   method: `use ui::widgets::{ButtonStyle, Buttons};` → `theme.button(…)`. The
   groups are `Buttons`, `Controls`, `Icons`, `Scaffolding`, `Layout`, `Content`
   and `Status`. Stateful things are entities you hold (`TextField`, `Table`,
   `Orb`, `Palette`).
3. **Interaction is yours.** A widget returns a `gpui::Div`; you attach `.id()`
   and `.on_click()`. bezel never owns your handlers.
4. **Write no numbers.** `ui::stack::row()` already carries the system gap.
   `.gap(px(12.0))` is a deviation, and should read as one. Sizes come from
   `TextStyle` and `ControlSize`, not from `px`.
5. **Motion is named.** Take a spec from the `motion` catalog; no inline
   durations or cubic-beziers.
6. **Theme by `Brand`, not by token.** One hue for greys, one for accent, one
   radius, set before `appearance::init`. Lightness is not a knob — that is what
   keeps a rebranded palette at the contrast ratios it was verified at.

## Traps

- **Two gpuis.** Declaring your own `gpui` dependency alongside bezel can put a
  second copy in the graph: two type universes, whose best failure is a trait
  bound error and whose worst is a window that paints shapes but no text. Go
  through the `package = "bezel-gpui"` rename above, or `bezel::gpui`.
- **`Theme::of(cx)` before `appearance::init`** — the global is not there yet.
- **wasm.** `std::time::Instant` panics on `wasm32-unknown-unknown`; use
  `web-time`. Reach for `bezel-gpui-web` instead of `gpui_platform`, and turn
  bezel's `platform` feature off (it is off by default).
- **No fonts registered** silently falls back to system faces; on Linux, missing
  the `geist-weights` faces paints every weight at 400.
- **No menu, no `cmd-q`.** A gpui app gets no menu bar for free.
- **Icon color.** gpui reads an SVG's color off the element's own style and
  paints *nothing* when unset — set it on the glyph, not on the parent.
  `theme.icon(glyph::Search)` carries a tone and a ladder step already;
  `icons::icon(..)` is the bare builder and needs both.
- **Fade keys must be unique.** Two elements sharing one trade hover animations.

## When bezel is the thing that is wrong

Component missing a case, a doc page whose snippet does not compile, a panic
inside `ui`/`theme` — that is a bug in the library, not something to paper over
in app code. Reduce it to the smallest gpui app that shows it, then open an issue
at https://github.com/crabtalk/bezel/issues with the repro, the bezel and
bezel-gpui versions, the platform, and what you expected. Do not work around it
silently: the workaround is the bug report nobody filed.
