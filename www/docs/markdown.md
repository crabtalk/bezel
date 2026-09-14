---
title: Markdown
description: The dialect bezel reads and writes — every block and mark, the prefixes that type them, and what a flat document does with nesting.
---

The document model is Notion's — a flat list of blocks, each carrying an indent — and markdown is its wire form. `markdown::parse` reads a string into a `Doc`, `markdown::serialize` writes it back, and the two are inverses up to a fixed point: parse, serialize, parse again, and nothing has moved. That property, not byte-identical round tripping, is what an editor needs, and it is what everything below is in service of.

Each row of the page beside this one is a spelling on the left and the block it makes on the right, rendered by the library itself.

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

A newline inside a block is a line break, here and in Notion both. Markdown's soft and hard breaks are the same thing to this model, and which one it paints as is the renderer's decision.

## Marks

`**bold**`, `_italic_` or `*italic*`, `~~strikethrough~~`, `` `code` ``, and `[text](url)`. Marks nest, and the nesting order survives a round trip — `**_x_**` and `_**x**_` are different documents, which is why marks are a list of spans over the text rather than flags on a run.

Two inline shapes have no CommonMark name of their own. `![alt](url)` *among* text stays an inline image rather than becoming a picture block, so a sentence with a picture in it does not quietly turn into a link on save. A link painted as a chip is the same `[text](url)` with a title — see below.

## Typing is the same vocabulary

Typing `## ` makes a heading because pasting `## ` would have. The prefixes are matched at the start of a block as you finish them: the heading levels, `- `/`* `/`+ `, `1. `, `- [ ] ` and `- [x] `, `> `, ` ``` ` and `---`.

The inline half works the same way, on the closing delimiter: typing the last `*` of `**bold**` collapses the four asterisks into a mark. Emphasis will not open or close against a space, and an underscore inside a word is not emphasis at all — which is the only reason `snake_case_names` survive being typed.

## Fences

A fence tag names the grammar [`syntax`](/docs/syntax) highlights the block with — `rs`, `py`, `ts`, `json`, `go`, `sh`, `toml` and their aliases. A tag nothing claims paints plain, and never fails.

````
```rs
fn main() {}
```
````

A fence is also where an app's own block lives. `markdown::set_block_renderer` hands a tag and its source to a function of yours, so ` ```chart ` paints as a chart and still holds a caret, still round trips byte for byte, and still degrades to its own source anywhere the renderer is not installed. [`blocks`](/docs/document) ships one; an app with its own writes the same function.

A fence tagged `md` is coloured by `markdown` itself, with no grammar and no tree-sitter — which is what gives the [editor](/docs/editor)'s source mode colour in a browser build.

## Links and pictures

A link inside a sentence is an underlined span. A link with a line to itself is a card, and the angle-bracket spelling is what says so:

```markdown
<https://bezel.gallery>
```

The other two shapes have no shorthand, so they say their name in the title slot — the one place markdown leaves for it:

```markdown
[https://bezel.gallery](https://bezel.gallery "chip")
[https://bezel.gallery](https://bezel.gallery "embed")
```

What a card *shows* past its URL — title, description, cover, favicon — is the app's, through `markdown::set_link_preview`. The crate fetches nothing.

A picture's caption is its alt text, because markdown has one slot and a reader who cannot see the picture reads the same words. A width dragged out in the editor is written after the alt text, in whole pixels:

```markdown
![A caption is the alt text|480](https://example.com/cover.png)
```

## Tables and nesting

GFM's table, alignment row included — `:---`, `:---:`, `---:`. Every cell is one line, and a caret sits in each of them.

Nesting is **four spaces per level, and list nesting only**. A document is a flat list of blocks carrying a depth, so `> - a` flattens to the bullet it reads as and loses the quote, and a list inside a quote inside a list does not survive. Notion has the same limitation, and the fixed point is what keeps it from mattering: whatever the first parse decides is stable from then on.

## What the parse settles

Reading a document normalizes what markdown itself would not preserve, so that writing it back and reading it again lands in the same place: leading and trailing whitespace per line, blank lines at a block's edges, headings and table cells flattened to one line, and each run of ordered items renumbered consecutively.

Escaping on the way out is deliberately narrow — `#`, `>`, `-` and friends are escaped only at the start of a line, where they would actually mean something. Over-escaping is its own bug: escape `#` everywhere and a `#123` reference becomes `\#123`, which no reader matches.

## What is not in the dialect

HTML is not markup here: a tag arrives as the text it spells and is written back escaped, so it stays text. Footnotes and reference-style definitions are not carried — a reference link comes back inline and its definition line is gone. A setext heading is read and written back as `#`. Nothing else in GFM is silently dropped; if it parses, it round trips.

The source is at `apps/gallery/src/patterns/dialect.rs`. Copy the file.
