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

The pure parts are separate and tested on their own: `menu_step` wraps the active row at both ends, `filter_indices` ranks prefix matches ahead of substring matches, and `Filter` holds the items, the ranked view and the active row for every picker in the library.
