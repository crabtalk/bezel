---
title: Ribbon
description: A formatting bar that is always on, painted from one call — the marks that are lit, the block's name, what cmd-E would do, and whether any of it applies.
prompt: Put a formatting bar above my editor with bold, italic and code buttons that light up for the selection, and a dropdown for the block type.
---

```rust
use markdown::Mark;

let formatting = editor.read(cx).formatting();       // once per frame
let enabled = formatting.mode == editor::Mode::Blocks;
let bold = formatting.marks.contains(&Mark::Bold);
let label = formatting.block.clone();                 // "Heading 2"

editor.update(cx, |editor, cx| editor.toggle_mark(Mark::Bold, cx));   // on press

let block = editor.read(cx).selection().head.block;
for (name, kind) in editor::turns() {                 // the dropdown's rows
    // on click: editor.update(cx, |editor, cx| editor.set_block(block, kind.clone(), cx));
}
```

- `marks` lights a button only when the whole selection carries the mark. At a bare caret it includes a mark pressed before typing.
- `fenceable` is `true` when `Mark::Code` would make a fence rather than an inline span.
- `toggle_mark` does nothing in `Mode::Source`, so grey the bar out there.
- For a mark of your own, register it with `cx.set_marks(markdown::Marks::new().with("highlight", "=="))`, then press it with `toggle_mark(Mark::Custom("highlight".into()))`.
