//! WebView2 raises `AcceleratorKeyPressed` for a key the page is about to take
//! while it holds focus; gpui's window never sees the key.

use crate::page::Report;
use gpui::{App, KeyContext, Keymap, Keystroke, Modifiers, Window};
use std::{cell::RefCell, rc::Rc};
use webview2_com::{
    AcceleratorKeyPressedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
        COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, ICoreWebView2Controller,
    },
};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// What the handler reads to decide whether gpui takes a key.
pub(super) struct Keys {
    keymap: Rc<RefCell<Keymap>>,
    /// The key contexts on the view's dispatch path, as of the last
    /// frame it was focused in.
    contexts: RefCell<Vec<KeyContext>>,
}

impl Keys {
    pub(super) fn new(cx: &App) -> Self {
        Self {
            keymap: cx.key_bindings(),
            contexts: RefCell::default(),
        }
    }

    /// Call while the view's focus handle is focused.
    pub(super) fn watch(&self, window: &Window) {
        *self.contexts.borrow_mut() = window.context_stack();
    }

    /// Whether the keymap binds `keystroke`, alone or as the first of a
    /// sequence, in the watched contexts.
    fn bound(&self, keystroke: &Keystroke) -> bool {
        let Ok(keymap) = self.keymap.try_borrow() else {
            return false;
        };
        let (bindings, pending) =
            keymap.bindings_for_input(std::slice::from_ref(keystroke), &self.contexts.borrow());
        pending || !bindings.is_empty()
    }
}

pub(super) fn attach(
    controller: &ICoreWebView2Controller,
    keys: Rc<Keys>,
    reports: async_channel::Sender<Report>,
) {
    let mut token = 0;
    // SAFETY: called on the thread that owns the controller, which runs
    // the handler on the same thread.
    unsafe {
        let _ = controller.add_AcceleratorKeyPressed(
            &AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let mut vkey = 0;
                args.VirtualKey(&mut vkey)?;
                let Some(keystroke) = keystroke(VIRTUAL_KEY(vkey as u16)) else {
                    return Ok(());
                };
                if keys.bound(&keystroke) {
                    args.SetHandled(true)?;
                    let _ = reports.try_send(Report::Key(keystroke));
                }
                Ok(())
            })),
            &mut token,
        );
    }
}

/// The keystroke gpui's Windows platform makes of `vkey` under the held
/// modifiers; `None` unless ctrl or alt is held or `vkey` is a function key.
fn keystroke(vkey: VIRTUAL_KEY) -> Option<Keystroke> {
    let mut modifiers = Modifiers {
        control: held(VK_CONTROL),
        alt: held(VK_MENU),
        shift: held(VK_SHIFT),
        platform: held(VK_LWIN) || held(VK_RWIN),
        function: false,
    };
    let function = (VK_F1.0..=VK_F24.0).contains(&vkey.0);
    if !(modifiers.control || modifiers.alt || function) {
        return None;
    }
    let key = match named(vkey) {
        Some(key) => key.to_owned(),
        None if modifiers.shift && shifts(vkey) => {
            modifiers.shift = false;
            shifted(vkey)?
        }
        None => {
            // SAFETY: a plain query of the active keyboard layout.
            let char = unsafe { MapVirtualKeyW(u32::from(vkey.0), MAPVK_VK_TO_CHAR) };
            char::from_u32(char & 0xFFFF)
                .filter(|char| *char != '\0')?
                .to_ascii_lowercase()
                .to_string()
        }
    };
    Some(Keystroke {
        modifiers,
        key,
        key_char: None,
    })
}

fn held(vkey: VIRTUAL_KEY) -> bool {
    // SAFETY: reads the calling thread's key state.
    unsafe { GetKeyState(i32::from(vkey.0)) < 0 }
}

fn named(vkey: VIRTUAL_KEY) -> Option<&'static str> {
    const FUNCTION: [&str; 24] = [
        "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "f10", "f11", "f12", "f13", "f14",
        "f15", "f16", "f17", "f18", "f19", "f20", "f21", "f22", "f23", "f24",
    ];
    Some(match vkey {
        VK_SPACE => "space",
        VK_BACK => "backspace",
        VK_RETURN => "enter",
        VK_TAB => "tab",
        VK_UP => "up",
        VK_DOWN => "down",
        VK_RIGHT => "right",
        VK_LEFT => "left",
        VK_HOME => "home",
        VK_END => "end",
        VK_PRIOR => "pageup",
        VK_NEXT => "pagedown",
        VK_ESCAPE => "escape",
        VK_INSERT => "insert",
        VK_DELETE => "delete",
        VK_APPS => "menu",
        vkey if (VK_F1.0..=VK_F24.0).contains(&vkey.0) => FUNCTION[usize::from(vkey.0 - VK_F1.0)],
        _ => return None,
    })
}

/// Keys gpui names by their shifted character, dropping shift.
fn shifts(vkey: VIRTUAL_KEY) -> bool {
    (VK_0.0..=VK_9.0).contains(&vkey.0)
        || matches!(
            vkey,
            VK_OEM_1
                | VK_OEM_2
                | VK_OEM_3
                | VK_OEM_4
                | VK_OEM_5
                | VK_OEM_6
                | VK_OEM_7
                | VK_OEM_8
                | VK_OEM_102
                | VK_OEM_PLUS
                | VK_OEM_MINUS
                | VK_OEM_COMMA
                | VK_OEM_PERIOD
                | VK_ABNT_C1
        )
}

/// The character `vkey` types with shift alone held.
fn shifted(vkey: VIRTUAL_KEY) -> Option<String> {
    let mut state = [0u8; 256];
    state[usize::from(VK_SHIFT.0)] = 0x80;
    let mut buffer = [0u16; 8];
    // SAFETY: plain queries of the active keyboard layout; flag 0x4 leaves
    // the kernel's dead-key state alone.
    let written = unsafe {
        let scan = MapVirtualKeyW(u32::from(vkey.0), MAPVK_VK_TO_VSC);
        ToUnicode(u32::from(vkey.0), scan, Some(&state), &mut buffer, 0x4)
    };
    let written = usize::try_from(written).ok().filter(|n| *n > 0)?;
    String::from_utf16(&buffer[..written]).ok()
}
