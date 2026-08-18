---
title: Tabs
description: A hairline-underlined strip whose active tab is marked by an underline that overlaps the hairline.
---

```rust
use ui::widgets;

widgets::tab_bar(&theme).children(TABS.iter().enumerate().map(|(index, label)| {
    widgets::tab(&theme, *label, index == self.tab)
        .id(SharedString::from(format!("tab-{index}")))
        .on_click(cx.listener(move |view, _, _, cx| view.select(index, cx)))
}))
```

The active tab carries the text tone, a medium weight, and a 2px underline that sits *over* the bar's hairline rather than under it. Nothing about it changes the row's height, so switching tabs never nudges the content below.

Like every control in `widgets`, a tab keeps a 1px border it usually paints nothing into — that is the slot `focus::focusable` fills with the focus ring, and it is always there so the label never shifts by a pixel when focus arrives. The underline's insets carry that pixel too, which is why it still spans the tab's full width.

Which panel a tab shows is the caller's: `tab_bar` is a strip, not a container that swallows its content.
