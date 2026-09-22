---
title: Menu
description: The floating card, its rows, headings and dividers — plus the anchored layers that hang one off a trigger.
---

```rust
use motion::{Fade, Painter};
use ui::popover;

let view = Painter::of(cx);

popover::popover_card(&theme).w(px(240.0)).children([
    popover::menu_heading(&theme, "Section").into_any_element(),
    popover::menu_row(&theme, false, Some(Fade::new(view, "m-one")))
        .child("First item")
        .into_any_element(),
    popover::menu_row(&theme, true, None)
        .child("Active item")
        .into_any_element(),
    popover::divider().into_any_element(),
])
```

`active` is the row the cursor is on, and a menu has exactly one cursor. Pass `Some(fade)` to let the mouse light rows by itself, `None` when the menu owns an active index and moves it from `on_mouse_move`.

## Floating it

```rust
trigger.child(popover::anchored_menu_below("theme-menu", card, self.menu.closing_since()))
```

## Closing

```rust
if self.menu.begin_close() {
    popover::reap_popup(self, cx, |view: &mut Self| &mut view.menu);
}
```

gpui unmounts an element the frame its state drops, so `Popup` is what holds it alive while `menu-out` plays.

## Described rows

```rust
menu::Item::action("Open…")
    .with_long_description("Choose a markdown file from this workspace to edit")
```

One clipped line under the title, which widens the panel to 280px. `with_tooltip` sets hover text alone, including why a disabled row is disabled.

## Submenus

```rust
menu::card(&theme, "file-menu", &items, &self.cursor, cx, |view, hit, _, cx| match hit {
    menu::Hit::Point(path) => {
        if view.cursor.point_at(&items, &path) { cx.notify() }
    }
    menu::Hit::Choose(path) => view.run(path, cx),
    menu::Hit::Dismiss => view.close(cx),
})
```

One cursor, both devices: the pointer moves the cursor rather than lighting a row of its own, so an open submenu can only hang off the row that is live. Hover, click and `right` all arrive as `Hit::Point`.

A panel opens to the right of the card it hangs on, or to its left where the window has no room on the right. Every panel of one open chain takes the side the first one took.

## API

```rust
// ui::popover

pub fn popover_card(theme: &Theme) -> gpui::Div;
pub fn menu_heading(theme: &Theme, label: impl Into<SharedString>) -> gpui::Div;
pub fn menu_row(theme: &Theme, active: bool, fade: Option<Fade>) -> gpui::Div;
pub fn divider() -> gpui::Div;

/// Pins to the trigger's top-left. `_below` for dropdowns, `_above` near the
/// window's bottom edge, and `_below_end` and `_above_end` right-aligned to the
/// trigger for a control at a row's end — gpui does not flip sides for you.
pub fn anchored_menu(
    id: impl Into<SharedString>,
    content: AnyElement,
    closing: Option<web_time::Instant>,
) -> AnyElement;

/// Begin and schedule the close together.
pub fn close_popup<V: 'static, T: 'static>(
    view: &mut V,
    cx: &mut gpui::Context<V>,
    popup: impl Fn(&mut V) -> &mut Popup<T> + Copy + 'static,
);

// ...
```

```rust
// ui::menu

/// Paints every panel; the caller holds one `Cursor`.
pub fn card<V: 'static>(
    theme: &Theme,
    id: impl Into<SharedString>,
    items: &[Item],
    cursor: &Cursor,
    cx: &mut Context<V>,
    on: impl Fn(&mut V, Hit, &mut Window, &mut Context<V>) + 'static,
) -> gpui::Div;

/// A hit carries a path — one row index per level, outermost first.
/// `Dismiss` comes back from the card rather than from `.on_mouse_down_out`:
/// with a submenu open, a click on its rows lands outside the parent.
pub enum Hit { Point(Vec<usize>), Choose(Vec<usize>), Dismiss }

pub fn at<'a>(items: &'a [Item], path: &[usize]) -> Option<&'a Item>;

impl Cursor {
    /// Keyboard travel, pure and tested on its own.
    pub fn step(&mut self, root: &[Item], delta: isize);
    pub fn descend(&mut self, root: &[Item]) -> bool;
    pub fn ascend(&mut self) -> bool;

    /// Which row each panel draws lit.
    pub fn lit(&self, depth: usize) -> Option<usize>;

    /// Hover, click and `right` all arrive here.
    pub fn point_at(&mut self, root: &[Item], path: &[usize]) -> bool;

    // ...
}
```

Every layer occludes — hitboxes are paint-order only in gpui, so without it a click on a row would also fire whatever sits underneath.
