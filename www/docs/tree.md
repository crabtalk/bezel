---
title: Tree view
description: Nested rows with disclosure, indent guides and arrow keys — driven by a depth-annotated flat list, not by a tree the library walks.
prompt: Add a file tree sidebar for the project folder, with folders that open and close by click and by arrow keys.
---

```rust
use ui::tree::{self, Direction, Move};

// self.rows: Vec<Row>, the rows visible now, flattened by the app
tree::tree().children(self.rows.iter().enumerate().map(|(index, row)| {
    tree::tree_row(&theme, row, self.selected == Some(index), self.cursor == index)
        .id(("row", index))
        .child(self.labels[index].clone())
}))

// on tree::Expand:
match tree::step(&self.rows, self.cursor, Direction::Right) {
    Some(Move::To(index)) => self.cursor = index,
    Some(Move::Expand(index) | Move::Collapse(index)) => self.toggle(index),
    None => {}
}
```

- Call `tree::init(cx)` once at startup.
- The app flattens its tree and owns the expansion set; `step` only reports a `Move`.
- Put `.key_context(tree::KEY_CONTEXT)`, focus tracking and handlers for `SelectPrevious`, `SelectNext`, `Collapse` and `Expand` on the element holding the rows.
- Scrolling is the caller's, through [`scroll`](/docs/scroll-area).
