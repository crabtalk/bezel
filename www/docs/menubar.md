---
title: Menubar
description: The in-window bar of titles that drop menus, where one menu being open changes what the others do.
---

Not the *native* bar. On macOS that is `cx.set_menus` and four lines in `main`, which is where it belongs. This is the bar an app with a custom titlebar draws for itself, and the one every other platform expects to see inside the window.

```rust
use ui::menubar::{self, Item, Menu, Menubar, MenubarEvent};

menubar::init(cx);   // once, at startup

let bar = cx.new(|cx| Menubar::new(vec![
    Menu::new("File", vec![
        Item::action("New Window").with_keystroke("⌘N"),
        Item::Separator,
        Item::action("Close").with_keystroke("⌘W").disabled(),
    ]),
], cx));

cx.subscribe(&bar, |_, _, event, _| match event {
    MenubarEvent::Selected { menu, item } => { /* dispatch */ }
})
.detach();
```

What makes it a menubar rather than a row of dropdowns: sliding the pointer onto a sibling title switches to it with no click, and `left`/`right` cross between menus without leaving the keyboard.

The menus are data you hand over, shaped like gpui's own `Menu` and `MenuItem` so an app drawing both bars writes them the same way. It does not *take* those types — they carry a boxed action, and reporting an index leaves dispatch with the app.

The keystroke on an item is the accelerator to **print**. The binding itself is the app's and bezel never dispatches it; a menu that showed a keystroke it did not own would be documenting a lie.

`Item` is an enum rather than a struct with an `is_separator` flag: a separator has no label, no accelerator and nothing to enable, and every one of those fields would have to be answered anyway.

`menubar::next_selectable(items, from, delta)` is the row-stepping rule — separators and disabled rows are stepped straight over, both ends wrap, and `None` back means nothing in the menu can be landed on, which is the one shape that would otherwise spin forever.
