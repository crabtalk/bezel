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

| | |
| --- | --- |
| `MotionSpec::new(ms, curve)` | `const`, with public fields. gpui has no native delay, so a spec with one runs for `delay + duration` and holds `progress` at 0 until it elapses. |
| `fade_in` / `fade_quick` / `menu_in` / `dialog_in` / `splash_out` | The wrapped entrances. |
| `menu_out` | Takes its progress from the caller: `with_animation`'s clock replays from 0 on remount, and a replay mid-exit is a full-opacity flash. |
| `painter.lease(fps, until, cx)` | Claims a rate for one view. The app runs a single timer, notifies only the view that is owed a frame, and parks when the last claim lapses. |
| `set_speed(10.0)` | Stretches every timeline, for sampling a 200ms tween frame by frame. |
| `set_reduced_motion(true)` | Elements snap to their end state and schedule nothing; `pulse_delta` returns a static 0. |
| `set_pause_when_inactive(..)` | On by default. A spinner in a backgrounded window held 30fps and 22% of a core indefinitely against 2% with this on. |
