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

Hover washes are colors computed at paint time, blended through a per-fade store:

```rust
let fade = Fade::new(cx.entity_id(), "row-3");

div()
    .on_hover(motion::hover_listener(fade.clone()))
    .bg(motion::hover_blend(&fade, theme.surface, theme.element_hover))
```

A [`Fade`] is which view paints the wash and which element inside it — the view is half the identity, so two views using `"row-3"` never trade each other's fade.

The frames come from the same clock everything else in the library repeats on. `motion::lease(view, fps, until, cx)` claims a rate for one view: the app runs a single timer, wakes only when some view is owed a frame, notifies that view alone, and parks when the last claim lapses. A hover fade leases for its own 150ms and stops; a spinner renews its claim every render and drops off the moment it unmounts.

That is the whole reason not to reach for `with_animation(…).repeat()`. Its request is the *window's*, at the display's rate, for as long as the element stays mounted — one spinner row measured 36% CPU at 120Hz, almost all of it the window rebuilding its element tree. Your own repeating animation belongs on the clock too: `MotionSpec::new` is `const` and its fields are public, so naming a spec is all it takes.

```rust
const BREATHE: MotionSpec = MotionSpec::new(1800, motion::EASE_IN_OUT);

let phase = motion::pulse_delta(&BREATHE, cx.entity_id(), cx);
```

`set_speed(10.0)` stretches every timeline in the catalog, which is how a screenshot burst samples a 200ms tween frame by frame.

The app-level settings hang off `App` itself, through `AppExt`:

```rust
use motion::AppExt as _;

cx.set_reduced_motion(true);
cx.set_pause_when_inactive(false);
```

Reduced motion is gpui's own flag: `with_animation` elements snap to their end state and schedule nothing, and `pulse_delta` returns a static 0.

`pause_when_inactive` is on by default, and stops the clock while no window is active. Nothing above stops on its own — a spinner in a backgrounded window held 30fps and 22% of a core indefinitely (2026-08, debug build), against 2% with this on. The claim is refused rather than cancelled, so nothing has to resume it: the ones already held lapse, the loop runs out of work and parks, and the refresh gpui does on activation brings the renders that claim again. Turn it off for a window that must keep moving while something else has focus — a side panel, a floating HUD.
