---
title: Menubar
description: The in-window bar of titles that drop menus, where one menu being open changes what the others do.
---

```rust
use ui::menu::Item;
use ui::menubar::{self, Menu, Menubar, MenubarEvent};

menubar::init(cx);   // once, at startup

let bar = cx.new(|cx| Menubar::new(vec![
    Menu::new("File", vec![
        Item::action("New Window").with_keystroke("⌘N"),
        Item::submenu("Open Recent", vec![
            Item::action("bezel.md"),
            Item::Separator,
            Item::action("Clear Menu"),
        ]),
        Item::Separator,
        Item::action("Close").with_keystroke("⌘W").disabled(),
    ]),
], cx));

cx.subscribe(&bar, |_, bar, event, cx| {
    let MenubarEvent::Selected { menu, path } = event;
    let item = bar.read(cx).menus()[*menu].at(path);   // /* dispatch */
})
.detach();
```

Not the *native* bar — on macOS that is `cx.set_menus` and four lines in `main`. This is the bar an app with a custom titlebar draws for itself.

## API

```rust
// ui::menubar

pub fn init(cx: &mut App);

impl Menubar {
    /// Sliding onto a sibling title switches to it with no click; `left` and
    /// `right` cross between menus.
    pub fn new(menus: Vec<Menu>, cx: &mut Context<Self>) -> Self;

    // ...
}

/// A path — one row index per level, outermost first. `Menu::at` turns it back
/// into the item.
pub enum MenubarEvent { Selected { menu: usize, path: Vec<usize> } }
```

```rust
// ui::menu — shaped like gpui's own `Menu` and `MenuItem`, so an app drawing
// both bars writes them the same way.

impl Item {
    pub fn action(label: impl Into<SharedString>) -> Self;
    pub fn submenu(label: impl Into<SharedString>, items: Vec<Item>) -> Self;

    /// The accelerator to **print**. The binding is the app's; bezel never
    /// dispatches it.
    pub fn with_keystroke(self, keystroke: impl Into<SharedString>) -> Self;

    pub fn checked(self, checked: bool) -> Self;
    pub fn disabled(self) -> Self;

    // ...
}

/// Steps over separators and disabled rows, wraps at both ends, and answers
/// `None` when nothing can be landed on.
pub fn next_selectable(items: &[Item], from: Option<usize>, delta: isize) -> Option<usize>;
```
