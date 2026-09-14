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

| | |
| --- | --- |
| `empty_state(icon, title, hint)` | Fills its parent's width and centres in it — usually inside a `group_box`. |
