---
title: Document
description: A reader with an outline and a source view — the screen the `markdown` crate exists for, and the round trip you can see.
prompt: Show a markdown file as a read-only document with an outline of its headings beside it, and let the reader tick its task boxes.
---

```rust
use markdown::BlockKind;

let doc = markdown::parse(&source);
let outline: Vec<_> = doc.blocks.iter()
    .filter_map(|block| match &block.kind {
        BlockKind::Heading { level, text } => Some((*level, text.text.clone())),
        _ => None,
    })
    .collect();

markdown::render(&doc, markdown::Caption::Shown, window, cx)   // read-only
```

- A `Doc` is a flat list of blocks, each carrying its own indent, so an outline is one pass.
- `render` paints a task's box as a mark. `Editing::toggle` with `render_with` makes it a control and hands you the block index.
- A custom mark opens and closes on one line, and is plain text inside a fence, a code span, a link destination or a picture's caption. A delimiter markdown already spells (`*`, `_`, `` ` ``, `~`, `[`) never fires.
- `http` pictures need an http client on the app; `gpui_platform::application` installs one. Without it they paint their fallback and report nothing.

## Make it yours

| call | what it decides |
| --- | --- |
| `Marks`, `parse_with`, `serialize_with` | marks of your own, such as `==highlight==` |
| `AppExt::set_marks`, `set_mark_paint` | the dialect every reader and editor uses, and how custom marks paint |
| `render_with`, `Editing` | task toggles, the fence copy button, picture clicks, menus and overlays |
| `parse_ranges`, `Synced` | writing back edited blocks and leaving every other byte as it was |
| `parse_at`, `serialize_at` | carrying a caret between the source and the blocks |
| `source_spans`, `render_source` | a coloured source view, with no grammar |
| `AppExt::set_markdown_layout`, `set_typography` | whether fences wrap, and the type metrics |
