---
title: Catalog
description: Every named motion spec — duration, delay and curve — plus the hover-fade store components blend their washes through.
---

```rust
use motion::{MotionSpec, MENU_IN};

element.with_animation("menu", MENU_IN.animation(), |el, t| el.opacity(t))
```

No component inlines a duration or a curve; it names a spec.

## Hover washes

```rust
let fade = Fade::new(Painter::of(cx), "row-3");

div()
    .on_hover(motion::hover_listener(fade.clone()))
    .bg(motion::hover_blend(&fade, theme.surface, theme.element_hover))
```

A `Fade` is which view paints the wash and which element inside it — the `Painter` is half the identity, so two views using `"row-3"` never trade fades.

## Your own repeating animation

```rust
const BREATHE: MotionSpec = MotionSpec::new(1800, motion::EASE_IN_OUT);

let phase = motion::pulse_delta(&BREATHE, Painter::of(cx), cx);
```

Not `with_animation(..).repeat()`: its request is the *window's*, at the display's rate, for as long as the element stays mounted — one spinner row measured 36% CPU at 120Hz.

## App settings

```rust
use motion::AppExt as _;

cx.set_reduced_motion(true);
cx.set_pause_when_inactive(false);
```

## API

```rust
// motion

impl MotionSpec {
    /// `const`, with public fields. gpui has no native delay, so a spec with
    /// one runs for `delay + duration` and holds `progress` at 0 until it
    /// elapses.
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self;

    pub fn animation(&self) -> Animation;

    /// Pure, which is what makes the catalog testable.
    pub fn progress(&self, raw_delta: f32) -> f32;

    // ...
}

// The wrapped entrances: fade_in, fade_quick, menu_in, dialog_in, splash_out.
// `menu_out` takes its progress from the caller, because `with_animation`'s
// clock replays from 0 on remount and a replay mid-exit is a full-opacity flash.

impl Fade {
    /// Which view paints the wash and which element inside it — the `Painter`
    /// is half the identity, so two views using `"row-3"` never trade fades.
    pub fn new(painter: Painter, key: impl Into<SharedString>) -> Self;

    // ...
}

pub fn hover_listener(fade: Fade) -> impl Fn(&bool, &mut Window, &mut App) + 'static;
pub fn hover_blend(fade: &Fade, rest: Hsla, hover: Hsla) -> Hsla;

pub fn pulse_delta(spec: &MotionSpec, painter: Painter, cx: &mut App) -> f32;

impl Painter {
    /// Claims a rate for one view. The app runs a single timer, notifies only
    /// the view that is owed a frame, and parks when the last claim lapses.
    pub fn lease(self, fps: f32, until: Duration, cx: &mut App);

    // ...
}

/// Stretches every timeline, for sampling a 200ms tween frame by frame.
pub fn set_speed(scale: f32);

pub trait AppExt {
    fn reduced_motion(&self) -> bool;

    /// Elements snap to their end state and schedule nothing; `pulse_delta`
    /// returns a static 0.
    fn set_reduced_motion(&mut self, reduced: bool);

    fn pause_when_inactive(&self) -> bool;

    /// On by default. A spinner in a backgrounded window held 30fps and 22% of
    /// a core indefinitely, against 2% with this on.
    fn set_pause_when_inactive(&mut self, pause: bool);
}
```
