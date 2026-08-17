---
title: Toggle group
description: A segmented control — one pill of mutually exclusive choices, where the selected segment carries a raised plate.
---

The track is a container and each segment is a child, so the caller keeps the list and the selection:

```rust
use bezel_ui::widgets;

widgets::toggle_group(&theme).children(
    ["Day", "Week", "Month"].into_iter().enumerate().map(|(index, label)| {
        widgets::toggle_group_item(&theme, label, self.segment == index)
    }),
)
```

Exactly one segment reads as pressed: the selected one gets the raised plate, the rest stay bare.

The track sets `self_start`. A segmented control has to hug its segments, and dropped into a `flex_col`, flexbox's default `align-items: stretch` would blow it out to the column's full width.

Segment corners are derived from the track's radius and the inset it comes in by — both numbers are read at both ends, so a segment cannot stop being concentric with the track it sits in.

Reach for this over a select when there are few enough choices that a menu would be overkill.
