---
title: Command palette
description: A filtered command list over a text field, reporting selections as events so it never learns what your commands are.
---

```rust
use bezel_ui::palette::{self, CommandPalette, PaletteEvent};

palette::init(cx);   // once, at startup, alongside input::init

let palette = cx.new(|cx| CommandPalette::new(COMMANDS.to_vec(), cx));

cx.subscribe(&palette, |_, _, event, _| match event {
    PaletteEvent::Selected(index) => { /* run command `index` */ }
    PaletteEvent::Dismissed => { /* unmount */ }
})
.detach();
```

Stateful for the same reason a text field is: it owns a query, a filtered view and an active row. It reports outcomes as gpui events rather than taking a callback, so the host decides what a selection *means* and the palette never knows about the app's actions.

Indices are into the **original** item list, never into the filtered view. A caller matching on a filtered index would run the wrong command the moment a query is typed.

Navigation is `up`/`down`, `ctrl-p`/`ctrl-n`, `enter` and `escape`, all scoped to the palette's key context. That context wraps the query field's own, so typing goes to the field while the navigation keys fall through — which is why `TextField` does not bind `up`/`down` itself.

Mounting is the caller's: the palette is an entity you render where you want it, usually centred over a scrim. `popover::modal_glass` is the frame for that.

The filtering underneath is `popover::Filter`, shared with the combobox: prefix matches first, then substring matches, stable within each rank.
