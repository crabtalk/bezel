---
title: Menu
description: The floating card, its rows, headings and dividers — plus the anchored layers that hang one off a trigger.
---

A menu is a card and a list of rows, assembled by the caller:

```rust
use ui::popover;

popover::popover_card(&theme).w(px(240.0)).children([
    popover::menu_heading(&theme, "Section").into_any_element(),
    popover::menu_row(&theme, false, "m-one").child("First item").into_any_element(),
    popover::menu_row(&theme, true, "m-two").child("Active item").into_any_element(),
    popover::divider().into_any_element(),
])
```

`menu_row` takes a fade key — unique app-wide and stable across frames; the row's id string is a good choice — which is what the hover wash blends against. `menu_row_nav` distinguishes the keyboard cursor from the selection, so two rows never look selected at once.

To float it, hang an anchored layer off the trigger while open:

```rust
trigger.child(popover::anchored_menu_below("theme-menu", card))
```

`anchored_menu` pins to the trigger's top-left, which reads right for a context-style menu and covers a button-shaped trigger — hence `anchored_menu_below` for dropdowns, `anchored_menu_above` for anything near the window's bottom edge, and `anchored_menu_above_end` when a right-side trigger would otherwise run off the window. gpui's `anchored` does not flip sides for you; the caller picks.

Every layer occludes. Hitboxes are paint-order only in gpui, so without it a click on a menu row would *also* fire whatever clickable sits underneath.

Dismissal is the caller's `.on_mouse_down_out` on the card. To animate the close rather than have the menu vanish, hold the state in a `Popup`:

```rust
if self.menu.begin_close() {
    popover::reap_popup(cx, |view: &mut Self| &mut view.menu);
}
```

gpui unmounts an element the frame its state drops, so a closing animation needs the state held alive while `menu-out` plays. `Popup` is that hold: `is_open` for logic — a closing popup already reads as closed — and `get`/`is_closing` for rendering, with `reap_popup` scheduling the drop once the exit's span is up.

## Submenus

`menu::card` takes rows rather than elements, and a row can be a menu of its own — the shape a SwiftUI `Menu` nests in a `Menu`:

```rust
menu::Item::submenu("Open Recent", vec![
    menu::Item::action("bezel.md"),
    menu::Item::Separator,
    menu::Item::action("Clear Menu"),
])
```

The panels are painted by `card` itself, so nesting costs the caller nothing beyond one piece of state: a `menu::Cursor`, holding which submenus are down and which row is live.

```rust
menu::card(&theme, "file-menu", &items, &self.cursor, cx, |view, hit, _, cx| match hit {
    menu::Hit::Point(path) => {
        if view.cursor.point_at(&items, &path) { cx.notify() }
    }
    menu::Hit::Choose(path) => view.run(path, cx),
    menu::Hit::Dismiss => view.close(cx),
})
```

A `Hit` carries a **path** — one row index per level, outermost first. `menu::at(items, path)` turns it back into the `Item`.

One cursor, both devices. The pointer moves the cursor rather than lighting a row of its own, so an open submenu can only ever hang off the row that is live; a menu that tracked hover separately could light two rows and hang a panel off neither. Pointing at a submenu row is what opens it, which is why hover, click and `right` all arrive as `Hit::Point`.

`Cursor::descend` / `ascend` are the keyboard's two levels of travel, `step` walks rows inside the innermost panel, and `lit(depth)` is which row each panel draws lit. They are pure and tested on their own.

Dismissal comes back as `Hit::Dismiss` rather than being left to an `.on_mouse_down_out` on the card, because a card sees only its own bounds: with a submenu open, a click on one of its rows lands *outside* the parent and would read as a click away. Each panel reports the press it did not contain, and the press no panel contained is the one that dismisses.

`anchored_submenu` is the layer they hang on — pinned to the row's top-right and pulled back by the card's inset, so the child's first row lines up with the row that opened it and the two cards touch. The gap is zero on purpose: a strip of nothing between them is a strip the pointer crosses on its way in, and it would land on a sibling row and close what it was reaching for. gpui allows ten levels of nested deferred draws, which is the ceiling on nesting depth.

A submenu with nothing selectable in it is not selectable itself — opening it would drop a panel that is a dead end — so the keyboard steps over it like any other dead row.

The pure parts are separate and tested on their own: `menu_step` wraps the active row at both ends, `filter_indices` ranks prefix matches ahead of substring matches, and `Filter` holds the items, the ranked view and the active row for every picker in the library.
