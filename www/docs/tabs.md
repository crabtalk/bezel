---
title: Tabs
description: A hairline-underlined strip whose active tab is marked by an underline that overlaps the hairline.
prompt: Put General, Network and Advanced tabs at the top of the settings page and show the matching section under them.
---

```rust
use ui::widgets::Layout;

theme.tab_bar().children(TABS.iter().enumerate().map(|(index, label)| {
    theme.tab(*label, index == self.tab)
        .id(SharedString::from(format!("tab-{index}")))
        .on_click(cx.listener(move |view, _, _, cx| view.select(index, cx)))
}))
```

- The bar holds the tabs only; render the selected panel under it yourself.
- Keys are the caller's: wrap each tab in `focus::focusable` and handle `focus::Decrement` / `focus::Increment` for `←` / `→`. Move focus with the selection.
- Tabs that open and close are [tab strip](/docs/tab-strip); picking a value is [toggle group](/docs/toggle-group).
