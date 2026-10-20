---
title: Menubar
description: The in-window bar of titles that drop menus, where one menu being open changes what the others do.
prompt: Draw a File and Edit menubar in my custom titlebar and run the item the user picks.
---

```rust
use ui::menu::Item;
use ui::menubar::{self, Menu, Menubar, MenubarEvent};

menubar::init(cx);   // once, at startup
let bar = cx.new(|cx| Menubar::new(vec![
    Menu::new("File", vec![
        Item::action("New Window").with_keystroke("⌘N"),
        Item::submenu("Open Recent", vec![Item::action("bezel.md")]),
        Item::action("Close").with_keystroke("⌘W").disabled(),
    ]),
], cx));
cx.subscribe(&bar, |view, bar, MenubarEvent::Selected { menu, path }, cx| {
    let item = bar.read(cx).menus()[*menu].at(path).cloned();
    view.run(item, cx);
}).detach();
```

- This is the bar drawn inside the window. The native macOS bar is gpui's `cx.set_menus`.
- `with_keystroke` only prints the chord; binding it is the app's. `with_shortcut` reads it off the keymap.
