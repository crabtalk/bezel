---
title: Document
description: A reader with an outline and a source view — the screen the `markdown` crate exists for, and the round trip you can see.
---

```rust
doc.blocks
    .iter()
    .filter_map(|block| match &block.kind {
        BlockKind::Heading { level, text } => Some((*level, text.text.clone())),
        _ => None,
    })
    .collect()
```

The outline is a `filter`, not a walk: a `Doc` is a flat list of blocks carrying their own indent, so the table of contents is one pass. The Source segment shows `serialize(&doc)` rather than the string the file holds, and it matches byte for byte.

## Marks of your own

```rust
let marks = markdown::Marks::new().with("highlight", "==").with("underline", "++");

let doc = markdown::parse_with("a ==lit== word", &marks);
assert_eq!(markdown::serialize_with(&doc, &marks), "a ==lit== word");
```

`Mark` is closed, because each of its variants is something CommonMark already spells. Underline, highlight and colour are not, so an app registers them.

## API

| | |
| --- | --- |
| `parse_with` / `serialize_with` | Pure, so the registry is a parameter rather than a global. |
| `set_marks` / `set_mark_paint` | The gpui-side half the editing surface reads. Paint only — colour, background, weight, italic, underline, strike. A name nothing paints round trips and reads as the text it wraps. |
| `source_spans(..)` | Classifies markdown *source* without a grammar, which is what gives a source view colour in a browser build. |
| `serialize_at` / `parse_at` | The pair a caret crosses on. Both put a sentinel where the caret is, so neither can drift from the serializer or parser it rides on. |
| `set_layout(cx, Layout { wrap_code: false })` | A long fence line wraps by default, since the caret reads a fence in the editor and a sideways scroller can hold it off the right edge. |

A registered mark cannot reach across a line break, mean anything inside a fence, code span or link destination, or sit in a picture's caption. Its delimiters are lifted out before CommonMark sees the source, which is the only place `==` and `\=\=` still differ.

The source is at `apps/gallery/src/patterns/document.rs`. Copy the file.
