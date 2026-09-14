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

The bar takes the width it is *given* rather than hugging its controls: equal rails need free space to be equal about. Width and placement are the caller's, and a `max_w` is how a wide window gets a floating bar instead of a docked one.

## API

```rust
// ui::control_bar

/// The two rails are equal-flex and the centre is not, so a cluster of five
/// and one of three still keep the middle on axis.
pub fn control_bar(
    theme: &Theme,
    shape: Shape,
    leading: Vec<AnyElement>,
    centre: Option<AnyElement>,
    trailing: Vec<AnyElement>,
) -> AnyElement;

pub enum Shape {
    /// A stadium, radius half the bar's height.
    Pill,
    /// `Theme::bubble_radius()` — what most composers want.
    Rounded,
}

/// The circular control inside it. Builds the icon rather than taking one,
/// since gpui reads an svg's colour off that element's own style.
pub fn bar_button(icon: impl Into<Icon>, diameter: f32, tint: gpui::Hsla) -> gpui::Div;

// ...
```

One radius comes out of `Shape` and feeds both the border and the backdrop blur, so there is no second number to keep in step. Add your own `.hover(..)` — gpui panics on a second hover call, and `Theme::element_hover` is the wash.
