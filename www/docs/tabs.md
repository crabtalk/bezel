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

This one switches between sections of one page, over a set fixed at compile time. Tabs that open and close are [tab strip](/docs/tab-strip); picking a *value* out of a fixed set is [toggle group](/docs/toggle-group).

## Keys

Nothing is bound here. `focus::focusable` puts each tab in the tab order, gives it the ring, and dispatches `Activate` on `enter` / `space`. `←` / `→` arrive as `focus::Decrement` and `focus::Increment` — the pair bezel dispatches for a focused control holding a value, which for a strip is which tab is open:

```rust
pressable(focus::focusable(&theme, &self.tabs[index], theme.tab(label, self.tab == index)), ..)
    .on_action(cx.listener(move |view, _: &focus::Decrement, window, cx| {
        view.open(index as isize - 1, window, cx)
    }))
    .on_action(cx.listener(move |view, _: &focus::Increment, window, cx| {
        view.open(index as isize + 1, window, cx)
    }))
```

Move the focus with the selection, or the next arrow starts from the tab you left.

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
