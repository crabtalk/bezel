---
title: Empty state
description: The centred "nothing here yet" panel — icon, headline, one line of hint.
prompt: When the project has no repositories, show an empty state with a folder icon and an "Open folder" button under it.
---

```rust
use ui::{icons, widgets::{Button, Content}};

theme
    .empty_state(icons::glyph::Folder, "No repositories", "Open a folder to get started.")
    .child(Button::new("open", "Open folder").on_press(cx.listener(|view, _, _, cx| view.open(cx))))
```

- There is no action slot; an action is a child you add.
- It fills its parent's width and centres in it.
