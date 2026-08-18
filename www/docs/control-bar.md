---
title: Control bar
description: The floating glass bar — a leading cluster, an optional centre and a trailing cluster, with the centre centred on the bar rather than on what is left.
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

Apple Music's transport, an agent app's composer, a floating toolbar — `Shape` is the only thing that differs between them. `Pill` is a stadium, its radius half the bar's height; `Rounded` is the rounded rectangle at `BUBBLE_RADIUS`, which is what most composers want.

Two things it exists to get right.

**The blur corners follow the border.** One radius comes out of `Shape` and feeds both the border and the backdrop blur, so there is no second number to keep in step — a mismatch frosts square corners outside a round border.

**The centre is centred on the bar, not on what the clusters leave.** The two rails are equal-flex and the centre is not, so clusters of five controls and three still keep the middle on axis. Flexing the centre between them is the classic toolbar bug: it lands wherever the wider cluster pushes it.

That second rule is why the bar takes the width it is *given* rather than hugging its controls. Equal rails need free space to be equal about, and a shrink-to-fit bar has none. So width and placement are the caller's, and a `max_w` is how a wide window gets a floating bar instead of a docked one. This bar floats over content and must never reflow it — a bar that does reflow is a dock, which is a different thing with no blur and no float.

`bar_button(icon, diameter, tint)` is the circular control inside it. The diameter is a parameter because a transport's primary action is deliberately bigger than its neighbours, and that difference is what makes the cluster readable at a glance. It builds the icon rather than taking one, because gpui reads an svg's color off that element's own style and paints nothing when it is unset — a tint set on the button would silently never reach the glyph. Add your own `.hover(..)`: gpui panics on a second hover call, and `Theme::glass_hover` is the wash to reach for.
