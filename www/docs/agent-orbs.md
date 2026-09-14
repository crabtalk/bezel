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

```rust
impl Orb {
    pub fn new() -> Self;

    // Content, not style — each with a `set_*` twin for a host that already
    // owns the value.
    pub fn state(self, state: OrbState) -> Self;
    pub fn size(self, size: OrbSize) -> Self;
    pub fn speed(self, speed: f32) -> Self;
    pub fn paused(self, paused: bool) -> Self;
    pub fn visible(self, visible: bool) -> Self;
    pub fn target_fps(self, fps: f32) -> Self;
    pub fn pause_when_inactive(self, pause: bool) -> Self;

    // ...
}

pub enum OrbState {
    Working, Searching, Solving, Listening, Connecting, Weaving,
    Composing, Breathing, Shaping, Focusing, Reasoning, Recalling,
}

impl OrbState {
    /// The status line to put beside one.
    pub fn label(self) -> &'static str;
    /// The stable snake_case name.
    pub fn as_str(self) -> &'static str;

    // ...
}

/// A larger one adds detail rather than stretching the artwork.
pub enum OrbSize { Avatar, Inline, Large, Hero }

/// `Auto` resolves against the installed appearance at paint time; `Dark` and
/// `Light` pin it for a surface that is not the window's.
pub enum OrbTheme { Auto, Dark, Light }

/// The same paint one layer down, for a host that already ticks.
pub fn orb_element(
    state: OrbState,
    size: OrbSize,
    t: f32,
    frame: &Rc<RefCell<Frame>>,
) -> impl IntoElement;
```

Motion parks rather than idles: the entity redraws at 30fps and drops its timer when paused, invisible, in an inactive window or under reduced motion. Geometry is recomputed only on a tick or a semantic change, and the whole thing paints inside one `paint_layer`, which keeps hundreds of overlapping discs off the scene's bounds tree. The pure engine is public as `agent::orbs::engine`.

Motion parks rather than idles: the entity redraws at 30fps and drops its timer when paused, invisible, in an inactive window or under reduced motion. Geometry is recomputed only on a tick or a semantic change, and the whole thing paints inside one `paint_layer`, which keeps hundreds of overlapping discs off the scene's bounds tree.

Ported from gpui-thinking-orbs (MIT), itself a port of Jakub Antalik's thinking-orbs. The source is at `apps/gallery/src/patterns/orbs.rs`.
