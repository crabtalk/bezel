---
title: Multi-select
description: Several names chosen from a shared set, with the set managed in place — Notion's multi-select as a popover body.
prompt: Add a labels picker to each row of the issue table, where I can toggle labels, create new ones, and rename or delete them in place.
---

```rust
use ui::multi_select::{self, MultiSelect, MultiSelectEvent};

multi_select::init(cx);   // once, at startup, alongside input::init

let labels = cx.new(|cx| MultiSelect::new(choices, cx).with_create(normalize).with_manage());

cx.subscribe(&labels, |view, picker, event, cx| match event {
    MultiSelectEvent::Toggled { name, on } => { /* apply, then set_choices */ }
    MultiSelectEvent::Dismissed => { /* close the popover */ }
    _ => {}
})
.detach();
```

- The picker is the popover's body: mount it under your trigger through a `popover` anchored layer, and call `reset` as it opens.
- Every event is a request. The picker paints what `set_choices` last handed it, so answer each one.
- Delete is reported unconfirmed; confirm it yourself if you need to.
