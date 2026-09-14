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

## API

| | |
| --- | --- |
| `popover_card(theme)` | The card. `menu_heading`, `menu_row(theme, active, fade)` and `divider()` go in it. |
| `anchored_menu(id, card, closing)` | Pins to the trigger's top-left. `_below` for dropdowns, `_above` / `_above_end` near the window's bottom and right edges — gpui does not flip sides for you. |
| `menu::card(theme, id, items, cursor, cx, on_hit)` | Paints every panel; the caller holds one `menu::Cursor`. |
| `Hit` | Carries a path — one row index per level, outermost first. `menu::at(items, path)` turns it back into the `Item`. |
| `Hit::Dismiss` | Comes back from the card rather than from `.on_mouse_down_out`: with a submenu open, a click on its rows lands outside the parent. |
| `Cursor::step` / `descend` / `ascend` / `lit(depth)` | Keyboard travel, pure and tested on their own. |
| `close_popup(view, cx, |v| &mut v.menu)` | Begin and schedule the close together. |

Every layer occludes — hitboxes are paint-order only in gpui, so without it a click on a row would also fire whatever sits underneath.
