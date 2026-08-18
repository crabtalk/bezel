---
title: Empty state
description: The centred "nothing here yet" panel — icon, headline, one line of hint.
---

```rust
use ui::{icons, widgets};

widgets::empty_state(
    &theme,
    icons::FOLDER,
    "No repositories",
    "Open a folder to get started.",
)
```

Three fixed slots: a 24px icon, a headline, and one line of hint saying what to do next. It fills its parent's width and centres in it, which is why it usually goes inside a `group_box`.

There is no action slot — it returns a plain `Div`, so a button under the hint is a child you add.
