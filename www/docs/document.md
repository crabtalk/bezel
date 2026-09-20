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

```rust
// markdown — pure, so the registry is a parameter rather than a global.

pub fn parse_with(source: &str, marks: &Marks) -> Doc;
pub fn serialize_with(doc: &Doc, marks: &Marks) -> String;

/// The pair a caret crosses on. Both put a sentinel where the caret is, so
/// neither can drift from the serializer or parser it rides on.
pub fn parse_at(source: &str, offset: usize, marks: &Marks) -> (Doc, Cursor);

/// `parse`, keeping where each block came from. The ranges partition the
/// source — first starts at 0, each ends where the next begins, last ends at
/// `source.len()` — so re-serializing the blocks you edited and splicing
/// `source[range]` for the rest gives the untouched bytes back exactly.
pub fn parse_ranges(source: &str) -> ParsedDoc;  // { doc, block_ranges }
pub fn serialize_at(doc: &Doc, at: Cursor, marks: &Marks) -> (String, usize);

// The gpui-side half the editing surface reads. `set_mark_paint` is paint only
// — colour, background, weight, italic, underline, strike. A name nothing
// paints round trips and reads as the text it wraps.
pub fn set_marks(cx: &mut App, marks: Marks);
pub fn set_mark_paint(cx: &mut App, paint: Painter);

/// Classifies markdown *source* without a grammar, which is what gives a
/// source view colour in a browser build.
pub fn source_spans(source: &str) -> Vec<(Range<usize>, HighlightKind)>;

/// A long fence line wraps by default, since the caret reads a fence in the
/// editor and a sideways scroller can hold it off the right edge.
pub fn set_layout(cx: &mut App, layout: Layout);

/// Makes a task block's checkbox a control in a document you render yourself:
/// the box takes the press, stops it, and hands you the block it belongs to.
/// `render` and `markdown` leave it unset, and the box paints as a marker.
pub struct Editing<'a> { pub on_toggle: Option<OnToggle>, /* ... */ }

// ...
```

A registered mark cannot reach across a line break, mean anything inside a fence, code span or link destination, or sit in a picture's caption. Its delimiters are lifted out before CommonMark sees the source, which is the only place `==` and `\=\=` still differ.

A registered mark cannot reach across a line break, mean anything inside a fence, code span or link destination, or sit in a picture's caption. Its delimiters are lifted out before CommonMark sees the source, which is the only place `==` and `\=\=` still differ.

The source is at `apps/gallery/src/patterns/document.rs`. Copy the file.
