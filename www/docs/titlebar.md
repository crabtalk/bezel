---
title: Titlebar
description: The strip a window with no system titlebar moves itself by, the grip that moves it, and the caption buttons off macOS.
prompt: Replace the system titlebar with my own that shows the document title and a share button, and still drags the window.
---

```rust
use ui::titlebar::{self, CaptionSide, DragState};

// self.drag: DragState, one field on the view
titlebar::titlebar("titlebar", true, window)
    .px(px(8.0))
    .child(titlebar::controls(CaptionSide::Left, window, cx))
    .child(title)
    .child(titlebar::grip("titlebar-grip", &self.drag, window))
    .child(actions)
    .child(titlebar::controls(CaptionSide::Right, window, cx))
```

- Open the window with `appears_transparent: true`, and on Linux ask for `WindowDecorations::Client`.
- Only the `grip` moves the window, and it takes the free space the content leaves. A bar with no grip has no handle off macOS.
- Call `controls` at both ends; the desktop decides which one fills. They are empty on macOS and in full screen. The frame around the window is `ui::window::frame`.
- Pass `traffic_lights: true` on the one strip the macOS lights sit over.
