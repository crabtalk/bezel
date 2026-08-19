---
title: Blob avatars
description: Deterministic identity from a name — the same seed renders the same face, anywhere in the app, at any size.
---

```rust
use agent;

// The face is the seed. Wrap it in a sized element; the canvas fills its
// layout bounds and centers the 100×100 design space inside.
div()
    .w(px(48.))
    .h(px(48.))
    .child(agent::avatar("Sara"))
```

`agent::avatar(name)` derives everything from the name — silhouette, pose, and palette — so the same person is recognizably the same on every surface, and the face never depends on the theme. The palette is chosen for contrast against dark surfaces and enforces a 4.5:1 floor between eyes and face.

Two deliberate deviations from the reference: input is trimmed and lowercased but not NFC-normalized, and the trait reader skips the `pick`/`bool` pair the ported geometry never calls. Pinned byte-for-byte against the reference's golden fixture in `crates/agent/tests/avatar.rs`, so a future drift must be a deliberate change.
