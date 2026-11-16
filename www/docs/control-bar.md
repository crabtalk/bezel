---
title: Control bar
description: The floating glass bar — a leading cluster, an optional centre and a trailing cluster, with the centre centred on the bar rather than on what is left.
prompt: Float a playback bar over the bottom of the page with shuffle on the left, play/pause in the middle and volume on the right.
---

```rust
use ui::{control_bar::{self, Shape}, icons::glyph};

div().relative().size_full()
    .child(page)
    .child(
        div().absolute().bottom(px(20.0)).left_0().right_0()
            .flex().justify_center()
            .child(div().w_full().max_w(px(880.0)).child(
                control_bar::control_bar("transport", Shape::Pill, move |theme, _, _| {
                    let leading = control_bar::bar_button(glyph::Shuffle, 30., theme.text_muted);
                    let trailing = control_bar::bar_button(glyph::Volume2, 30., theme.text_muted);
                    (vec![leading.into_any_element()], Some(div().child("Now playing").into_any_element()), vec![trailing.into_any_element()])
                }),
            )),
    )
```

- The bar fills the width it is given; width and placement are the caller's. Give it a `max_w` to keep it floating in a wide window.
- `bar_button(icon, diameter, tint)` is the round control for it. Add id, click and one `.hover(..)` yourself; gpui panics on a second.
