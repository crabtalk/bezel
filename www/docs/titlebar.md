---
title: Titlebar
description: The strip a window with no system titlebar moves itself by, the grip that moves it, and the caption buttons off macOS.
---

```rust
use ui::titlebar::{self, CaptionSide};

titlebar::titlebar("titlebar", true, window)
    .px(px(8.0))
    .child(title)
    .child(titlebar::grip("titlebar-grip", &self.drag, window))
    .child(actions)
    .child(titlebar::controls(CaptionSide::Right, window, cx))
```

The strip is inert. A `grip` in it moves the window, takes the free space the content leaves, and is the only part that drags — which is what leaves a button beside it its own click. The window moves on the first **motion** after a press, never on the press itself.

Nothing in the bar has to opt out of the grip. Windows answers `WM_NCHITTEST` from a flat list of control areas and takes the first one the pointer falls in, parent before child, so a bar that is itself one drag area turns every control in it into a window handle unless each control blocks the mouse — and blocking the mouse takes the scroll wheel from everything behind it too. Naming the drag surface is what AppKit does with a drag gesture on a view, and it costs the controls nothing.

Three platforms, three mechanisms, all on the grip: AppKit drags by itself, Linux is told with `start_window_move`, and Windows implements neither and reads `WindowControlArea::Drag` back out of the hit test.

## Window options

Open the window with `appears_transparent: true` **and** `app_owns_titlebar_drag: true`. The second stops AppKit dragging the window itself, and stops it delaying titlebar clicks while it waits for a possible double-click.

Off macOS the window has no caption of its own, so the bar owes the buttons: `controls` at both ends, and the desktop decides which end fills. The frame around them — border, corners, shadow and resize edges — is `ui::window::frame`, which paints only where the compositor hands the window over undecorated.

## API

```rust
// ui::titlebar

/// The strip. `traffic_lights` reserves `Theme::TRAFFIC_LIGHT_INSET` on the
/// leading edge, and stands down in full screen where AppKit takes the lights
/// away.
pub fn titlebar(
    id: impl Into<ElementId>,
    traffic_lights: bool,
    window: &Window,
) -> Stateful<Div>;

/// The bare stretch that drags the window, zooms it on a double click and
/// opens the desktop's window menu on a right press. Takes the free space.
pub fn grip(id: impl Into<ElementId>, drag: &DragState, window: &Window) -> Stateful<Div>;

/// Minimize, maximize and close, on the side the desktop puts them. Empty on
/// macOS, in full screen, and of whatever the compositor refuses.
pub fn controls(side: CaptionSide, window: &Window, cx: &App) -> Div;

/// One field on the view, an `Rc<Cell<bool>>` like `scroll::FollowState` — no
/// listeners to wire.
pub struct DragState(Rc<Cell<bool>>);

// ...
```
