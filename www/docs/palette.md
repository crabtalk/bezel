---
title: Command palette
description: A filtered command list over a text field, reporting selections as events so it never learns what your commands are.
prompt: Add a cmd-K command palette over my app's commands that runs the chosen one and closes on escape.
---

```rust
use ui::palette::{self, CommandPalette, PaletteEvent};

palette::init(cx);   // once, at startup, alongside input::init

let palette = cx.new(|cx| CommandPalette::new(COMMANDS.to_vec(), cx));
cx.subscribe(&palette, |view, _, event, cx| match event {
    PaletteEvent::Selected(index) => view.run(COMMANDS[*index], cx),
    PaletteEvent::Dismissed => view.close_palette(cx),
})
.detach();
```

- `Selected` carries an index into the list given to `new`, never into the filtered view.
- The palette paints its card only. Mount it yourself, usually in `popover::modal_glass`, and call `focus` once it is mounted.
- Keys: `up`/`down`, `ctrl-p`/`ctrl-n`, `enter`, `escape`.
