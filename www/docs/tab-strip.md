---
title: Tab strip
description: A row of open things, one in front, each closable — with the order and the activation held apart from the paint.
prompt: Give the editor pane a tab per open file, with a close button on hover and tabs I can drag to reorder.
---

```rust
use ui::tabs::{self, Close, State};

self.reorder.bar("files", &self.strip, self.strip.tabs().iter().map(|id| {
    let id = *id;
    let state = if self.strip.active() == Some(&id) { State::Focused } else { State::Resting };
    let tab = tabs::tab(&theme, self.key(id), self.label(id), state)
        .on_click(cx.listener(move |view, _, _, cx| view.show(id, cx)))
        .child(tabs::close(&theme, self.key(id), Close::OnHover)
            .on_click(cx.listener(move |view, _, _, cx| view.close(id, cx))));
    (id, tab)
}))
.on_reorder(cx.listener(|view, step: &tabs::Move, _, cx| {
    view.strip.reorder(step.from, step.to);
    cx.notify();
}))
```

- `Strip<Id>` holds the order and the front tab; `self.strip.open(id)` / `close(&id)` keep exactly one in front. `tabs::bar(id)` is the same row without reordering.
- Pass the same `key` to `tab` and `close`, and keep it stable. Don't chain `.hover(..)` on a tab; it already has one.
- A control inside a tab stops mouse-down propagation, as `close` does, or pressing it picks up the tab.
- Apply `on_reorder` synchronously. `on_drop_outside` gets a tab released off the strip; mount strips in a [docking surface](/docs/docking) for splits.
