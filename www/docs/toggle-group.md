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

Reach for this over a [select](/docs/select) when there are few enough choices that a menu would be overkill. It picks a *value*; switching between sections of one page is [tabs](/docs/tabs), and things that open and close are [tab strip](/docs/tab-strip).

A segment can carry a glyph instead of a word, for a control with no room for one — a view switcher on a board:

```rust
theme.toggle_group().children([(glyph::SquareKanban, "Lanes"), (glyph::LayoutList, "List")]
    .into_iter()
    .enumerate()
    .map(|(index, (icon, label))| {
        theme.toggle_group_icon(icon, self.view == index)
            .id(("view", index))
            .tooltip(move |window, cx| Tooltip::text(label, window, cx))
    }),
)
```

The tooltip is yours, and a glyph nobody recognises says nothing without one.

## API

```rust
pub trait Controls: ThemeExt {
    /// The track. Sets `self_start`, or flexbox's default `stretch` blows it
    /// out to a column's full width.
    fn toggle_group(&self) -> Div;

    /// Exactly one reads as pressed; the rest stay bare until the pointer is
    /// on them.
    fn toggle_group_item(&self, label: impl Into<SharedString>, selected: bool) -> Div;

    /// The same segment carrying a glyph. Comes out 24×24, the height a
    /// labelled segment takes.
    fn toggle_group_icon(&self, icon: impl Into<Icon>, selected: bool) -> Div;

    // ...
}
```

An unselected segment takes its own `hover`, and gpui panics on a second one — reach for a `group_hover` rather than chaining `.hover(..)` on.

Segment corners derive from the track's radius and its inset — both numbers are read at both ends, so a segment cannot stop being concentric with its track.
