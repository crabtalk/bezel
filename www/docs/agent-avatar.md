---
title: Blob avatars
description: Deterministic identity from a name — the same seed renders the same face, anywhere in the app, at any size.
---

```rust
use agent::Face;

// The face is the seed; `pose(t)` samples it at an instant. `t` is your own
// clock, and a face that never moves is `pose(0.)`. The canvas fills its
// layout bounds and centers the design space inside.
div()
    .w(px(48.))
    .h(px(48.))
    .child(agent::avatar(Face::from("Sara").pose(t)))
```

`Face::from(name)` derives the silhouette and the eyes from the name, so the same person is recognizably the same on every surface. Color is the caller's: left unset it follows `theme.accent`, and the painter takes the eye ink from whichever end of the theme reads as a hole in that body.

## Pixel

The same face on an eight-cell grid, for the sizes a spline cannot survive:

```rust
div().w(px(13.)).h(px(13.)).child(agent::mascot(&Face::from("Sara"), t))
```

`mascot` takes the `Face` rather than a `Pose` because it samples the silhouette per cell instead of tracing an outline — which is also why it cannot draw a blend of two faces, where the spline painter can. Eyes are punched through rather than painted, so a row dims whole instead of the eyes fighting the body on the way down.

Two deliberate deviations from the reference: input is trimmed and lowercased but not NFC-normalized, and the trait reader skips the `pick`/`bool` pair the ported geometry never calls. Pinned byte-for-byte against the reference's golden fixture in `crates/agent/tests/avatar.rs`, so a future drift must be a deliberate change.
