---
title: Toggle group
description: A segmented control — one pill of mutually exclusive choices, where the selected segment carries a raised plate.
prompt: Add a Day / Week / Month switcher above the calendar view.
---

```rust
use ui::widgets::Controls;

theme.toggle_group().children(
    ["Day", "Week", "Month"].into_iter().enumerate().map(|(index, label)| {
        theme.toggle_group_item(label, self.segment == index)
            .id(("range", index))
            .on_click(cx.listener(move |view, _, _, cx| view.set_segment(index, cx)))
    }),
)
```

- An unselected segment carries its own `hover`; a second `.hover(..)` panics. Use `group_hover`.
- `toggle_group_icon` segments need a tooltip from the caller.
- `theme.segmented(id, labels, selected, on_pick)` is the same control with the clicks wired.
