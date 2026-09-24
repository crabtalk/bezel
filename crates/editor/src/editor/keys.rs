//! The keymap: every action the editor answers to, and the chords bound to it.
//!
//! Separate from the surface because it is configuration rather than behaviour
//! — an app that wants a different keymap replaces this call, not the editor.

use gpui::{App, KeyBinding, actions};

use crate::editor::{CONTEXT, image};

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
        DocumentStart,
        DocumentEnd,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        SelectDocumentStart,
        SelectDocumentEnd,
        SelectAll,
        WordLeft,
        WordRight,
        SelectWordLeft,
        SelectWordRight,
        SplitBlock,
        SoftBreak,
        InsertParagraph,
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
        ToggleHighlight,
        MoveBlockUp,
        MoveBlockDown,
        DuplicateBlock,
        RemoveBlock,
        ConfirmUrl,
        CancelUrl,
        IncreaseTextSize,
        DecreaseTextSize,
        ResetTextSize,
    ]
);

/// Install the editor's key bindings — [`bindings`], bound.
pub fn init(cx: &mut App) {
    cx.bind_keys(bindings());
}

/// The editor's default keymap, as data, so an app can have it without having
/// to take it. Scoped to the editor's own key context, so binding `tab` here
/// does not make `tab` mean "indent" for the whole app.
///
/// The chords are [`ui::input::TextField`]'s, because a document is not the place to
/// invent a second set: what `alt-left` does in a search box is what a reader
/// expects it to do here.
///
/// An app with a keymap of its own has three ways in, none of which is copying
/// this list: bind over it (a later binding wins), bind `gpui::NoAction` to a
/// chord to take it away, or skip [`init`] and bind a filtered `bindings()`.
/// Either way the actions are public and [`CONTEXT`] names the scope, so what
/// the editor answers to is the app's to say.
pub fn bindings() -> Vec<KeyBinding> {
    let ctx = Some(CONTEXT);
    let mut bindings = Vec::new();
    bindings.extend([
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
        KeyBinding::new("shift-enter", SoftBreak, ctx),
        KeyBinding::new("ctrl-enter", InsertParagraph, ctx),
        KeyBinding::new("tab", Indent, ctx),
        KeyBinding::new("shift-tab", Outdent, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);

    // The URL prompt's own context, because the field holds focus while it is
    // open and the document behind it must not answer the same two keys.
    let prompt = Some(image::PROMPT_CONTEXT);
    bindings.extend([
        KeyBinding::new("enter", ConfirmUrl, prompt),
        KeyBinding::new("escape", CancelUrl, prompt),
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
    // and `cmd-w`, so a kill deletes and `cmd-x` is how text travels. Off
    // macOS `ctrl-y` is redo, which is the chord Windows reaches for.

    #[cfg(target_os = "macos")]
    bindings.extend([
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
        KeyBinding::new("cmd-shift-h", ToggleHighlight, ctx),
        // Three chords for one key: `cmd-+` is `cmd-shift-=` on the keyboards
        // that have no `+` of their own, and which of the two a platform
        // reports is not ours to guess.
        KeyBinding::new("cmd-=", IncreaseTextSize, ctx),
        KeyBinding::new("cmd-+", IncreaseTextSize, ctx),
        KeyBinding::new("cmd-shift-=", IncreaseTextSize, ctx),
        KeyBinding::new("cmd--", DecreaseTextSize, ctx),
        KeyBinding::new("cmd-0", ResetTextSize, ctx),
        // cmd = line, option = word: the macOS convention.
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("cmd-up", DocumentStart, ctx),
        KeyBinding::new("cmd-down", DocumentEnd, ctx),
        KeyBinding::new("cmd-shift-left", SelectHome, ctx),
        KeyBinding::new("cmd-shift-right", SelectEnd, ctx),
        KeyBinding::new("cmd-shift-up", SelectDocumentStart, ctx),
        KeyBinding::new("cmd-shift-down", SelectDocumentEnd, ctx),
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
    bindings.extend([
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("ctrl-z", Undo, ctx),
        KeyBinding::new("ctrl-shift-z", Redo, ctx),
        KeyBinding::new("ctrl-y", Redo, ctx),
        KeyBinding::new("ctrl-b", ToggleBold, ctx),
        KeyBinding::new("ctrl-i", ToggleItalic, ctx),
        KeyBinding::new("ctrl-e", ToggleCode, ctx),
        KeyBinding::new("ctrl-shift-x", ToggleStrike, ctx),
        KeyBinding::new("ctrl-shift-h", ToggleHighlight, ctx),
        KeyBinding::new("ctrl-=", IncreaseTextSize, ctx),
        KeyBinding::new("ctrl-+", IncreaseTextSize, ctx),
        KeyBinding::new("ctrl-shift-=", IncreaseTextSize, ctx),
        KeyBinding::new("ctrl--", DecreaseTextSize, ctx),
        KeyBinding::new("ctrl-0", ResetTextSize, ctx),
        // ctrl = word on Windows/Linux, where there is no line modifier.
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-shift-left", SelectWordLeft, ctx),
        KeyBinding::new("ctrl-shift-right", SelectWordRight, ctx),
        KeyBinding::new("ctrl-home", DocumentStart, ctx),
        KeyBinding::new("ctrl-end", DocumentEnd, ctx),
        KeyBinding::new("ctrl-shift-home", SelectDocumentStart, ctx),
        KeyBinding::new("ctrl-shift-end", SelectDocumentEnd, ctx),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-delete", DeleteWordRight, ctx),
        // `ctrl-k` stays free here: GTK entries kill the line with it, Windows
        // reads it as "insert link", and a chord with two meanings is one this
        // library does not get to claim.
    ]);

    bindings
}
