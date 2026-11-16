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

use gpui::{
    Action, App, KeyContext, KeybindingKeystroke, Keystroke, Modifiers, OperatingSystem,
    SharedString, Window,
};

/// The chord bound to `action` for whatever holds focus right now, formatted
/// for the platform.
///
/// The right question for a control that sits *with* the surface the binding
/// belongs to — a formatting bar above a focused editor, a button in the panel
/// it acts on. `None` when nothing is bound, or when what is focused is out of
/// the binding's context.
pub fn shortcut(action: &dyn Action, window: &Window, cx: &App) -> Option<SharedString> {
    let binding = window.highest_precedence_binding_for_action(action)?;
    Some(format(binding.keystrokes(), cx))
}

/// The chord bound to `action` in a named key context, whatever holds focus.
///
/// The right question for a control that names a chord belonging to a surface
/// that is not focused — a menu row printing the editor's `⌘B` while the menu
/// itself holds focus. The context is the one the binding was scoped to:
/// [`crate::input::KEY_CONTEXT`], `editor::CONTEXT`, and so on.
pub fn shortcut_in(
    action: &dyn Action,
    context: &str,
    window: &Window,
    cx: &App,
) -> Option<SharedString> {
    // `new_with_defaults` rather than `default`, because it sets the `os` key
    // a binding is free to predicate on.
    let mut key_context = KeyContext::new_with_defaults(cx.platform_info().os);
    key_context.add(context.to_owned());
    let binding = window.highest_precedence_binding_for_action_in_context(action, key_context)?;
    Some(format(binding.keystrokes(), cx))
}

/// A chord nothing is bound to, written the way this platform writes it.
///
/// For a label that names a chord the keymap does not own: a menu describing
/// another app, a printed cheat sheet, a demo. Where there *is* a binding,
/// [`shortcut`] is the one to reach for — it cannot drift, and this can.
///
/// The syntax is gpui's own, so `secondary-s` is the primary accelerator —
/// `⌘S` on macOS, `Ctrl+S` elsewhere — while `cmd-s`, `super-s` and `win-s`
/// all name the platform key itself and print `⌘S` against `Win+S`. Whitespace
/// separates the keystrokes of a sequence, and one that will not parse is
/// dropped rather than printed wrong.
pub fn printed(chord: &str, cx: &App) -> SharedString {
    let keystrokes = chord
        .split_whitespace()
        .filter_map(|keystroke| Keystroke::parse_with_os(keystroke, cx.platform_info().os).ok())
        .map(KeybindingKeystroke::from_keystroke)
        .collect::<Vec<_>>();
    format(&keystrokes, cx)
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
pub fn format(keystrokes: &[KeybindingKeystroke], cx: &App) -> SharedString {
    let mut out = String::new();
    for keystroke in keystrokes {
        if !out.is_empty() {
            out.push(' ');
        }
        modifiers(keystroke.modifiers(), &mut out, cx.platform_info().os);
        out.push_str(&key(keystroke.key(), cx.platform_info().os));
    }
    SharedString::from(out)
}

fn modifiers(modifiers: &Modifiers, out: &mut String, os: OperatingSystem) {
    if os == OperatingSystem::Mac {
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
        return;
    }
    if modifiers.platform {
        out.push_str(match os {
            OperatingSystem::Windows => "Win+",
            _ => "Super+",
        });
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

fn key(key: &str, os: OperatingSystem) -> String {
    if os == OperatingSystem::Mac {
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
            key if key.chars().count() == 1 => return key.to_uppercase(),
            key => return title_case(key),
        };
        return glyph.to_owned();
    }
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
