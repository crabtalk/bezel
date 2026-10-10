---
title: Multi-select
description: Several names chosen from a shared set, with the set managed in place — Notion's multi-select as a popover body.
---

```rust
use ui::multi_select::{self, Check, Choice, MultiSelect, MultiSelectEvent};

multi_select::init(cx);   // once, at startup, alongside input::init

let labels = cx.new(|cx| {
    MultiSelect::new(choices, cx)
        .with_create(normalize)
        .with_manage()
});

cx.subscribe(&labels, |view, picker, event, cx| match event {
    MultiSelectEvent::Toggled { name, on } => { /* apply, then set_choices */ }
    MultiSelectEvent::Dismissed => { /* close the popover */ }
    _ => {}
})
.detach();
```

The picker is the popover's body: it paints its own card, and the host mounts it under whatever trigger opens it through one of `popover`'s anchored layers, calling `reset` as it opens. One picker serves a table cell, a column heading and a toolbar button alike.

The host owns the data. Every event is a request, and the picker paints what `set_choices` last handed it.

## API

```rust
// ui::multi_select

/// Once at startup, alongside `input::init`.
pub fn init(cx: &mut App);

/// A colour for a name, the same on every machine.
pub fn tint(theme: &Theme, name: &str) -> Hsla;

pub enum Check { Off, On, Mixed }

pub struct Choice {
    pub name: SharedString,
    pub check: Check,
    /// Painted after the name — how many entries carry it.
    pub count: Option<usize>,
}

impl MultiSelect {
    pub fn new(choices: Vec<Choice>, cx: &mut Context<Self>) -> Self;

    /// A "Create" row for a query naming no choice. `normalize` turns the
    /// query into the name, or `None` for one that cannot be a name.
    pub fn with_create(self, normalize: impl Fn(&str) -> Option<SharedString> + 'static) -> Self;

    /// A `···` on each row, opening a pane to rename or delete it.
    pub fn with_manage(self) -> Self;

    /// Colour names with `tint` instead of `multi_select::tint`.
    pub fn with_tint(self, tint: impl Fn(&str, &Theme) -> Hsla + 'static) -> Self;

    pub fn set_choices(&mut self, choices: Vec<Choice>, cx: &mut Context<Self>);

    /// An empty query, the list pane, focus in the query.
    pub fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>);
}

pub enum MultiSelectEvent {
    Toggled { name: SharedString, on: bool },
    Created(SharedString),
    Renamed { from: SharedString, to: SharedString },
    Deleted(SharedString),
    Dismissed,
}
```

Choices that are on sit as chips in the query line, each with a ×. Backspace in an empty query turns off the last of them.

Rows show a check for on and a dash for mixed. Choosing a mixed row turns it on. Choosing any row clears the query.

The Create row sits first while the normalised query matches no choice exactly, and Enter takes it. A rename is normalised the same way, and may name a choice that already exists. Delete is reported as asked, unconfirmed.

Keys are `up`/`down`, `ctrl-p`/`ctrl-n`, `enter` and `escape`. Escape in the manage pane returns to the list; in the list it reports `Dismissed`, as does a press outside the card.

The list shows twelve rows and scrolls past that.
