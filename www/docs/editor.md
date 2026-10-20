---
title: Editor
description: A Notion-style block editor over the markdown document model — one entity, a scroll handle and an observe, and the markdown a save would write on every keystroke.
prompt: Add a notes pane with a block editor that opens notes.md and writes the markdown back to the file on every change.
---

```rust
use editor::{Editor, EditorEvent};

editor::init(cx);   // once, at startup

let scroll = ScrollHandle::new();   // the pane's: .track_scroll(&scroll) on it
let editor = cx.new(|cx| Editor::new(&source, cx).with_scroll(scroll.clone()));
cx.subscribe(&editor, |_, editor, event, cx| {
    if let EditorEvent::Changed = event {
        save(editor.read(cx).source());
    }
}).detach();
```

- `with_scroll` takes the handle of the pane the document scrolls in, not one inside the editor. Without it the caret does not follow typing down the page.
- `source()` is the markdown in either mode. Blocks you did not edit keep their original bytes.
- Editors made with `Editor::for_document` on one `Document` share its text, mode and undo stack. Each one keeps its own caret and scroll.
- Pasting a screenshot needs `cx.set_image_store(..)`. With no store installed, the paste does nothing.

## Make it yours

| call | what it decides |
| --- | --- |
| `with_chrome`, `Chrome` | which of the built-in handles and menus paint |
| `formatting`, `toggle_mark`, `set_block`, `turns` | a toolbar or block menu of your own (see [Ribbon](/docs/ribbon)) |
| `set_mode`, `toggle_source`, `Mode` | editing the raw markdown in the same view, with the same caret and undo |
| `Document::with_marks`, `with_mode`, `with_undo_limit` | one document's dialect, starting form and undo depth |
| `AppExt::set_image_store`, `ImageStore` | where pasted and dropped pictures are kept, and what URL is written |
| `AppExt::set_paste_handler`, `PasteContent` | rewriting a clipboard paste before the default runs |
| `with_image_overlay`, `with_image_overlay_corner` | a control of yours on hovered pictures |
| `with_text_size`, `AppExt::set_editor_text_size` | the base size, and the zoom steps |
| `with_base` | the directory relative image paths resolve against |
| `AppExt::set_slash_items`, `set_block_menu_items`, `set_handles` | the slash menu, the block menu and the handle glyphs |
| `AppExt::set_mention_source`, `Chrome::mention` | mention menus opened by a trigger character |
| `Document::set_anchors`, `EditorEvent::AnchorActivated` | comment ranges that follow edits |
| `keys::bindings`, `CONTEXT` | a keymap of your own |

## Keys

macOS shown; elsewhere `ctrl` stands in for `cmd`.

| key | does |
| --- | --- |
| `cmd-b` / `cmd-i` / `cmd-shift-x` | bold / italic / strike |
| `cmd-e` | code, or a fence over several lines |
| `cmd-shift-h` | the custom mark registered as `"highlight"` |
| `enter` / `shift-enter` | split the block / a line break |
| `tab` / `shift-tab` | indent / outdent |
| `cmd-z` / `cmd-shift-z` | undo / redo |
| `cmd-=` / `cmd--` / `cmd-0` | text size up / down / reset |
| `escape` | close a menu, leave a painted fence |
| `/` in an empty block | the slash menu |
