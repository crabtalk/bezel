---
title: Text field
description: A single-line gpui text field with IME, selection, clipboard and undo — an entity you hold, with its key bindings scoped to the field.
---

```rust
use ui::input::{self, TextField};

input::init(cx); // once, at startup

let field = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
// …then render it: .child(field.clone())
```

The one component here that is not a plain function: editing needs state, so a field is an entity the caller holds, the way SwiftUI's `TextField` binds to `@State`.

## Rebinding

```rust
use ui::input::{self, Home, KEY_CONTEXT};

cx.bind_keys([KeyBinding::new("ctrl-a", Home, Some(KEY_CONTEXT))]);
```

`init` is a convenience. Every action is public and every binding is scoped to the field's key context, so `cmd-a` never comes to mean "select all" for the whole app. It is all-or-nothing — take the defaults or bind the lot.

## API

```rust
impl TextField {
    pub fn new(cx: &mut Context<Self>) -> Self;
    pub fn with_placeholder(self, placeholder: impl Into<SharedString>) -> Self;

    /// Default ten. Steps are runs, not keystrokes — a run ends when the caret
    /// moves or you switch between typing and deleting.
    pub fn with_undo_limit(self, limit: usize) -> Self;

    pub fn content(&self) -> &SharedString;

    /// Clears the undo history: a programmatic reset is not something the user
    /// did, so there is nothing to walk back past.
    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>);

    pub fn clear(&mut self, cx: &mut Context<Self>);

    /// The byte offset the caret sits at.
    pub fn cursor(&self) -> usize;

    /// Where a byte offset sits on screen — the anchor for a mention picker
    /// under the `#` that opened it. `None` until the field has painted once.
    pub fn offset_bounds(&self, offset: usize, window: &Window) -> Option<Bounds<Pixels>>;

    // ...
}
```

Motion follows the platform: on macOS `cmd` is line, `option` is word, plus the emacs chords every native field honours; elsewhere `ctrl` is word. Word bounds are UAX#29, and arrows step by grapheme so a flag emoji moves as a unit. Platform input ranges are UTF-16 and the field stores bytes.
