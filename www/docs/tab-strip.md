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
pub fn bar(id: impl Into<ElementId>) -> Stateful<Div>;

/// One tab, up to the `×`.
pub fn tab(theme: &Theme, key: impl Into<SharedString>, label: Label, state: State) -> Stateful<Div>;

/// The `×` for the `key` its tab was built with.
pub fn close(theme: &Theme, key: impl Into<SharedString>, when: Close) -> Stateful<Div>;
```

One `key` names both the element and the hover group `Close::OnHover` reads, so the two cannot drift apart.

`State` has three cases because a window can hold several strips. `Resting` is a background tab; `Front` is the tab its own strip is on; `Focused` is the one holding the keyboard. A background pane's front tab still has to say what is under it.

`Label` carries the text, and optionally a leading glyph, an unsaved dot and a trailing badge:

```rust
Label::new(path.file_name()).with_icon(glyph::File).dirty(buffer.unsaved()).with_badge("#12")
```

The dot sits outside the truncating label, so a long name cannot hide it. The badge does not truncate — keep it to a few characters.

Drag is the caller's: the payload belongs to the app, and a bar that is itself a drop target is a pane layout's business rather than every strip's.
