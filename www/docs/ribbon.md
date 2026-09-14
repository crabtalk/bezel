---
title: Ribbon
description: A formatting bar that is always on, painted from one call — the marks that are lit, the block's name, what cmd-E would do, and whether any of it applies.
---

```rust
let formatting = editor.read(cx).formatting();

let lit = formatting.marks.contains(&mark);          // and cmd-B before typing counts
let label = formatting.block.clone();                // "Heading 2", for the dropdown
let fence = formatting.fenceable;                    // cmd-E would fence, not span
let live = formatting.mode == editor::Mode::Blocks;  // the source has nothing to light
```

## The dropdown

```rust
for (label, kind) in editor::turns() {
    let lit = formatting.block.as_ref() == Some(&label);
    // on click: editor.update(cx, |editor, cx| editor.set_block(block, kind.clone(), cx));
}
```

Matched on kind alone, so a numbered list at 7 and a fence tagged `rs` still report their row.

## Marks the library does not have

```rust
markdown::set_marks(cx, markdown::Marks::new().with("highlight", "==").with("underline", "++"));
markdown::set_mark_paint(cx, paint);
```

`toggle_mark(Mark::Custom("highlight".into()))` is the same call bold takes, and the editor never learns what the name means.

## What the snapshot answers

| | |
| --- | --- |
| `marks` | What a button *means*, not what the text looks like. Over a mixed selection it is unlit, because pressing it will bold the rest. |
| `block` | The caret's block, matched against `editor::turns()` so your menu cannot drift from the two bezel opens. |
| `fenceable` | The one thing a bar cannot work out itself: `Mark::Code` over more than one line makes a fence, and over a table neither. |
| `mode` | `toggle_mark` refuses in source mode, so the bar greys out rather than promising something the editor will not do. |

The bar is **docked**, so it reflows the document under it — which is why it is a plain row with a hairline rather than [`control_bar`](/docs/control-bar), the floating kind that must never move what it sits over. The other answer to the same question is the bubble toolbar on the [Editor](/docs/editor) page; an app picks one.

The source is at `apps/gallery/src/patterns/ribbon.rs`. Copy the file.
