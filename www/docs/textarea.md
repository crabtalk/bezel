---
title: Textarea
description: The same text field in a taller box — fixed rows or growing between a min and a max, with enter and pasted newlines behaving accordingly.
---

A textarea is a `TextField` with a different `Shape`. Editing is identical across all three shapes — every action works on the content and a byte range, and none of them cares where the lines break. What the shape decides is the box:

```rust
use bezel_ui::input::{Shape, TextField};

TextField::new(cx).with_shape(Shape::Rows(4))
TextField::new(cx).with_shape(Shape::Grow { min: 3, max: 12 })
```

`Rows(n)` is exactly `n` lines tall and scrolls past that. `Grow { min, max }` is the composer shape: it grows with the content and scrolls once it hits `max`. `Shape::Line` is the single-line default, where a pasted newline becomes a space rather than silently truncating what was pasted.

Multi-line fields claim a second key context, `MULTILINE_KEY_CONTEXT`, and `enter`, `up`, `down` and their shift variants are bound there rather than on every field. A single-line field is routinely nested inside something that has already claimed those keys — the command palette and the combobox both drive their lists with `up`/`down`/`enter`, and their query field sits *deeper* in the focus path, so a binding on every field would win the dispatch and break list navigation in both.

When `enter` needs to mean something else in one particular box, give that box a context of its own:

```rust
const COMPOSER: &str = "Composer";

cx.bind_keys([
    KeyBinding::new("enter", Send, Some(COMPOSER)),
    KeyBinding::new("shift-enter", input::InsertNewline, Some(COMPOSER)),
]);

let field = cx.new(|cx| {
    TextField::new(cx)
        .with_shape(Shape::Grow { min: 3, max: 12 })
        .with_key_context(COMPOSER)
});
```

gpui resolves a keystroke to the binding whose context matches deepest in the focus path, and nothing is deeper than the focused field — so a container around it cannot win `enter`, however it is bound. Rebinding the shared multi-line context would win, and would take the newline away from every other textarea in the app.

`home`/`ctrl-a` goes to the start of the logical line — the byte after the previous newline — not to the start of the visual row a soft wrap put you on. That is emacs' `C-a`, and a deliberate divergence from `NSTextView`.
