---
title: Document
description: A reader with an outline and a source view — the screen the `markdown` crate exists for, and the round trip you can see.
---

Nothing on this page is library code. The reader is an outline, a scroll area and a segmented toggle; the calls into the library are `markdown::render` and `markdown::serialize`. Typing into a document is the `editor` crate, one page along.

**The outline is a `filter`, not a walk.** A `Doc` is a flat list of blocks carrying their own indent, so the table of contents is one pass picking out headings:

```rust
doc.blocks
    .iter()
    .filter_map(|block| match &block.kind {
        BlockKind::Heading { level, text } => Some((*level, text.text.clone())),
        _ => None,
    })
    .collect()
```

On a nested document tree the same list costs a recursive descent that has to reconstruct depth on the way down. That is the whole argument for the flat model, and it is the same reason the editor's Enter and Backspace are list operations rather than restructures.

**Source view is the round trip.** The Source segment does not show the string the file holds — it shows `serialize(&doc)`, the document written back out, and it matches the original byte for byte. That is what makes an edit/save cycle safe, and it is the one property worth *seeing* rather than reading about in a test.

`parse` and `serialize` are inverses up to a fixed point: parse, serialize, parse again, and the document is unchanged. Byte-identical round tripping is deliberately not promised, because a flat model cannot represent arbitrarily nested CommonMark.

The source is at `apps/gallery/src/patterns/document.rs`. Copy the file.
