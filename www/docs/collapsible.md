---
title: Collapsible
description: A disclosure header — chevron plus title — with the body rendered by the caller, and a flag that can follow something until you take it over.
prompt: Add a collapsible "Details" section to the run view that opens itself while the run is going, until I click it.
---

```rust
use ui::widgets::{Layout, Takeover};

let open = self.details.get(self.running);   // self.details: Takeover
div()
    .child(div().id("details")
        .on_click(cx.listener(|view, _, _, cx| {
            view.details.toggle(view.running);
            cx.notify();
        }))
        .child(theme.collapsible_header("Details", open)))
    .when(open, |el| el.child(body))
```

- The header is a row, not a container: render the body yourself. A plain `bool` works where nothing drives it.
- `Takeover` follows its `auto` value until the first `toggle`, then holds the user's choice. Pass the same `auto` to `get` and `toggle`.
- Hover is the caller's; `Theme::element_hover` is the wash.
