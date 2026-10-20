---
title: Menu
description: The floating card, its rows, headings and dividers — plus the anchored layers that hang one off a trigger.
prompt: Add a Sort dropdown under my list header with Name and Date rows that closes when I click away.
---

```rust
use motion::{Fade, Painter};
use ui::popover::{self, Popup};

// self.menu: Popup<()>
let card = popover::dismiss_on_out(popover::popover_card(&theme).w(px(200.0)), |v: &mut Self| &mut v.menu, cx)
    .child(popover::menu_heading(&theme, "Sort by"))
    .child(popover::menu_row(&theme, false, Some(Fade::new(Painter::of(cx), "name")))
        .id("name")
        .on_click(cx.listener(|view, _, _, cx| view.sort_by_name(cx)))
        .child("Name"));
popover::menu_trigger(div().id("sort").relative().child("Sort"), |v: &mut Self| &mut v.menu, |_| (), cx)
    .when(self.menu.get().is_some(), |trigger| trigger.child(
        popover::anchored_menu_below("sort-menu", card.into_any_element(), self.menu.closing_since())))
```

- The trigger is `relative()`. Mount while `get()` is `Some` and pass `closing_since()`, so the card stays up while `menu-out` plays.
- Close through `popover::close_popup` (which `dismiss_on_out` uses), never `Popup::close`, or the press that shut the menu reopens it.
- `menu_row`'s `active` is the row the cursor is on. `Some(fade)` lights rows under the mouse and its key must stay stable; pass `None` when the menu owns an active index.
- For rows, submenus and keys in one call, use `ui::context_menu` with `Place::Below`.

## Make it yours

| call | what it decides |
| --- | --- |
| `anchored_menu_below`, `_above`, `_below_end`, `_above_end`, `menu_at` | which side of the trigger, or which window point, the card hangs from |
| `menu::card`, `menu::Cursor`, `menu::Hit` | a card painted from `Item`s, with submenus and one cursor for pointer and keys |
| `Item::with_long_description`, `with_tooltip` | a clipped second line, and the hover text that holds all of it |
| `Item::with_keystroke`, `with_shortcut`, `checked`, `disabled` | what a row prints and whether it can be chosen |
| `menu_note`, `menu_section`, `kbd_hint`, `search_line` | the other pieces a picker card is built from |
