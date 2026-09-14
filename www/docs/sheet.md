---
title: Sheet
description: The dialog card pinned to a window edge — down the side or up from the bottom, same scrim, sliding in and back out on the popup's exit phase.
---

```rust
use ui::popover::{self, Side};

popover::sheet(
    "gallery-sheet",
    window.viewport_size(),
    Side::Right,
    px(320.0),   // a width here; a height on Side::Bottom
    popover::sheet_panel(&theme, Side::Right)
        .p(px(20.0))
        .child(popover::dialog_title(&theme, "Details"))
        .child(popover::dialog_body(&theme, "…"))
        .into_any_element(),
    self.sheet.closing_since(),
    cx.listener(|view, _, _, cx| view.close_sheet(cx)),
)
```

The exit clock is not optional: `Popup::finish_close` reaps on `MENU_OUT`'s span, so a sheet ignoring `closing_since` is unmounted mid-slide.

## API

```rust
// ui::popover

/// Slides in over `DIALOG_IN`, out over `MENU_OUT`. `extent` is measured
/// across the edge it is pinned to — a width on `Left` and `Right`, a height
/// on `Bottom` — and the other axis spans the viewport.
pub fn sheet(
    id: impl Into<SharedString>,
    viewport: gpui::Size<Pixels>,
    side: Side,
    extent: Pixels,
    content: AnyElement,
    closing: Option<web_time::Instant>,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement;

/// Rounds and hairlines its *inner* edge only — the corners on the window edge
/// are off screen.
pub fn sheet_panel(theme: &Theme, side: Side) -> gpui::Div;

pub enum Side {
    Left,
    Right,
    /// Up from the bottom edge, full width — what a narrow window wants
    /// instead of a side panel that leaves no room for the page behind it.
    Bottom,
}

// ...
```

As with `modal`, the scrim press is a parameter: the scrim is inside the deferred layer.
