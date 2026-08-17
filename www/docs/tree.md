---
title: Tree view
description: Nested rows with disclosure, indent guides and arrow keys — driven by a depth-annotated flat list, not by a tree the library walks.
---

bezel cannot walk your tree. It has no idea what a node is, and a trait or a callback to find out would be a data model this library does not want to own. So the app flattens what is currently visible — which it has to do to render it anyway:

```rust
use bezel_ui::tree::{self, Row};

tree::init(cx);   // once, at startup

let rows = self.flatten();   // Vec<(Row, label)>

tree::tree().children(rows.iter().enumerate().map(|(index, (row, label))| {
    tree::tree_row(&theme, row, self.selected == Some(index), self.cursor == index)
        .id(("row", index))
        .child(label.clone())
}))
```

A depth-annotated flat list is a complete navigation model. Everything a tree does falls out of `Row { depth, expanded }` with no parent pointers and no traversal: down and up are neighbouring indices, a first child is simply the next row, and a parent is the nearest row above with a smaller depth.

`expanded` is `None` for a leaf, which is a different thing from a closed branch — the difference is what stops `right` pretending a file can open.

Keys report an intent rather than performing one, because applying it means touching the expansion set the app owns:

```rust
match tree::step(&rows, self.cursor, tree::Direction::Right) {
    Some(tree::Move::To(index)) => self.cursor = index,
    Some(tree::Move::Expand(index)) => self.open.insert(index),
    Some(tree::Move::Collapse(index)) => self.open.remove(&index),
    None => {}
}
```

Neither end wraps. A menu wraps because it is a ring of choices; a tree is a document, and arriving back at the top because you pressed down once too often loses your place in it.

`tree_row` takes two flags: `selected` is what the app considers chosen, `cursor` is where the keyboard is. They are the same pair `popover::menu_row_nav` uses, so a tree and a menu never look like two different products.

Scrolling is the caller's, through `scroll`. Expansion stays with the app because it *is* app data — a file tree's open folders often outlive the window.
