---
title: Thinking orbs
description: Twelve hand-tuned dotted states over a sphere of particles, four sizes, monochrome ink on a transparent canvas.
---

```rust
use agent::orbs::{Orb, OrbSize, OrbState};

let orb = cx.new(|_| Orb::new().state(OrbState::Searching).size(OrbSize::Avatar));
```

An entity rather than a function: the modes mix incommensurate frequencies, so there is no seamless wrap point and gpui's folded `with_animation` clock would jump at every loop.

## On your own clock

```rust
use agent::orbs::orb_element;

// `frame` is the caller's geometry buffer, one per orb, kept for the view's life.
orb_element(state, size, t, &frame)
```

The same paint one layer down, for a host that already ticks. The reduced-motion convention is `t = 0.6` — a representative frame, not a blank box.

## API

| | |
| --- | --- |
| `OrbState` | Twelve: `Working`, `Searching`, `Solving`, `Listening`, `Connecting`, `Weaving`, `Composing`, `Breathing`, `Shaping`, `Focusing`, `Reasoning`, `Recalling`. `label()` is the status line, `as_str()` the stable snake_case name. |
| `OrbSize` | `Inline` 20, `Avatar` 64, `Large` 96, `Hero` 128 — a larger one adds detail rather than stretching the artwork. |
| builders | `state`, `size`, `speed`, `paused`, `visible`, `target_fps`, `pause_when_inactive`, each with a `set_*` twin. Content, not style. |
| `OrbTheme` | `Auto` resolves against the installed appearance at paint time; `Dark` / `Light` pin it for a surface that is not the window's. |
| `agent::orbs::engine` | The pure-Rust engine, public for a custom paint loop. |

Motion parks rather than idles: the entity redraws at 30fps and drops its timer when paused, invisible, in an inactive window or under reduced motion. Geometry is recomputed only on a tick or a semantic change, and the whole thing paints inside one `paint_layer`, which keeps hundreds of overlapping discs off the scene's bounds tree.

Ported from gpui-thinking-orbs (MIT), itself a port of Jakub Antalik's thinking-orbs. The source is at `apps/gallery/src/patterns/orbs.rs`.
