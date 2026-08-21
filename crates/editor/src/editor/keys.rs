//! The keymap: every action the editor answers to, and the chords bound to it.
//!
//! Separate from the surface because it is configuration rather than behaviour
//! — an app that wants a different keymap replaces this call, not the editor.

use gpui::{App, KeyBinding, actions};

use crate::editor::CONTEXT;

actions!(
    bezel_editor,
    [
        Backspace,
        Delete,
        KillLine,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteToHome,
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        SelectAll,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        SplitBlock,
        Indent,
        Outdent,
        Dismiss,
        Copy,
        Cut,
        Paste,
        Undo,
        Redo,
        ToggleBold,
        ToggleItalic,
        ToggleStrike,
        ToggleCode,
        MoveBlockUp,
        MoveBlockDown,
        DuplicateBlock,
        RemoveBlock,
    ]
);

/// Install the editor's key bindings. Scoped to the editor's own key context,
/// so binding `tab` here does not make `tab` mean "indent" for the whole app.
///
/// The chords are [`ui::input::TextField`]'s, because a document is not the place to
/// invent a second set: what `alt-left` does in a search box is what a reader
/// expects it to do here.
pub fn init(cx: &mut App) {
    let ctx = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
        KeyBinding::new("enter", SplitBlock, ctx),
        KeyBinding::new("tab", Indent, ctx),
        KeyBinding::new("shift-tab", Outdent, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);

    // `MoveBlockUp`, `MoveBlockDown`, `DuplicateBlock` and `RemoveBlock` are
    // deliberately unbound. Every chord that fits is already taken by something
    // standard — `cmd-shift-up`/`down` select to the ends of a document on
    // macOS, `alt-up`/`down` move by paragraph — and shadowing one of those in
    // a text surface is worse than reaching the block menu for it. They are
    // actions so an app can bind what suits its own keymap.
    //
    // The emacs kill ring is unbound for the same reason and stays unbuilt with
    // it: `ctrl-w`, `alt-w` and `ctrl-y` collide with `cmd-x`, `cmd-c`, `cmd-v`
    // and `cmd-w`, so a kill deletes and `cmd-x` is how text travels.

    #[cfg(target_os = "macos")]
    cx.bind_keys([
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-z", Undo, ctx),
        KeyBinding::new("cmd-shift-z", Redo, ctx),
        KeyBinding::new("cmd-b", ToggleBold, ctx),
        KeyBinding::new("cmd-i", ToggleItalic, ctx),
        KeyBinding::new("cmd-e", ToggleCode, ctx),
        KeyBinding::new("cmd-shift-x", ToggleStrike, ctx),
        // cmd = line, option = word: the macOS convention.
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("alt-left", WordLeft, ctx),
        KeyBinding::new("alt-right", WordRight, ctx),
        KeyBinding::new("alt-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("alt-shift-right", SelectWordRight, ctx),
        // The emacs bindings macOS honours in every native text field.
        KeyBinding::new("ctrl-a", Home, ctx),
        KeyBinding::new("ctrl-e", End, ctx),
        KeyBinding::new("ctrl-b", Left, ctx),
        KeyBinding::new("ctrl-f", Right, ctx),
        KeyBinding::new("ctrl-n", Down, ctx),
        KeyBinding::new("ctrl-p", Up, ctx),
        KeyBinding::new("ctrl-h", Backspace, ctx),
        KeyBinding::new("ctrl-d", Delete, ctx),
        // `ctrl-k` is the one chord emacs and AppKit spell the same way; the
        // rest are what option and cmd already mean for motion, deleting.
        KeyBinding::new("ctrl-k", KillLine, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("alt-delete", DeleteWordRight, ctx),
        KeyBinding::new("cmd-backspace", DeleteToHome, ctx),
    ]);

    #[cfg(not(target_os = "macos"))]
    cx.bind_keys([
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-b", ToggleBold, ctx),
        KeyBinding::new("ctrl-i", ToggleItalic, ctx),
        KeyBinding::new("ctrl-e", ToggleCode, ctx),
        KeyBinding::new("ctrl-shift-x", ToggleStrike, ctx),
        // ctrl = word on Windows/Linux, where there is no line modifier.
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-delete", DeleteWordRight, ctx),
        // `ctrl-k` stays free here: GTK entries kill the line with it, Windows
        // reads it as "insert link", and a chord with two meanings is one this
        // library does not get to claim.
    ]);
}
