---
title: Skeleton
description: Pulsing placeholder rows for a list that is still loading, staggered off the shared pulse clock.
---

```rust
use bezel_ui::popover;

popover::redacted_rows("recent-sessions", &theme, 3, cx.entity_id(), cx)
```

It takes the calling view's `EntityId` because the pulse is driven by a shared 30fps clock rather than by a per-element animation: the id is what leases this view onto the tick list and what lets the clock park when the last skeleton unmounts. Every row across every view shares one epoch, so nothing beats out of phase with anything else.

Rows are staggered — each one enters the wave a little after the one above it — which is what makes a stack read as loading rather than as three boxes blinking together.

`popover::Loadable<T>` is the state this pairs with: `Idle` (never requested) → `Loading` (these rows) → `Ready(T)` or `Error(String)`, with `popover::error_row` painting the last one — a plain `Div`, so the retry control is a child you add and wire.
