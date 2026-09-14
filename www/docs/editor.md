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

```rust
impl Editor {
    pub fn new(source: &str, cx: &mut Context<Self>) -> Self;

    /// The scroll handle of the pane the document sits in, not one of the
    /// editor's own, or the caret cannot follow typing down the page.
    pub fn with_scroll(self, handle: gpui::ScrollHandle) -> Self;

    /// The document written back to markdown, normalized, on every keystroke —
    /// in either mode, so a save needs no branch.
    pub fn source(&self) -> String;

    /// One read rather than four: a bar answering half its questions from this
    /// frame and half from the last lights the wrong button for a frame.
    pub fn formatting(&self) -> Formatting;

    /// The same entry point cmd-B takes, so a button and a chord cannot
    /// disagree.
    pub fn toggle_mark(&mut self, mark: Mark, cx: &mut Context<Self>);

    pub fn set_block(&mut self, ix: usize, kind: BlockKind, cx: &mut Context<Self>);

    // The trigger is yours to place, name and bind. `EditorEvent::ModeChanged`
    // hears about a switch you did not make.
    pub fn mode(&self) -> Mode;
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>);
    pub fn toggle_source(&mut self, cx: &mut Context<Self>);

    /// Turns the library's own affordances off where an app puts its own in
    /// the same place.
    pub fn with_chrome(self, chrome: Chrome) -> Self;

    /// One editor's own dialect, rather than the one `markdown::set_marks`
    /// installed.
    pub fn with_marks(self, marks: markdown::Marks) -> Self;

    /// Default 100, coalesced so a run of typing comes back as a word. Undo
    /// crosses a mode switch and carries the mode with it.
    pub fn with_undo_limit(self, limit: usize) -> Self;

    /// Where everything landed last frame — `block_bounds`, `picture_bounds`,
    /// `language_bounds`, `hit`, and `rects(selection)` for the painted rows
    /// of a range.
    pub fn layouts(&self) -> &BlockLayouts;

    // ...
}

// editor

/// The block vocabulary the slash and block menus offer, to pair with
/// `set_block`.
pub fn turns() -> Vec<(SharedString, BlockKind)>;

pub fn set_image_store(cx: &mut App, store: ImageStore);
```

Moving, duplicating and deleting a block ship as actions with no chord — `editor::keys` is the whole set. The slash menu, gutter handle, drag-to-reorder, language picker, link menu, undo and the clipboard need no wiring. `Mark::Code` over more than one line makes a fence instead of an inline span, and the same call takes it back out.

The slash menu, gutter handle, drag-to-reorder, language picker, link menu, undo and the clipboard need no wiring. `Mark::Code` over more than one line makes a fence instead of an inline span, and the same call takes it back out.

The source is at `apps/gallery/src/patterns/editor.rs`. Copy the file.
