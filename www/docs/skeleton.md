---
title: Skeleton
description: Pulsing placeholder rows for a list that is still loading, staggered off the shared pulse clock.
prompt: Show three pulsing placeholder rows while my session list loads, and the error with a Retry button if it fails.
---

```rust
use motion::Painter;
use ui::popover::{self, Loadable};

match &self.sessions {
    Loadable::Loading => popover::redacted_rows("sessions", &theme, 3, Painter::of(cx), cx),
    Loadable::Error(message) => popover::error_row(&theme, message.clone())
        .child(retry_button)
        .into_any_element(),
    Loadable::Ready(sessions) => rows(sessions).into_any_element(),
    Loadable::Idle => gpui::Empty.into_any_element(),
}
```

- The `Painter` is the view that renders the rows; it keeps that view repainting while they are mounted.
- `error_row` paints the message only; the retry control is a child you add.
