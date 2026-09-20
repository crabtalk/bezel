---
title: Markdown
description: The dialect bezel reads and writes — every block and mark, the prefixes that type them, and what a flat document does with nesting.
---

```rust
let doc = markdown::parse(source);
let source = markdown::serialize(&doc);
```

The document model is Notion's — a flat list of blocks, each carrying an indent — and markdown is its wire form. `parse` and `serialize` are inverses up to a fixed point: parse, serialize, parse again, and nothing has moved.

## Blocks

| Spelling | Block |
| --- | --- |
| `# ` … `###### ` | Heading, levels 1–6 |
| plain text | Paragraph |
| `- `, `* `, `+ ` | Bullet |
| `1. ` | Ordered item, keeping the number it starts at |
| `- [ ] `, `- [x] ` | Task |
| `> ` | Quote |
| ` ``` ` | Fence |
| `---` | Rule |
| `\| a \| b \|` | Table |
| `![alt](url)` alone on a line | Picture |
| a link alone on a line | Bookmark |

A newline inside a block is a line break, here and in Notion both.

## Marks

`**bold**`, `_italic_` or `*italic*`, `~~strikethrough~~`, `` `code` ``, and `[text](url)`. Nesting order survives a round trip — `**_x_**` and `_**x**_` are different documents, which is why marks are spans over the text rather than flags on a run.

Typing `## ` makes a heading because pasting `## ` would have; inline marks close on the last delimiter. Emphasis will not open or close against a space, and an underscore inside a word is not emphasis, which is the only reason `snake_case_names` survive being typed.

## Fences

````
```rs
fn main() {}
```
````

A tag names the grammar [`syntax`](/docs/syntax) highlights with. A tag nothing claims paints plain and never fails. `markdown::set_block_renderer` hands a tag and its source to a function of yours, so ` ```chart ` paints as a chart and still holds a caret, still round trips byte for byte, and still degrades to its own source where the renderer is not installed.

## Links and pictures

```markdown
<https://bezel.gallery>
[https://bezel.gallery](https://bezel.gallery "chip")
[https://bezel.gallery](https://bezel.gallery "embed")
![A caption is the alt text|480](https://example.com/cover.png)
```

A link with a line to itself is a card; chip and embed have no shorthand, so they say their name in the title slot. What a card *shows* past its URL is the app's, through `markdown::set_link_preview` — the crate fetches nothing. A picture's caption is its alt text, and a dragged width is written after it in whole pixels.

## Limits

| | |
| --- | --- |
| nesting | Four spaces per level, **list nesting only**. `> - a` flattens to the bullet it reads as; a list inside a quote inside a list does not survive. |
| normalized on parse | Edge whitespace, blank lines at block edges, headings and table cells flattened to one line, ordered runs renumbered. `parse_ranges` gives the source range each block came from, for an app that would rather splice the blocks it did not edit than write the whole file back in canonical form. |
| escaping | Only at the start of a line, where the character would mean something — escape `#` everywhere and `#123` becomes `\#123`, which no reader matches. |
| HTML | Arrives as the text it spells and is written back escaped. |
| not carried | Footnotes and reference-style definitions. A setext heading is read and written back as `#`. |

Tables are GFM's, alignment row included — `:---`, `:---:`, `---:`. Every cell is one line and holds a caret.

The source is at `apps/gallery/src/patterns/dialect.rs`. Copy the file.
