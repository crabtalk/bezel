//! Reading the keymap back, so a printed accelerator is the one that fires.
//!
//! Every chord a surface shows — a menu row's trailing `⌘N`, a toolbar
//! button's tooltip — is a claim about the keymap, and a hand-typed string is
//! a claim nothing checks. Bind `cmd-b` somewhere else and the button still
//! says `⌘B`. So the label is resolved from the binding instead of written
//! beside it: [`shortcut`] asks gpui what is bound right now, and an action
//! with nothing bound to it prints nothing rather than a chord that is no
//! longer true.
//!
//! Dispatch is untouched. Nothing here fires an action — a menu still runs
//! what the app wires to it, and this only answers what the keyboard would
//! have run.
//!
//! # Replacing what a component binds
//!
//! Every `init` in this crate is its `bindings()` bound, and that list is
//! public. Three ways to change it, none of which is copying it out:
//!
//! ```ignore
//! cx.bind_keys(input::bindings());                       // the defaults
//! cx.bind_keys([KeyBinding::new("ctrl-w", input::DeleteWordLeft, ctx)]);
//! cx.bind_keys([KeyBinding::new("cmd-z", NoAction, ctx)]);
//! ```
//!
//! A later binding wins the dispatch, `gpui::NoAction` takes a chord away, and
//! an app that wants neither skips `init` and binds a filtered `bindings()`.
//! Whichever it does, what is printed follows — that is what the rest of this
//! module is for.

use gpui::{Action, KeyContext, KeybindingKeystroke, Modifiers, SharedString, Window};

/// The chord bound to `action` for whatever holds focus right now, formatted
/// for the platform.
///
/// The right question for a control that sits *with* the surface the binding
/// belongs to — a formatting bar above a focused editor, a button in the panel
/// it acts on. `None` when nothing is bound, or when what is focused is out of
/// the binding's context.
pub fn shortcut(action: &dyn Action, window: &Window) -> Option<SharedString> {
    let binding = window.highest_precedence_binding_for_action(action)?;
    Some(format(binding.keystrokes()))
}

/// The chord bound to `action` in a named key context, whatever holds focus.
///
/// The right question for a control that names a chord belonging to a surface
/// that is not focused — a menu row printing the editor's `⌘B` while the menu
/// itself holds focus. The context is the one the binding was scoped to:
/// [`crate::input::KEY_CONTEXT`], `editor::CONTEXT`, and so on.
pub fn shortcut_in(action: &dyn Action, context: &str, window: &Window) -> Option<SharedString> {
    // `new_with_defaults` rather than `default`, because it sets the `os` key
    // a binding is free to predicate on.
    let mut key_context = KeyContext::new_with_defaults();
    key_context.add(context.to_owned());
    let binding = window.highest_precedence_binding_for_action_in_context(action, key_context)?;
    Some(format(binding.keystrokes()))
}

/// Format a binding's keystrokes the way the platform writes them.
///
/// macOS gets the glyphs in the order Apple sets them — `⌃⌥⇧⌘`, modifiers
/// before the key, nothing between — and every other platform gets
/// `Ctrl+Shift+P`, with the platform key leading: `Win+Shift+S`. A
/// two-keystroke chord is the two spelled out with a space between, which is
/// how both platforms print a sequence.
///
/// Written here rather than taken from gpui's `Display` because that one
/// orders `⌘` before `⇧` and leaves `enter`, `delete` and `space` spelled as
/// words in among the glyphs.
pub fn format(keystrokes: &[KeybindingKeystroke]) -> SharedString {
    let mut out = String::new();
    for keystroke in keystrokes {
        if !out.is_empty() {
            out.push(' ');
        }
        modifiers(keystroke.modifiers(), &mut out);
        out.push_str(&key(keystroke.key()));
    }
    SharedString::from(out)
}

#[cfg(target_os = "macos")]
fn modifiers(modifiers: &Modifiers, out: &mut String) {
    // Apple's order, which is not the order the struct declares them in.
    if modifiers.function {
        out.push_str("fn");
    }
    if modifiers.control {
        out.push('⌃');
    }
    if modifiers.alt {
        out.push('⌥');
    }
    if modifiers.shift {
        out.push('⇧');
    }
    if modifiers.platform {
        out.push('⌘');
    }
}

#[cfg(not(target_os = "macos"))]
fn modifiers(modifiers: &Modifiers, out: &mut String) {
    // The platform key leads here, where on macOS it trails: Windows prints
    // its own chords `Win+Shift+S` and `Win+Ctrl+Shift+B`, and GNOME writes
    // `Super+` first for the same reason. Apple's `⌘` last is Apple's order,
    // and copying it here is how this printed a chord nobody else writes.
    if modifiers.platform {
        out.push_str(PLATFORM_MODIFIER);
    }
    if modifiers.control {
        out.push_str("Ctrl+");
    }
    if modifiers.alt {
        out.push_str("Alt+");
    }
    if modifiers.shift {
        out.push_str("Shift+");
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
const PLATFORM_MODIFIER: &str = "Super+";
#[cfg(target_os = "windows")]
const PLATFORM_MODIFIER: &str = "Win+";
#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
const PLATFORM_MODIFIER: &str = "Super+";

/// The key itself. macOS spells most of the named keys as glyphs; everywhere
/// else they stay words, title-cased so they sit beside `Ctrl+` as one label.
#[cfg(target_os = "macos")]
fn key(key: &str) -> String {
    let glyph = match key {
        "backspace" => "⌫",
        "delete" => "⌦",
        "enter" => "↩",
        "tab" => "⇥",
        "escape" => "⎋",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "pageup" => "⇞",
        "pagedown" => "⇟",
        "home" => "↖",
        "end" => "↘",
        "space" => "Space",
        // A single character is the character, upper case — `cmd-b` prints
        // `⌘B`. Anything longer is a name (`f1`, `capslock`) and is title-cased
        // with the rest of the words.
        key if key.chars().count() == 1 => return key.to_uppercase(),
        key => return title_case(key),
    };
    glyph.to_owned()
}

#[cfg(not(target_os = "macos"))]
fn key(key: &str) -> String {
    match key {
        key if key.chars().count() == 1 => key.to_uppercase(),
        key => title_case(key),
    }
}

fn title_case(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
