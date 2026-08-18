---
title: Catalog
description: Every named motion spec — duration, delay and curve — plus the hover-fade store components blend their washes through.
---

Law 3: no component inlines a duration or a curve, it names a spec. A `MotionSpec` is a duration, an optional delay and a curve, and it hands gpui a ready animation:

```rust
use motion::{MotionSpec, MENU_IN};

element.with_animation("menu", MENU_IN.animation(), |el, t| el.opacity(t))
```

gpui `Animation` has no native delay, so a spec with one folds it into the timeline: the animation runs for `delay + duration` and `progress` holds 0 until the delay has elapsed. `progress(raw_delta)` is pure, which is what makes the catalog testable.

The common entrances are already wrapped, so a caller names the element rather than the tween — `fade_in`, `fade_quick`, `menu_in`, `dialog_in`, `splash_out`, and `menu_out`, which takes its progress from the caller because `with_animation`'s clock replays from 0 on remount and a replay mid-exit is a full-opacity flash.

Hover washes are not animation elements. They are colors computed at paint time, blended through a per-key store:

```rust
div()
    .on_hover(motion::hover_listener("row-3"))
    .bg(motion::hover_blend("row-3", theme.surface, theme.element_hover))
```

That means the app's root render has to pump the frames:

```rust
if motion::hover_fades_active() {
    window.request_animation_frame();
}
```

Not optional. The listener dirties the window once as the pointer crosses; every frame after that has to be asked for, and an app that skips this paints the blend's first frame and holds it until something unrelated repaints. It is also the tick that evicts fades whose elements are gone, so skipping it leaks an entry per hovered element.

The repeating loaders share one 30fps clock rather than running as repeating animations. A `with_animation` loop requests a redraw every display frame for as long as it is mounted — one spinner row measured 36% CPU at 120Hz — while `pulse_delta` leases the calling view onto a shared epoch, keeps every instance phase-locked, and parks the clock entirely when the last spinner unmounts.

`set_speed(10.0)` stretches every timeline in the catalog, which is how a screenshot burst samples a 200ms tween frame by frame. Reduced motion is gpui's own flag: `with_animation` elements snap to their end state and schedule nothing, and `pulse_delta` returns a static 0.
