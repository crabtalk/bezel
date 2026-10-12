---
title: Context menu
description: A view's menus in one field — opened at a right press or under a trigger, keyed so one is up at a time, closing itself and handing the keys back.
---

```rust
use ui::context_menu::{ContextMenu, Place};

#[derive(Clone, Copy, PartialEq)]
enum Menus { Row(usize), Add }

struct List {
    menu: ContextMenu<Self, Menus>,
}
```

## At a right press

```rust
.on_mouse_down(MouseButton::Right, cx.listener(move |view, event: &MouseDownEvent, window, cx| {
    view.menu.open(
        Menus::Row(ix),
        Place::At(event.position),
        vec![Item::action("Rename"), Item::action("Delete")],
        move |view, path, window, cx| view.act(ix, path, window, cx),
        window,
        cx,
    );
}))
// ...
.children(view.menu.render_for(&Menus::Row(ix), "row-menu", |view| &mut view.menu, window, cx))
```

## Under a trigger

```rust
let button = div()
    .id("add")
    .relative()
    .on_click(cx.listener(|view, _, window, cx| {
        ContextMenu::toggle(
            view,
            |view| &mut view.menu,
            Menus::Add,
            Place::Below,
            |view, window, cx| (view.add_items(window), View::add),
            window,
            cx,
        )
    }))
    .children(self.menu.render_for(&Menus::Add, "add-menu", |view| &mut view.menu, window, cx));
ContextMenu::trigger(button, |view| &mut view.menu, Menus::Add, cx)
```

`trigger` notes each press, so a press on the trigger of the menu that is up shuts it rather than opening it again. The rows are built when the menu opens.

`Place::Below`, `BelowEnd`, `Above` and `AboveEnd` hang the card off the element `render_for` is a child of; `Place::At` takes a window point.

## Keys

While open the menu holds the keyboard: `up` and `down` walk the rows, `right` or `enter` on a submenu row opens it, `left` closes it, `enter` chooses, and `escape` backs out of a submenu before it closes the menu. Choosing a row or `escape` hands the focus back to what held it before the menu opened; a press elsewhere closes it and leaves the focus to that press.

## Sizing the card

```rust
let card = view.menu.card("outline", |view| &mut view.menu, window, cx)?
    .max_h(px(320.0));
Some(view.menu.place("outline", card))
```

## API

```rust
// ui::context_menu

pub enum Place { At(Point<Pixels>), Below, BelowEnd, Above, AboveEnd }

pub struct ContextMenu<V, K = ()>;

impl<V, K: PartialEq> ContextMenu<V, K> {
    pub fn open(&mut self, key: K, place: Place, items: Vec<Item>,
        on_choose: impl Fn(&mut V, &[usize], &mut Window, &mut Context<V>) + 'static,
        window: &mut Window, cx: &mut Context<V>);
    pub fn toggle<F>(view: &mut V, slot: Slot<V, K>, key: K, place: Place,
        build: impl FnOnce(&mut V, &mut Window, &mut Context<V>) -> (Vec<Item>, F),
        window: &mut Window, cx: &mut Context<V>);
    pub fn trigger<E: StatefulInteractiveElement>(el: E, slot: Slot<V, K>, key: K, cx: &Context<V>) -> E;
    pub fn close(view: &mut V, slot: Slot<V, K>, cx: &mut Context<V>);
    pub fn key(&self) -> Option<&K>;
    pub fn showing(&self, key: &K) -> bool;
    pub fn point_at(&mut self, path: &[usize]) -> bool;
    pub fn clear_cursor(&mut self);
    pub fn card(&self, id, slot, window, cx) -> Option<Div>;
    pub fn place(&self, id, card: impl IntoElement) -> AnyElement;
    pub fn render(&self, id, slot, window, cx) -> Option<AnyElement>;
    pub fn render_for(&self, key: &K, id, slot, window, cx) -> Option<AnyElement>;
}
```
