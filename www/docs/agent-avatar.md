---
title: Blob avatars
description: Deterministic identity from a name — the same seed renders the same face, anywhere in the app, at any size.
prompt: Give each agent in the sidebar a generated blob avatar from its name, 48px in the chat header and a tiny pixel version in the list rows.
---

```rust
use agent::Face;

let face = Face::from("Sara");
div().w(px(48.)).h(px(48.)).child(agent::avatar(face.pose(t)))   // smooth, any size
div().w(px(13.)).h(px(13.)).child(agent::mascot(&face, t))       // 8×8 cells, for tiny sizes
```

- Both elements fill their parent; size the parent.
- `t` is the caller's clock in seconds. `pose(0.)` is a face that never moves.
- Names are trimmed, lowercased and whitespace-collapsed before seeding; they are not Unicode-normalised.
- Colour is `theme.accent` unless `Face::color` sets it. `Pose::lerp` blends two faces; `mascot` cannot draw a blend.
