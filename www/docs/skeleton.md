---
title: Skeleton
description: Pulsing placeholder rows for a list that is still loading, staggered off the shared pulse clock.
---

```rust
use motion::Painter;
use ui::popover;

popover::redacted_rows("recent-sessions", &theme, 3, Painter::of(cx), cx)
```

The `Painter` leases this view onto the shared 30fps clock, so every skeleton in the window pulses in phase and the clock parks when the last one unmounts.

## Loading state

```rust
use ui::popover::{self, Loadable};

match &self.sessions {
    Loadable::Loading => popover::redacted_rows("sessions", &theme, 3, view, cx),
    Loadable::Error(message) => popover::error_row(&theme, message.clone()).into_any_element(),
    Loadable::Ready(sessions) => rows(sessions).into_any_element(),
    Loadable::Idle => gpui::Empty.into_any_element(),
}
```

## API

```rust
// ui::popover

/// `count` rows, each entering the wave after the one above it.
pub fn redacted_rows(
    id: &'static str,
    theme: &Theme,
    count: usize,
    painter: Painter,
    cx: &mut gpui::App,
) -> AnyElement;

/// A `Div`, so the retry control is a child you add.
pub fn error_row(theme: &Theme, message: impl Into<SharedString>) -> gpui::Div;

pub enum Loadable<T> { Idle, Loading, Ready(T), Error(String) }

// ...
```
