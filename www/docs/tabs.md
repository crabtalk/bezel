---
title: Tabs
description: A hairline-underlined strip whose active tab is marked by an underline that overlaps the hairline.
---

```rust
use ui::widgets::Layout;

theme.tab_bar().children(TABS.iter().enumerate().map(|(index, label)| {
    theme.tab(*label, index == self.tab)
        .id(SharedString::from(format!("tab-{index}")))
        .on_click(cx.listener(move |view, _, _, cx| view.select(index, cx)))
}))
```

Which panel a tab shows is the caller's: `tab_bar` is a strip, not a container that swallows its content.

## API

```rust
pub trait Layout: ThemeExt {
    /// The strip and its hairline.
    fn tab_bar(&self) -> Div;

    /// Active takes the text tone, a medium weight, and a 2px underline *over*
    /// the hairline. Nothing changes the row's height, so switching never
    /// nudges the content below.
    fn tab(&self, label: impl Into<SharedString>, active: bool) -> Div;

    // ...
}
```

Like every control in `widgets`, a tab keeps a 1px border it usually paints nothing into — the slot `focus::focusable` fills with the ring, always present so the label never shifts by a pixel when focus arrives.
