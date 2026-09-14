---
title: Tree view
description: Nested rows with disclosure, indent guides and arrow keys — driven by a depth-annotated flat list, not by a tree the library walks.
---

```rust
use ui::tree::{self, Row};

tree::init(cx);   // once, at startup

let rows = self.flatten();   // Vec<(Row, label)>

tree::tree().children(rows.iter().enumerate().map(|(index, (row, label))| {
    tree::tree_row(&theme, row, self.selected == Some(index), self.cursor == index)
        .id(("row", index))
        .child(label.clone())
}))
```

The app flattens what is currently visible — which it has to do to render it anyway. A depth-annotated flat list is a complete navigation model: down and up are neighbouring indices, a first child is the next row, and a parent is the nearest row above with a smaller depth.

## Keys

```rust
match tree::step(&rows, self.cursor, tree::Direction::Right) {
    Some(tree::Move::To(index)) => self.cursor = index,
    Some(tree::Move::Expand(index)) => self.open.insert(index),
    Some(tree::Move::Collapse(index)) => self.open.remove(&index),
    None => {}
}
```

## API

| | |
| --- | --- |
| `Row { depth, expanded }` | `expanded: None` is a leaf, which is a different thing from a closed branch — the difference is what stops `right` pretending a file can open. |
| `step(rows, cursor, direction)` | Reports an intent rather than performing one: applying it means touching the expansion set the app owns. Neither end wraps. |
| `tree_row(theme, row, selected, cursor)` | `selected` is what the app considers chosen, `cursor` is where the keyboard is — the same pair a menu row draws with. |

Scrolling is the caller's, through [`scroll`](/docs/scroll-area). Expansion stays with the app because it *is* app data: a file tree's open folders often outlive the window.
