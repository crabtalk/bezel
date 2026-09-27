//! Keystrokes and pastes as PTY bytes.

use super::*;

/// Whether a key was pressed, held down, or let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyEvent {
    #[default]
    Press,
    Repeat,
    Release,
}

/// Encode a keystroke as PTY bytes. `None` means "not ours" — the event should
/// fall through (e.g. the platform-primary shortcuts that drive app actions).
///
/// A repeat or a release only encodes under the kitty keyboard protocol's
/// event-types flag; there is no legacy sequence that says either.
///
/// `layout` is what gpui reports for the key that was pressed, where it
/// reports one. gpui folds a shifted key into `key` for everything but `a`-`z`
/// and clears `Modifiers::shift` with it, so `shift-1` arrives as `!` with no
/// shift held; the kitty encoding states the unshifted key and the shift bit
/// in separate parameters, and reads both from here. The legacy encoding
/// below wants the folded key and uses `key`.
pub fn keystroke_bytes(
    key: &str,
    key_char: Option<&str>,
    layout: Option<&KeyLayout>,
    mods: &Modifiers,
    mode: KeyboardMode,
    event: KeyEvent,
) -> Option<Vec<u8>> {
    // Platform-primary combos (Cmd on macOS, the super key elsewhere) belong to
    // the app keymap, never the PTY.
    if mods.platform {
        return None;
    }
    // Where there is no Cmd there is no `cmd-c`, and a terminal copies with
    // `ctrl-shift-c` instead. Not ours either way: `ctrl-c` has to stay SIGINT,
    // so the shifted pair is the one that falls through to the app.
    if mods.control && mods.shift && matches!(key, "c" | "v") {
        return None;
    }
    let reportable = mode.event_types && !matches!(event, KeyEvent::Press);
    if !matches!(event, KeyEvent::Press) && !reportable {
        return None;
    }
    // The protocol takes a key off its legacy encoding when the program asked
    // for every key, when the key is `Esc`, when control or alt is held, or
    // when the event itself is what has to be said.
    if mode.enhanced()
        && (mode.all_as_escapes || key == "escape" || mods.control || mods.alt || reportable)
        && let Some(bytes) = kitty_bytes(key, key_char, layout, mods, mode, event)
    {
        return Some(bytes);
    }
    // Arrows, editing and function keys carry their modifiers as a CSI
    // parameter. Everything below this is a key that has no parameter to put
    // one in, and folds the modifier into the bytes instead.
    if let Some(bytes) = named_bytes(key, mods, mode.app_cursor) {
        return Some(bytes);
    }
    if mods.alt {
        // ESC-prefix the same keystroke without alt.
        let inner = keystroke_bytes(
            key,
            key_char,
            layout,
            &Modifiers {
                alt: false,
                ..*mods
            },
            mode,
            event,
        )?;
        let mut out = vec![0x1b];
        out.extend(inner);
        return Some(out);
    }
    if mods.control {
        return control_bytes(key);
    }

    match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" => Some(if mods.shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        "escape" => Some(vec![0x1b]),
        "space" => Some(b" ".to_vec()),
        _ => {
            // Printable: prefer the typed character (IME/shift-aware).
            let text = key_char.filter(|c| !c.is_empty()).or({
                // Fall back to single-char key names ("a", "/", …).
                if key.chars().count() == 1 {
                    Some(key)
                } else {
                    None
                }
            })?;
            Some(text.as_bytes().to_vec())
        }
    }
}

/// A key's kitty code and the byte its sequence ends with.
///
/// The protocol's functional key definitions, as far as the key names gpui
/// hands us reach: the keypad cluster and the bare modifier keys arrive as
/// neither, so neither is reported. `f3` is `13~` rather than the `1;<mod>R`
/// its legacy form would give, which would be read as a cursor position
/// report.
pub(super) fn kitty_key(key: &str, layout: Option<&KeyLayout>) -> Option<(u32, u8)> {
    Some(match key {
        "escape" => (27, b'u'),
        "enter" => (13, b'u'),
        "tab" => (9, b'u'),
        "backspace" => (127, b'u'),
        "insert" => (2, b'~'),
        "delete" => (3, b'~'),
        "left" => (1, b'D'),
        "right" => (1, b'C'),
        "up" => (1, b'A'),
        "down" => (1, b'B'),
        "pageup" => (5, b'~'),
        "pagedown" => (6, b'~'),
        "home" => (1, b'H'),
        "end" => (1, b'F'),
        "f1" => (1, b'P'),
        "f2" => (1, b'Q'),
        "f3" => (13, b'~'),
        "f4" => (1, b'S'),
        "f5" => (15, b'~'),
        "f6" => (17, b'~'),
        "f7" => (18, b'~'),
        "f8" => (19, b'~'),
        "f9" => (20, b'~'),
        "f10" => (21, b'~'),
        "f11" => (23, b'~'),
        "f12" => (24, b'~'),
        "space" => (32, b'u'),
        _ => {
            // A text key is its own unshifted codepoint, which is what
            // `layout` carries: `key` holds the shifted one, `!` where the
            // protocol asks for `1`. Without a layout the key name is all
            // there is.
            let name = layout.map_or(key, |layout| layout.unshifted.as_str());
            let mut chars = name.chars();
            let (first, rest) = (chars.next()?, chars.next());
            if rest.is_some() {
                return None;
            }
            (first as u32, b'u')
        }
    })
}

/// `CSI <number>[:<shifted>] ; <modifiers>[:<event>] [; <text>] <final>`,
/// leaving out every parameter the protocol allows to be left out.
pub(super) fn kitty_bytes(
    key: &str,
    key_char: Option<&str>,
    layout: Option<&KeyLayout>,
    mods: &Modifiers,
    mode: KeyboardMode,
    event: KeyEvent,
) -> Option<Vec<u8>> {
    let (number, final_byte) = kitty_key(key, layout)?;
    // Shift reaches the report through the layout where gpui folded it out of
    // the modifiers.
    let shift = mods.shift || layout.is_some_and(|layout| layout.shift);
    let modifier = modifier_parameter(&Modifiers { shift, ..*mods }).unwrap_or(1);
    let event = match event {
        KeyEvent::Press => 1,
        KeyEvent::Repeat => 2,
        KeyEvent::Release => 3,
    };
    // Text rides along only where the program asked for it, and a release
    // produces none.
    let text = key_char
        .filter(|_| mode.associated_text && event != 3)
        .filter(|text| !text.is_empty() && !text.chars().any(|ch| ch.is_control()));

    // Flag 4's shifted key, which the protocol carries only where shift is in
    // the modifiers.
    let shifted = layout
        .filter(|_| mode.alternate_keys && shift)
        .and_then(|layout| layout.shifted.as_deref())
        .and_then(|shifted| shifted.chars().next())
        .map(|shifted| shifted as u32);

    let mut out = format!("\x1b[{number}");
    if let Some(shifted) = shifted {
        out.push_str(&format!(":{shifted}"));
    }
    if modifier > 1 || event > 1 || text.is_some() {
        out.push_str(&format!(";{modifier}"));
        if event > 1 {
            out.push_str(&format!(":{event}"));
        }
    }
    if let Some(text) = text {
        out.push(';');
        let codepoints: Vec<String> = text.chars().map(|ch| (ch as u32).to_string()).collect();
        out.push_str(&codepoints.join(":"));
    }
    out.push(final_byte as char);
    Some(out.into_bytes())
}

/// A key that states its modifiers as a CSI parameter, and where the parameter
/// goes.
pub(super) enum Named {
    /// `CSI 1 ; <modifier> <letter>` — the arrows, home and end, and F1 to F4.
    Letter(u8),
    /// `CSI <number> ; <modifier> ~` — the editing keys and F5 up.
    Tilde(u8),
}

/// Encode a key that carries its modifiers as a CSI parameter. `None` for a
/// key that is not one of those.
///
/// A held modifier rules out the SS3 form, which has nowhere to put the
/// parameter: `ctrl-left` is `CSI 1;5D` whatever DECCKM is set to.
pub(super) fn named_bytes(key: &str, mods: &Modifiers, app_cursor: bool) -> Option<Vec<u8>> {
    let named = match key {
        "up" => Named::Letter(b'A'),
        "down" => Named::Letter(b'B'),
        "right" => Named::Letter(b'C'),
        "left" => Named::Letter(b'D'),
        "home" => Named::Letter(b'H'),
        "end" => Named::Letter(b'F'),
        "f1" => Named::Letter(b'P'),
        "f2" => Named::Letter(b'Q'),
        "f3" => Named::Letter(b'R'),
        "f4" => Named::Letter(b'S'),
        "insert" => Named::Tilde(2),
        "delete" => Named::Tilde(3),
        "pageup" => Named::Tilde(5),
        "pagedown" => Named::Tilde(6),
        "f5" => Named::Tilde(15),
        "f6" => Named::Tilde(17),
        "f7" => Named::Tilde(18),
        "f8" => Named::Tilde(19),
        "f9" => Named::Tilde(20),
        "f10" => Named::Tilde(21),
        "f11" => Named::Tilde(23),
        "f12" => Named::Tilde(24),
        _ => return None,
    };
    Some(match (named, modifier_parameter(mods)) {
        // DECCKM moves the arrows and home/end to SS3. F1 to F4 are SS3
        // whatever it says.
        (Named::Letter(letter), None) => {
            let introducer = if app_cursor || matches!(letter, b'P'..=b'S') {
                b'O'
            } else {
                b'['
            };
            vec![0x1b, introducer, letter]
        }
        (Named::Letter(letter), Some(modifier)) => {
            format!("\x1b[1;{modifier}{}", letter as char).into_bytes()
        }
        (Named::Tilde(number), None) => format!("\x1b[{number}~").into_bytes(),
        (Named::Tilde(number), Some(modifier)) => format!("\x1b[{number};{modifier}~").into_bytes(),
    })
}

/// The `1 + bits` parameter a modified key states: shift 1, alt 2, control 4.
/// `None` where nothing is held, which is the key's own plain form.
pub(super) fn modifier_parameter(mods: &Modifiers) -> Option<u8> {
    let bits = u8::from(mods.shift) + u8::from(mods.alt) * 2 + u8::from(mods.control) * 4;
    (bits != 0).then_some(bits + 1)
}

/// Ctrl-key encoding (caret notation).
pub(super) fn control_bytes(key: &str) -> Option<Vec<u8>> {
    let mut chars = key.chars();
    let (c, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return match key {
            "space" => Some(vec![0x00]),
            "backspace" => Some(vec![0x08]),
            "enter" => Some(b"\r".to_vec()),
            _ => None,
        };
    }
    match c {
        'a'..='z' => Some(vec![c as u8 - b'a' + 1]),
        '@' => Some(vec![0x00]),
        '[' => Some(vec![0x1b]),
        '\\' => Some(vec![0x1c]),
        ']' => Some(vec![0x1d]),
        '^' => Some(vec![0x1e]),
        '_' | '/' => Some(vec![0x1f]),
        '?' => Some(vec![0x7f]),
        _ => None,
    }
}

/// Wrap pasted text for the PTY (bracketed-paste aware; strips the one control
/// sequence a paste could inject).
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let sanitized = text.replace("\x1b[201~", "");
    if bracketed {
        let mut out = b"\x1b[200~".to_vec();
        out.extend_from_slice(sanitized.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        sanitized.into_bytes()
    }
}
