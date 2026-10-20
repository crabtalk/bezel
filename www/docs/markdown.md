---
title: Markdown
description: The dialect bezel reads and writes — every block and mark, the prefixes that type them, and what a flat document does with nesting.
prompt: Read todo.md, list its unchecked tasks, and write the file back after I tick one off, without reformatting the rest of it.
---

```rust
use markdown::{AppExt as _, BlockKind, Synced};

let mut file = Synced::new(source, &cx.marks());
let open: Vec<usize> = file.doc().blocks.iter().enumerate()
    .filter(|(_, b)| matches!(b.kind, BlockKind::Task { checked: false, .. }))
    .map(|(ix, _)| ix)
    .collect();

let mut doc = (**file.doc()).clone();
// …tick a task in `doc`…
file.set_doc(doc.into());
save(file.text());   // only the edited block's bytes changed
```

- `parse` and `serialize` reach a fixed point: parse, serialize and parse again lands on the same document. A whole-file `serialize` normalizes; `Synced` and `parse_ranges` keep untouched bytes.
- Nesting is list nesting only, four spaces a level. A quote adds no level, and a quote inside a quote flattens to one.
- Escapes are written only at the start of a line. Raw HTML is read as the text it spells. Footnotes and reference-style definitions are not carried, and a setext heading comes back as `#`.

## Blocks and marks

| spelling | block |
| --- | --- |
| `# ` … `###### ` | heading |
| `- `, `* `, `+ ` / `1. ` | bullet / ordered, keeping its first number |
| `- [ ] `, `- [x] ` | task |
| `> `, `> [!NOTE]` | quote, GFM alert |
| ` ```rs ` | fence, highlighted by the tag's grammar |
| `---` | rule |
| `\| a \| b \|` | GFM table, alignment row included; one line per cell |
| `![caption\|480](url)` alone on a line | picture; the caption is the alt text, `480` a width in pixels |
| `<url>` or a link alone on a line | bookmark card; title `"chip"` or `"embed"` picks the form |

Marks are `**bold**`, `_italic_`, `~~strike~~`, `` `code` `` and `[text](url)`, and their nesting order survives the round trip. A newline inside a block is a line break. An underscore inside a word is not emphasis.

## Make it yours

| call | what it decides |
| --- | --- |
| `Marks`, `AppExt::set_marks` | marks of your own, such as `==highlight==` |
| `AppExt::set_block_renderer`, `BlockRenderer` | painting a fence tag, such as ` ```chart `, as something other than code |
| `AppExt::set_links`, `Links` | opening links, and what a card or chip shows past its URL |
| `Editing::image_overlay`, `image_overlay_corner` | a control on hovered pictures; bottom-right unless set |
| `AppExt::set_highlighter` | fence colours (see [Syntax](/docs/syntax)) |
