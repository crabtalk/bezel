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

The window moves on the first **motion** after a press, never on the press itself — a bar that moved on mouse-down would swallow every click on the buttons sitting in it.

## Window options

Open the window with `appears_transparent: true` **and** `app_owns_titlebar_drag: true`. The second stops AppKit dragging the window itself, and stops it delaying titlebar clicks while it waits for a possible double-click.

## API

| | |
| --- | --- |
| `titlebar(id, drag, traffic_lights, window)` | A double click runs the system's own titlebar gesture; a no-op off macOS. |
| `DragState` | One field on the view, an `Rc<Cell<bool>>` like `scroll::FollowState` — no listeners to wire. |
| `traffic_lights` | Reserves `Theme::TRAFFIC_LIGHT_INSET` on the leading strip, and stands down in full screen where AppKit takes the lights away. |
