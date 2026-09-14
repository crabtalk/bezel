---
title: Toggle group
description: A segmented control — one pill of mutually exclusive choices, where the selected segment carries a raised plate.
---

```rust
use ui::widgets::Controls;

theme.toggle_group().children(
    ["Day", "Week", "Month"].into_iter().enumerate().map(|(index, label)| {
        theme.toggle_group_item(label, self.segment == index)
    }),
)
```

Reach for this over a [select](/docs/select) when there are few enough choices that a menu would be overkill.

## API

| | |
| --- | --- |
| `toggle_group()` | The track. Sets `self_start`, or flexbox's default `stretch` blows it out to a column's full width. |
| `toggle_group_item(label, selected)` | Exactly one reads as pressed; the rest stay bare. |

Segment corners derive from the track's radius and its inset — both numbers are read at both ends, so a segment cannot stop being concentric with its track.
