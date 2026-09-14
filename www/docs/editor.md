---
title: Editor
description: A Notion-style block editor over the markdown document model — one entity, a scroll handle and an observe, and the markdown a save would write on every keystroke.
---

`markdown` holds the document and paints it; `editor::Editor` is the surface you type on — focus, keys, the mouse, undo and the menus.

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

Pass the scroll handle of the pane the document sits in, not one of the editor's own, or the caret cannot follow typing down the page.

Typing notifies the editor, so anything a host reads off it needs that `observe`. Without it the markdown pane on this page would freeze on the opening text and the toolbar would never appear at all.

`editor.source()` is the document written back to markdown, normalized, on every keystroke — what a save would write, and what fills the right pane here.

A toolbar is three calls: `selection_bounds()` for where to float it, `toggle_mark` for what a button does, and `covered_by` for whether it is lit. That is the same entry point cmd-B takes, so a button and a chord cannot disagree.

```rust
let lit = editor.doc().covered_by(editor.selection(), &mark);

editor.update(cx, |editor, cx| editor.toggle_mark(mark, cx));
```

`Mark::Code` over a selection spanning more than one line makes a fence instead of an inline span, and the same call takes it back out.

## Markdown in the same view

`set_mode(Mode::Source, cx)` swaps the document for the markdown a save would write, in the same editor, with the same focus and the same undo history. `toggle_source` flips between the two and `mode()` reads which one is showing — the trigger is yours to place, name and bind:

```rust
editor.update(cx, |editor, cx| editor.toggle_source(cx));
```

The caret crosses with it. Going in, the offset is exact through markers, escapes and marks, because it is the serializer that places it; coming back it lands in the block and the word it was in. A caret between a heading's `#` and its space has no block to come back to, and the document opens at the start instead.

The source is one editable text: Enter is a newline, tab is two spaces, and the block chrome — the gutter handle, the slash menu, the block and language menus, drag-to-reorder, a dropped picture — stays out of it, because every one of those would edit the markup rather than the document it spells. `source()` answers with the markdown in either mode, so a save needs no branch. `doc()` in source mode is the one fence the text is held in, not the document it spells.

Undo crosses the switch and carries the mode with it: step back over a toggle and the document comes back in the form it was edited in. A comment anchor does not follow an edit made to the source — there are no blocks there to anchor to — and is clamped back onto the document on the way out.

The source view is coloured by `markdown` itself rather than by `syntax`: markdown is the one language this crate already parses, and a browser build can run it, which tree-sitter cannot. An installed highlighter wins where it answers for a `md` fence, so an app with a grammar of its own keeps it.

`EditorEvent::ModeChanged` is how a host's own toggle hears about a switch it did not make — an undo across one, for instance.

The slash menu, the gutter handle, drag-to-reorder, the language picker on a fence, the menu that turns a pasted URL into a chip, a bookmark, an embed or a picture, undo and the clipboard need no wiring — the source behind this page contains not one line for any of them. Undo keeps 100 steps, coalesced so a run of typing comes back as a word rather than a character; `with_undo_limit` for more.

An image arrives four ways: a pasted URL that names one, `/image` and the row it leaves asking for a URL, a file dragged in from the desktop, and a screenshot off the clipboard. Only the last needs wiring, because bytes have no address and a document holds one:

```rust
editor::set_image_store(cx, |source| match source {
    editor::Source::File(path) => Some(path.to_string_lossy().into_owned()),
    editor::Source::Bytes(image) => save_somewhere(image),  // your assets, your URL
});
```

A dropped file is offered to the store first so an app that keeps its own asset directory can copy it in; answer `None` and the picture paints from where it already is. With no store installed a screenshot cannot be pasted at all. The caption under a picture is its alt text, and a caret sits in it like any other line.

`init` binds `ui::input::TextField`'s chords inside the editor's own key context, so `tab` indents a list here and means nothing outside one. Replace that call for a different keymap. Moving, duplicating and deleting a block ship as actions with no chord for an app to bind as it likes; the block menu on the gutter handle reaches them meanwhile.

`editor` is a peer crate you name yourself, alongside `markdown` and `syntax`.

The source is at `apps/gallery/src/patterns/editor.rs`. Copy the file.
