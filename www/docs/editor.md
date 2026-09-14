---
title: Editor
description: A Notion-style block editor over the markdown document model — one entity, a scroll handle and an observe, and the markdown a save would write on every keystroke.
---

```rust
use editor::Editor;

editor::init(cx);   // once, at startup

let scroll = ScrollHandle::new();
let editor = cx.new({
    let scroll = scroll.clone();
    |cx| Editor::new("# Notes", cx).with_scroll(scroll)
});
cx.observe(&editor, |_, _, cx| cx.notify()).detach();
```

Pass the scroll handle of the pane the document sits in, not one of the editor's own, or the caret cannot follow typing down the page. Typing notifies the editor, so anything a host reads off it needs that `observe`.

## A toolbar

```rust
let formatting = editor.read(cx).formatting();

let lit = formatting.marks.contains(&mark);          // and cmd-B before typing counts
let label = formatting.block.clone();                // "Heading 2", for a dropdown
let fence = formatting.fenceable;                    // cmd-E would make a fence, not a span
let bar = formatting.mode == editor::Mode::Blocks;   // the source has nothing to light

editor.update(cx, |editor, cx| editor.toggle_mark(mark, cx));
```

One read rather than four: a bar answering half its questions from this frame and half from the last lights the wrong button for a frame.

## Block menus

```rust
for (label, kind) in editor::turns() {
    let lit = formatting.block.as_ref() == Some(&label);
    // …and on click: editor.update(cx, |editor, cx| editor.set_block(ix, kind.clone(), cx));
}
```

## Markdown in the same view

```rust
editor.update(cx, |editor, cx| editor.toggle_source(cx));
```

Same editor, same focus, same undo history. The caret crosses with it — exact going in, since the serializer places it; coming back it lands in the block and word it was in.

## Pasted images

```rust
editor::set_image_store(cx, |source| match source {
    editor::Source::File(path) => Some(path.to_string_lossy().into_owned()),
    editor::Source::Bytes(image) => save_somewhere(image),  // your assets, your URL
});
```

Only a screenshot needs this — bytes have no address and a document holds one. With no store installed, a screenshot cannot be pasted at all.

## API

| | |
| --- | --- |
| `source()` | The document written back to markdown, normalized, on every keystroke — in either mode, so a save needs no branch. |
| `formatting()` | `marks` is what the selection carries throughout; at a collapsed caret it is what the next character would carry, so a button stays lit between cmd-B and the letter. |
| `toggle_mark(mark, cx)` | The same entry point cmd-B takes, so a button and a chord cannot disagree. |
| `editor::turns()` | The block vocabulary the slash and block menus offer, to pair with `set_block`. |
| `set_mode` / `toggle_source` / `mode()` | The trigger is yours to place, name and bind. `EditorEvent::ModeChanged` hears about a switch you did not make. |
| `with_chrome(Chrome { .. })` | Turns the library's own affordances off where an app puts its own in the same place. |
| `with_marks(..)` | One editor's own dialect, rather than the one `markdown::set_marks` installed. |
| `with_undo_limit(n)` | Default 100, coalesced so a run of typing comes back as a word. Undo crosses a mode switch and carries the mode with it. |
| `layouts()` | `block_bounds`, `picture_bounds`, `language_bounds`, `hit`, and `rects(selection)` for the painted rows of a range. |
| `editor::keys` | Moving, duplicating and deleting a block ship as actions with no chord. |

The slash menu, gutter handle, drag-to-reorder, language picker, link menu, undo and the clipboard need no wiring. `Mark::Code` over more than one line makes a fence instead of an inline span, and the same call takes it back out.

The source is at `apps/gallery/src/patterns/editor.rs`. Copy the file.
