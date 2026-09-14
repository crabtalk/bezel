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

## Marks of your own

`Mark` is closed — bold, italic, strike, code, links — because each of those is something CommonMark already spells. Underline, a highlight and a colour are not, so an app registers them instead of waiting for a release:

```rust
let marks = markdown::Marks::new().with("highlight", "==").with("underline", "++");

let doc = markdown::parse_with("a ==lit== word", &marks);
assert_eq!(markdown::serialize_with(&doc, &marks), "a ==lit== word");
```

The registry is a parameter rather than a global because `parse_with` and `serialize_with` are pure. `markdown::set_marks` is the gpui-side half, which is what the editing surface reads; `markdown::set_mark_paint` says what each name looks like — colour, background, weight, italic, underline, strikethrough, all of it paint and none of it layout. A name nothing paints round trips and reads as the text it wraps.

Registered delimiters are lifted out of the source **before** CommonMark sees it, which is the only place `==` and `\=\=` still differ: a backslash escape is gone by the time there is a `Text` to scan, and a pass over one would read an escaped delimiter back as a mark and move the document on every save. The escape survives, and so does the fixed point — the generated sweep that pins it runs with a registry on.

Three things a registered mark deliberately cannot do: reach across a line break (the next line may belong to another block), mean anything inside a fence, a code span or a link's destination, or sit in a picture's caption, which markdown writes as a plain string.

**Markdown colours itself.** `markdown::source_spans` classifies markdown *source* — headings, emphasis, fences, link destinations — without a grammar and without tree-sitter, which is what lets a source view have colour in a browser build. A fence tagged `md` takes it automatically wherever the installed highlighter has no answer, so the editor's source mode is coloured with nothing installed at all.

`serialize_at` and `parse_at` are the pair a caret crosses on: the document and a cursor in, the markdown and a byte offset out, and back again. Both work by putting a sentinel where the caret is and reading off where it came out, so neither can drift from the serializer or the parser it rides on.

**A long line in a fence wraps.** Wrapping is the default because the caret is what reads a fence in the editor, and a sideways scroller can hold it off the right edge with nothing on the page to bring it back. An app that would rather have the scroller says so once at boot, the same way it installs its typography:

```rust
markdown::set_layout(cx, markdown::Layout { wrap_code: false });
```

Either way a source line is one text layout and the caret resolves through it, so wrapping costs nothing at the hit test — a wrapped line is rows of one layout, exactly as a paragraph already is.

The source is at `apps/gallery/src/patterns/document.rs`. Copy the file.
