---
title: Text field
description: A single-line gpui text field with IME, selection, clipboard and undo — an entity you hold, with its key bindings scoped to the field.
---

The one component in the library that is not a plain function. Editing needs state — content, selection, the IME marked range, a focus handle — so a field is an entity the caller holds, the way SwiftUI's `TextField` binds to `@State`:

```rust
use ui::input::{self, TextField};

input::init(cx); // once, at startup

let field = cx.new(|cx| TextField::new(cx).with_placeholder("Search…"));
// …then render it: .child(field.clone())
```

Read and write it through the entity — `content()`, `set_content()`, `clear()`, `cursor()` for the byte offset the caret sits at. `set_content` clears the undo history: a programmatic reset is not something the user did, so there is nothing to walk back past.

`init` is a convenience, not a requirement. Every action is a public type and every binding is scoped to the field's key context, so `cmd-a` never comes to mean "select all text" for the whole application:

```rust
use ui::input::{self, Home, KEY_CONTEXT};

cx.bind_keys([KeyBinding::new("ctrl-a", Home, Some(KEY_CONTEXT))]);
```

It is all-or-nothing — take the defaults or bind the lot yourself.

Undo steps are runs, not keystrokes: a stretch of typing coalesces into one step, and the run ends when the caret moves or you switch between typing and deleting. That is structural rather than a millisecond threshold — adjacency is what actually separates a run of typing from a fresh thought somewhere else. The default ceiling is ten steps, which is deeper than it sounds for that reason; `with_undo_limit` moves it. A field is not a document, and nobody walks a search box back through a long history.

Motion follows the platform. On macOS `cmd` is line, `option` is word, and the emacs chords every native field honours — `ctrl-a`, `ctrl-e`, `ctrl-k`, `ctrl-b`/`ctrl-f`/`ctrl-h`/`ctrl-d` — come along; elsewhere `ctrl` is word. Word bounds are Unicode UAX#29 segments, so `foo.bar` and `foo_bar` are one word while `path/to/file` breaks. Arrows and backspace step by grapheme, so a flag emoji moves as a unit instead of shattering.

`offset_bounds(offset, window)` returns where a byte offset sits on screen. That is the anchor for anything hanging off a position in the *text* rather than off the field — a mention picker under the `#` that opened it, handed to `popover::menu_at`. It is `None` until the field has painted once, since there is no shaped layout before then.
