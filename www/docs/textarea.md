---
title: Textarea
description: The same text field in a taller box — fixed rows or growing between a min and a max, with enter and pasted newlines behaving accordingly.
---

```rust
use ui::input::{Shape, TextField};

TextField::new(cx).with_shape(Shape::Rows(4))
TextField::new(cx).with_shape(Shape::Grow { min: 3, max: 12 })
```

Editing is identical across all three shapes — every action works on the content and a byte range. What the shape decides is the box.

## Making enter mean something else

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

Give that one box a context of its own. Rebinding the shared multi-line context would take the newline away from every other textarea in the app.

## API

```rust
impl TextField {
    pub fn with_shape(self, shape: Shape) -> Self;

    /// Give one box a context of its own; rebinding the shared multi-line one
    /// would take the newline away from every other textarea in the app.
    pub fn with_key_context(self, context: impl Into<SharedString>) -> Self;

    // ...
}

pub enum Shape {
    /// The default. A pasted newline becomes a space rather than truncating
    /// the paste.
    Line,
    /// Exactly `n` lines, scrolling past that.
    Rows(usize),
    /// The composer shape: grows with the content, scrolls at `max`.
    Grow { min: usize, max: usize },
}
```

`MULTILINE_KEY_CONTEXT` is where `enter`, `up` and `down` are bound — not on every field, since a single-line field nested in a palette or combobox would win those keys and break list navigation.

`home`/`ctrl-a` goes to the start of the logical line, not of the visual row a soft wrap put you on — emacs' `C-a`, and a deliberate divergence from `NSTextView`.
