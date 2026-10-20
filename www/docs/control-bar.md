---
title: Control bar
description: The floating glass bar — a leading cluster, an optional centre and a trailing cluster, with the centre centred on the bar rather than on what is left.
prompt: Float a playback bar over the bottom of the page with shuffle on the left, play/pause in the middle and volume on the right.
---

```rust
use ui::control_bar::{self, Shape};

div().relative().size_full()
    .child(page)
    .child(
        div().absolute().bottom(px(20.0)).left_0().right_0()
            .flex().justify_center()
            .child(div().w_full().max_w(px(880.0)).child(
                control_bar::control_bar(&theme, Shape::Pill, leading, Some(centre), trailing),
            )),
    )
```

- The bar fills the width it is given; width and placement are the caller's. Give it a `max_w` to keep it floating in a wide window.
- `bar_button(icon, diameter, tint)` is the round control for it. Add id, click and one `.hover(..)` yourself; gpui panics on a second.
