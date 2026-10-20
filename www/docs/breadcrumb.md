---
title: Breadcrumb
description: A trail of crumbs and chevrons, assembled by the caller — the last one is current and stops looking clickable.
prompt: Show the open file's path as a breadcrumb above the editor, each folder clickable to reveal it in the sidebar.
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

- The caller adds the separators and any click handlers; collapsing a long trail is the caller's too.
- Mark only the last crumb `current`.
