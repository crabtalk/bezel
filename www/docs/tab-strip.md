---
title: Tab strip
description: A row of open things, one in front, each closable — with the order and the activation held apart from the paint.
---

```rust
use ui::tabs::{self, Close, Label, State, Strip};

tabs::bar("panel-tabs").children(self.strip.tabs().iter().map(|id| {
    let id = *id;
    let state = match self.strip.active() == Some(&id) {
        true => State::Focused,
        false => State::Resting,
    };
    tabs::tab(&theme, id, Label::new(self.title(id)), state)
        .on_click(cx.listener(move |view, _, _, cx| view.show(id, cx)))
        .child(
            tabs::close(&theme, id, Close::OnHover)
                .on_click(cx.listener(move |view, _, _, cx| view.close(id, cx))),
        )
}))
```

Not [tabs](/docs/tabs), which switches between sections of one page, and not [toggle group](/docs/toggle-group), which picks a value out of a fixed set. A tab here has identity: it arrives, it can be closed, it can be dragged past its neighbour.

## The model

`Strip<Id>` is the order and the front, and it imports no gpui — the rules are `Vec` arithmetic, testable without a window. `Id` is whatever names a tab to its owner; what a tab *opens* never enters the crate.

```rust
pub struct Strip<Id> { /* .. */ }

impl<Id: Clone + PartialEq> Strip<Id> {
    /// Left to right.
    pub fn tabs(&self) -> &[Id];
    pub fn active(&self) -> Option<&Id>;

    /// Bring to the front, adding at the end if it is not there. An id that
    /// already is keeps its place.
    pub fn open(&mut self, id: Id);
    pub fn activate(&mut self, id: &Id) -> bool;

    /// Closing the front tab hands the front to its right-hand neighbour, or
    /// to the new last tab when it had none. Closing any other tab leaves the
    /// front where it is.
    pub fn close(&mut self, id: &Id) -> bool;

    /// Wraps at both ends.
    pub fn cycle(&mut self, step: isize);

    /// The front is held by identity, so this never changes which tab is in
    /// front.
    pub fn reorder(&mut self, from: usize, to: usize);
}
```

A strip with tabs in it always has one in front: `active` is `None` only while `is_empty`.

## The paint

```rust
/// Tabs go in it; a `+`, a `···` and anything else on the row are the
/// caller's, outside this. It scrolls sideways once the tabs stop fitting.
pub fn bar(id: impl Into<ElementId>) -> TabBar;

/// One tab, up to the `×`.
pub fn tab(theme: &Theme, key: impl Into<SharedString>, label: Label, state: State) -> Stateful<Div>;

/// The `×` for the `key` its tab was built with.
pub fn close(theme: &Theme, key: impl Into<SharedString>, when: Close) -> Stateful<Div>;
```

Both `bar` and `Reorder::bar` own a horizontal overlay scrollbar. It follows the
app's scrollbar visibility setting, takes no layout space, and disappears when
the tabs fit. Plain `bar` keeps its offset across renders; `track_scroll` is
optional for programmatic access. No host wrapper is needed.

One `key` names both the element and the hover group `Close::OnHover` reads, so the two cannot drift apart.

`State` has three cases because a window can hold several strips. `Resting` is a background tab; `Front` is the tab its own strip is on; `Focused` is the one holding the keyboard. A background pane's front tab still has to say what is under it.

Every tab but a `Focused` one washes on hover — a `Front` tab in a background pane is still one click from the keyboard. `tab` takes that `hover` itself, and gpui panics on a second, so reach for a `group_hover` rather than chaining one on.

`Label` carries the text, and optionally a leading glyph, a mark and a trailing badge:

```rust
Label::new(path.file_name())
    .with_icon(glyph::File)
    .mark(Icon::glyph(glyph::CircleSmall).solid())
    .with_badge("#12")
```

The mark is whatever the tab has to say beside its name — unsaved work, a running job, something unread. The crate names no glyph for it: `ui` carries only the icon categories it paints itself, and Cargo unions features down the graph, so a default here would put a category on the floor of every app that depends on `bezel-ui`. Pass one your app already pays for. It paints at `MARK_SIZE` in the tab's own tone, outside the truncating label so a long name cannot hide it; Lucide's round glyphs are outlines, and `Icon::solid` fills one.

The badge does not truncate — keep it to a few characters.

## Live reordering

Keep a `tabs::Reorder<Id>` beside the model, initialized with
`tabs::Reorder::new(motion::Painter::of(cx))`. It owns the pointer gesture and
animations; the host applies moves to its data:

```rust
self.reorder.bar("panel-tabs", &self.strip,
    self.strip.tabs().iter().map(|id| {
        (id.clone(), tabs::tab(&theme, self.key(id), self.label(id), self.state(id)))
    }),
).on_reorder(cx.listener(|view, movement: &tabs::Move, _, cx| {
    view.strip.reorder(movement.from, movement.to);
    cx.notify();
}))
```

Supply children in model order with stable keys. Keep activation and close
handlers on the tabs. The preview moves live, but `on_reorder` fires once on
release. Apply that move synchronously; the active tab stays active.

The drag is gpui's, carrying a `drag::Carry<Id>`; its preview is empty and
the carried tab follows the pointer in place. Neighbours slide into the gap;
releasing settles the tab. Reduced motion skips slides. Do not add `on_drag`
to the tabs.
Custom buttons inside a tab should stop mouse-down propagation, as `tabs::close`
already does, so pressing them does not pick up the tab.

For cross-pane moves, add `.on_drop_outside(cx.listener(...))`. It receives an
`OutsideDrop<Id>` with the tab id and the release position in window coordinates;
the host resolves the destination pane and moves its data. Without this hook,
an outside release cancels the local reorder. Escape and host edits to the
strip order also cancel.

This is one axis-locked region of [sortable lists](/docs/sortable). Use that
component for moves between lists.

For pane splits and joins, mount the strips inside
[a docking surface](/docs/docking) of the same item type. A tab leaves its
strip once the pointer is 12px off it across the strip's axis.
