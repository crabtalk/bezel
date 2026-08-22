---
title: Titlebar
description: The strip a window with no system titlebar moves itself by, and the room the macOS traffic lights need.
---

```rust
use ui::titlebar;

titlebar::titlebar("titlebar", &self.drag, true, window)
    .px(px(8.0))
    .child(title)
    .child(actions)
```

`DragState` is one field on the view — an `Rc<Cell<bool>>` like `scroll::FollowState`, so the element carries the gesture and you wire no listeners.

The window moves on the first **motion** after a press, never on the press itself. A bar that moved on mouse-down would swallow every click on the buttons sitting in it, and that is the bug the strip exists to not have: the browser version of this in `../desktop` needs a selector listing every interactive descendant to work around it.

`traffic_lights` reserves the leading inset for the macOS buttons. Pass it on the one strip they sit over — the leftmost — and it stands down in full screen, where AppKit takes the lights away and the gap would be a hole. The number is `Theme::TRAFFIC_LIGHT_INSET`, and it clears the lights where AppKit puts them: an app that moves them with `TitlebarOptions::traffic_light_position` owns the inset too.

Open the window with `appears_transparent: true` **and** `app_owns_titlebar_drag: true`. The second one stops AppKit from dragging the window itself and from delaying titlebar clicks while it waits to see whether a double-click is coming.

A double click runs the system's own titlebar gesture — zoom, minimise, or nothing, whichever the user has set. It is a no-op off macOS.
