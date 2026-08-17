---
title: Music player
description: A sidebar, a table and a floating transport — the pattern that produced the control bar by finding it missing.
---

Nothing here is library code and none of it needs to be. A player is a sidebar, a table and a floating bar; bezel already had two of the three, and composing this is what showed the third was missing. `bezel_ui::control_bar` exists because of this page.

Two pieces are worth reading before you copy them.

**The clock.** Elapsed time is a stored position plus the time since it was last set, so a playing track needs no timer and no per-frame state:

```rust
let position = if self.playing {
    self.position + self.position_at.elapsed().as_secs_f32()
} else {
    self.position
};
```

**Why the thumb does not fight the clock.** While the scrubber is held, the transport shows the *grab* rather than the clock. Otherwise a playing track drags the thumb back out from under the pointer on every frame, which reads as a seek bar that fights you.

It is an entity rather than a handful of fields on its host, and that is the rule for every pattern after it: a component demo owns a value or two and can borrow its host's, but a screen owns a screen's worth of state. Its host holds one field, and the next pattern costs one more.

The album is invented, and it has to be — quoting a real catalogue in a component gallery is a licensing question rather than documentation.

The source is at `apps/gallery/src/patterns/music.rs`.
