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
use editor::AppExt as _;
cx.set_image_store(editor::ImageStore {
    keep: |source, _editor, _base, _cx| match source {
        editor::Source::File(path) => Some(path.to_string_lossy().into_owned()),
        editor::Source::Bytes(image) => save_somewhere(image),  // your assets, your URL
    },
    ..Default::default()
});
```

Only a screenshot needs this — bytes have no address and a document holds one. With no store installed, a screenshot cannot be pasted at all.

## Table controls

Hover a cell to reveal six-dot handles in the table's left and top lanes.
Click one to insert, delete, or move its row or column one step. Drag a handle
to reorder within the table; a line marks the drop position. Escape or dropping
outside the table cancels. Right-click a cell to open
both sets of actions, including when the table header is scrolled out of view.
The bottom and right `+` strips append rows and columns. The header row and
last column cannot be deleted; table edits support undo.

## Picture controls

Article editors can use the same hover controls as markdown previews:

```rust
Editor::new(source, cx)
    .with_image_overlay(Rc::new(|block, url, window, cx| {
        Some(open_image_button(block, url, window, cx).into_any_element())
    }))
    .with_image_overlay_corner(markdown::ImageOverlayCorner::TopRight)
```

Bottom-right is the default. `set_image_overlay` replaces the callback or removes
it with `None`; `set_image_overlay_corner` changes its position. Both take the
editor's context. Controls appear only in rich mode, and their presses do not
move the caret or start a selection.

## Paste policy

```rust
use editor::{AppExt as _, PasteContent};

cx.set_paste_handler(|item, _editor, _destination, _cx| {
    item.text().map(PasteContent::Literal)
});
```

The handler runs before default clipboard handling. Return `Literal` to insert
plain text, `Markdown` to use normal text-paste rules, or `None` to fall back.
Markdown uses the editor's marks and URL handling; it stays literal inside a
fence or in source mode. Insertion uses the normal selection and undo history.

The destination supplies `mode`, `in_fence`, and `base`. The editor entity is
already being updated: use it only as an identity, without reading or updating
it. To retain media, call `(cx.image_store().keep)(source, editor,
destination.base, cx)` and return the text your app wants inserted. No handler
means unchanged paste behavior. File drops use their existing separate path.

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

    /// Check or uncheck a task block. A press on the box already calls this;
    /// the caret does not move, and one toggle is one undo step.
    pub fn toggle_task(&mut self, ix: usize, cx: &mut Context<Self>);

    // The trigger is yours to place, name and bind. `EditorEvent::ModeChanged`
    // hears about a switch you did not make.
    pub fn mode(&self) -> Mode;
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>);
    pub fn toggle_source(&mut self, cx: &mut Context<Self>);

    /// Turns the library's own affordances off where an app puts its own in
    /// the same place.
    pub fn with_chrome(self, chrome: Chrome) -> Self;

    /// One editor's own dialect, rather than the one `markdown::AppExt::set_marks`
    /// installed.
    pub fn with_marks(self, marks: markdown::Marks) -> Self;

    /// Default 100, coalesced so a run of typing comes back as a word. Undo
    /// crosses a mode switch and carries the mode with it.
    pub fn with_undo_limit(self, limit: usize) -> Self;

    /// Where everything landed last frame — `block_bounds`, `picture_bounds`,
    /// `language_bounds`, `checkbox_bounds`, `hit`, and `rects(selection)` for
    /// the painted rows of a range.
    pub fn layouts(&self) -> &BlockLayouts;

    // ...
}

// editor

/// The block vocabulary the slash and block menus offer, to pair with
/// `set_block`.
pub fn turns() -> Vec<(SharedString, BlockKind)>;

pub trait AppExt {
    fn set_image_store(&mut self, store: ImageStore);
    fn image_store(&self) -> ImageStore;
    fn set_paste_handler(&mut self, handler: PasteHandler);
}
```

Moving, duplicating and deleting a block ship as actions with no chord — `editor::keys` is the whole set. The slash menu, gutter handle, drag-to-reorder, language picker, link menu, undo and the clipboard need no wiring. `Mark::Code` over more than one line makes a fence instead of an inline span, and the same call takes it back out.

The slash menu, gutter handle, drag-to-reorder, language picker, link menu, undo and the clipboard need no wiring. `Mark::Code` over more than one line makes a fence instead of an inline span, and the same call takes it back out.

The source is at `apps/gallery/src/patterns/editor.rs`. Copy the file.
