---
title: Tag
description: A removable chip — a token in a filter bar or a recipient field, with the ✕ wired by the caller.
prompt: Show each active filter as a removable tag above the results, and drop the filter when its ✕ is clicked.
---

```rust
use ui::widgets::Content;

theme.tag(filter.name.clone())
    .id(("filter", index))
    .on_click(cx.listener(move |view, _, _, cx| view.remove_filter(index, cx)))
```

- The ✕ is painted, not wired. The caller adds the click handler.
- `theme.chip(label, tint)` is the same name on a coloured wash, with no ✕; it truncates once the caller caps its width.
