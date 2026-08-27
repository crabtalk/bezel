---
title: Pixel mascots
description: The same seed the blob avatar draws, sampled onto eight cells — one identity at the size a list row can afford.
---

```rust
use agent::Face;

// Size the box; the sprite centres inside it on whole device pixels.
div()
    .w(px(13.))
    .h(px(13.))
    .child(agent::mascot(&Face::from("bezel"), t))
```

`mascot` is not a second generator. It reads the same `Shape` and `Eyes` a name already produces and asks each cell whether the body covers it, so a rail and a chat header showing one name show one being. Nothing is picked from a roster: a name nobody has typed yet has a mascot already, for the reason the [blob avatar](/docs/agent-avatar) has a face already.

The eyes are holes, not ink. The body takes one colour and the surface shows through where the eyes are, which is what lets a whole row dim at once — paint the eyes in a contrasting colour instead and they fight the fade rather than joining it. It also means the mascot never consults the theme for a second colour, where `avatar` has to.

A blink is the only motion that survives the trip. Breath moves the outline by about a twentieth of a cell and gaze drift by about an eighth, so at this size both are nothing; the wobble moves it far enough to flip cells on the edge at random, which reads as noise rather than life. The silhouette is therefore the resting one and `lid` does all the work — which is why a mascot is still, then blinks, rather than breathing the way its larger self does.
