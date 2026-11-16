use crate::page::platform::State;
use gpui::{App, Modifiers, Pixels, Point};
use webview2_com::Microsoft::Web::WebView2::Win32 as webview;
use windows::Win32::Foundation::POINT;

impl State {
    pub(super) fn cursor(&self) -> gpui::CursorStyle {
        let mut id = 0;
        if let Some(hosted) = self.hosted.borrow().as_ref() {
            if let Err(error) = unsafe { hosted.composition.SystemCursorId(&mut id) } {
                tracing::debug!(%error, "read browser cursor");
            }
        }
        match id {
            32513 => gpui::CursorStyle::IBeam,
            32649 => gpui::CursorStyle::PointingHand,
            32644 => gpui::CursorStyle::ResizeLeftRight,
            32645 => gpui::CursorStyle::ResizeUpDown,
            32642 => gpui::CursorStyle::ResizeUpLeftDownRight,
            32643 => gpui::CursorStyle::ResizeUpRightDownLeft,
            32648 => gpui::CursorStyle::OperationNotAllowed,
            32515 => gpui::CursorStyle::Crosshair,
            _ => gpui::CursorStyle::Arrow,
        }
    }
    pub(crate) fn share_clipboard(&self, _cx: &App) {}

    fn mouse_input(
        &self,
        kind: webview::COREWEBVIEW2_MOUSE_EVENT_KIND,
        position: Point<Pixels>,
        data: u32,
        modifiers: Modifiers,
    ) {
        let hosted = self.hosted.borrow();
        let Some(hosted) = hosted.as_ref() else {
            return;
        };
        let Some((_, scale)) = hosted.geometry else {
            return;
        };
        let mut keys = webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_NONE;
        if modifiers.shift {
            keys |= webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_SHIFT;
        }
        if modifiers.control {
            keys |= webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_CONTROL;
        }
        for (button, key) in [
            (
                1,
                webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_LEFT_BUTTON,
            ),
            (
                2,
                webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_MIDDLE_BUTTON,
            ),
            (
                3,
                webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_RIGHT_BUTTON,
            ),
            (4, webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_X_BUTTON1),
            (5, webview::COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS_X_BUTTON2),
        ] {
            if self.pressed.get() & (1 << button) != 0 {
                keys |= key;
            }
        }
        let position = POINT {
            x: (f32::from(position.x) * scale).round() as i32,
            y: (f32::from(position.y) * scale).round() as i32,
        };
        if let Err(error) = unsafe {
            hosted
                .composition
                .SendMouseInput(kind, keys, data, position)
        } {
            self.fail(error);
        }
    }
}

impl State {
    pub(super) fn mouse(
        &self,
        kind: &str,
        position: Point<Pixels>,
        button: u32,
        modifiers: Modifiers,
        count: usize,
    ) {
        use webview::COREWEBVIEW2_MOUSE_EVENT_KIND as Kind;
        // WebView2's values follow Win32 WM_MOUSE* messages.
        let base = match button {
            1 => 0x201,
            2 => 0x207,
            3 => 0x204,
            4 | 5 => 0x20b,
            _ => 0x200,
        };
        let kind = match kind {
            "down" if count >= 2 => Kind(base + 2),
            "down" => Kind(base),
            "up" => Kind(base + 1),
            "leave" => webview::COREWEBVIEW2_MOUSE_EVENT_KIND_LEAVE,
            _ => webview::COREWEBVIEW2_MOUSE_EVENT_KIND_MOVE,
        };
        let data = match button {
            4 => 1,
            5 => 2,
            _ => 0,
        };
        self.mouse_input(kind, position, data, modifiers);
    }
    pub(super) fn wheel(
        &self,
        position: Point<Pixels>,
        delta: Point<f32>,
        precise: bool,
        _stop: bool,
        modifiers: Modifiers,
    ) {
        let unit = if precise { 1. } else { 120. };
        for (value, kind) in [
            (delta.y, webview::COREWEBVIEW2_MOUSE_EVENT_KIND_WHEEL),
            (
                -delta.x,
                webview::COREWEBVIEW2_MOUSE_EVENT_KIND_HORIZONTAL_WHEEL,
            ),
        ] {
            let value = (value * unit).round() as i32;
            if value != 0 {
                self.mouse_input(kind, position, value as u32, modifiers);
            }
        }
    }
}
