---
title: Thinking orbs
description: Twelve hand-tuned dotted states over a sphere of particles, four sizes, monochrome ink on a transparent canvas.
---

An entity rather than a function, because the clock is unbounded: the modes mix incommensurate frequencies, so there is no seamless wrap point and gpui's folded `with_animation` clock would jump at every loop.

```rust
use agent::orbs::{Orb, OrbSize, OrbState};

let orb = cx.new(|_| Orb::new().state(OrbState::Searching).size(OrbSize::Avatar));
```

The builders configure content, not style — `state`, `size`, `speed`, `paused`, `visible`, `target_fps`, `pause_when_inactive` — each with a `set_*` twin for a host that already owns the value. Ink is `OrbTheme::Auto`, resolved against the installed appearance at paint time; `Dark` and `Light` pin it for a surface that is not the window's.

Twelve states ship, each a hand-tuned animation: `Working` (particles on tilted orbits), `Searching` (a scan meridian over a dotted globe), `Solving` (bands scrambling in quarter turns), `Listening` (a waveform through latitude rings), `Connecting` (a constellation wiring itself), `Weaving`, `Composing`, `Breathing`, `Shaping`, `Focusing`, `Reasoning` and `Recalling`. `label()` is the status line to put beside one; `as_str()` is the stable snake_case name.

Four sizes — `Inline` (20), `Avatar` (64), `Large` (96), `Hero` (128). A larger one adds detail rather than stretching the 64px artwork.

## On your own clock

```rust
use agent::orbs::orb_element;

// `frame` is the caller's geometry buffer, one per orb, kept for the view's life.
orb_element(state, size, t, &frame)
```

The same paint one layer down, for a host that already ticks. The pattern page drives all twelve off a single 40ms timer scheduled from `render`, so a grid that is scrolled away or backgrounded stops with the renders. The reduced-motion convention is `t = 0.6` — a representative frame, not a blank box.

Motion parks rather than idles. The entity redraws at 30fps, and drops its timer entirely when it is paused, `visible(false)`, in an inactive window, or under reduced motion. Geometry is recomputed only on a tick or a semantic change, so a parent re-rendering faster than the orb reuses the retained frame; the whole thing paints inside one `paint_layer`, which is what keeps hundreds of overlapping discs off the scene's bounds tree.

Ported from gpui-thinking-orbs (MIT), itself a port of Jakub Antalik's thinking-orbs. The engine is pure Rust and public as `agent::orbs::engine` for a custom paint loop.

The source is at `apps/gallery/src/patterns/orbs.rs`.
