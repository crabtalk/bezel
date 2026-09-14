---
title: Blob avatars
description: Deterministic identity from a name — the same seed renders the same face, anywhere in the app, at any size.
---

```rust
use agent::Face;

// The face is the seed; `pose(t)` samples it at an instant. `t` is your own
// clock, and a face that never moves is `pose(0.)`.
div()
    .w(px(48.))
    .h(px(48.))
    .child(agent::avatar(Face::from("Sara").pose(t)))
```

`Face::from(name)` derives the silhouette and the eyes from the name, so the same person is recognisably the same on every surface.

## Pixel

```rust
div().w(px(13.)).h(px(13.)).child(agent::mascot(&Face::from("Sara"), t))
```

The same face on an eight-cell grid, for the sizes a spline cannot survive.

## API

```rust
// agent

impl Face {
    /// Derives the silhouette and the eyes from the name.
    fn from(name: &str) -> Self;

    /// Samples the face at an instant; `t` is your own clock, and a face that
    /// never moves is `pose(0.)`.
    pub fn pose(&self, t: f32) -> Pose;

    // ...
}

/// The canvas fills its layout bounds and centres the design space inside.
/// Colour is the caller's: left unset it follows `theme.accent`, and the eye
/// ink comes from whichever end of the theme reads as a hole in that body.
pub fn avatar(pose: Pose) -> AnyElement;

/// Takes the `Face` rather than a `Pose`, since it samples the silhouette per
/// cell instead of tracing an outline — which is why it cannot draw a blend of
/// two faces and the spline painter can. Eyes are punched through, so a row
/// dims whole.
pub fn mascot(face: &Face, t: f32) -> AnyElement;
```

Two deliberate deviations from the reference: input is trimmed and lowercased but not NFC-normalised, and the trait reader skips the `pick`/`bool` pair the ported geometry never calls. Pinned byte for byte against the reference's golden fixture in `crates/agent/tests/avatar.rs`.
