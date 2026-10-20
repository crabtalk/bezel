---
title: Context menu
description: A view's menus in one field — opened at a right press or under a trigger, keyed so one is up at a time, closing itself and handing the keys back.
prompt: Add a right-click menu with Rename and Delete to each row of my file list.
---

```rust
use ui::context_menu::{ContextMenu, Place};
use ui::menu::Item;

// self.menu: ContextMenu<Self, usize>
div().id(("row", ix))
    .on_mouse_down(MouseButton::Right, cx.listener(move |view, event: &MouseDownEvent, window, cx| {
        view.menu.open(ix, Place::At(event.position),
            vec![Item::action("Rename"), Item::action("Delete")],
            move |view, path, window, cx| view.act(ix, path, window, cx),
            window, cx);
    }))
    .children(self.menu.render_for(&ix, "row-menu", |view| &mut view.menu, window, cx))
```

- The keys need `ui::menu::init(cx)` at startup.
- One `ContextMenu` serves every menu in a view; the key names which is open, and opening one closes the rest.
- A button that opens a menu calls `ContextMenu::toggle` and is wrapped in `ContextMenu::trigger`, or the press that shuts the menu opens it again.
- Every `Place` but `At` hangs off the element `render_for` is a child of, which is `relative()`. For a sized card, use `card` then `place`.

## Keys

| key | does |
| --- | --- |
| `up` / `down` | walk the rows |
| `right`, `enter` on a submenu row | open it |
| `left` | close the submenu |
| `enter` | choose the row |
| `escape` | close the submenu, then the menu |
