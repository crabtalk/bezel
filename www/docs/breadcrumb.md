---
title: Breadcrumb
description: A trail of crumbs and chevrons, assembled by the caller — the last one is current and stops looking clickable.
---

```rust
use ui::widgets::Content;

theme
    .breadcrumb()
    .child(theme.breadcrumb_item("crates", false))
    .child(theme.breadcrumb_separator())
    .child(theme.breadcrumb_item("ui", false))
    .child(theme.breadcrumb_separator())
    .child(theme.breadcrumb_item("widgets.rs", true))
```

The separators are children rather than something the container inserts, because a trail that collapses in the middle — `crates / … / widgets.rs` — is the caller's decision about its own path, not a rule the container could apply.

A `current` crumb takes the text tone and drops the pointer cursor. The rest truncate individually, and the container sets `min_w_0` so a long path shortens rather than pushing its row wide.
