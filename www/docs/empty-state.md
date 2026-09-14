---
title: Empty state
description: The centred "nothing here yet" panel — icon, headline, one line of hint.
---

```rust
use ui::{icons, widgets::Content};

theme.empty_state(
    icons::glyph::Folder,
    "No repositories",
    "Open a folder to get started.",
)
```

No action slot: it returns a `Div`, so a button under the hint is a child you add.

## API

```rust
pub trait Content: ThemeExt {
    /// Fills its parent's width and centres in it — usually inside a `group_box`.
    fn empty_state(
        &self,
        icon: impl Into<Icon>,
        title: impl Into<SharedString>,
        hint: impl Into<SharedString>,
    ) -> Div;

    // ...
}
```
