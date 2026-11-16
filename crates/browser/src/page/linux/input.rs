use crate::page::{Page, platform::State};
use gpui::{self, App, Modifiers, Pixels, Point};
use serde_json::json;

pub(crate) trait WpeModifiers {
    fn wpe_mask(self) -> u32;
}
impl WpeModifiers for Modifiers {
    fn wpe_mask(self) -> u32 {
        u32::from(self.shift)
            | (u32::from(self.control) << 2)
            | (u32::from(self.alt) << 3)
            | (u32::from(self.platform) << 26)
    }
}

impl State {
    pub(crate) fn share_clipboard(&self, cx: &App) {
        let text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .unwrap_or_default();
        self.command(json!({"cmd":"clipboard","text":text}));
    }

    pub(crate) fn key(&self, stroke: &gpui::Keystroke, down: bool) {
        let printable = stroke.key_char.as_deref().filter(|text| {
            text.chars().count() == 1
                && !text.chars().any(char::is_control)
                && !stroke.modifiers.control
                && !stroke.modifiers.platform
        });
        let key = match printable.unwrap_or(&stroke.key) {
            "enter" => "Return",
            "backspace" => "BackSpace",
            "delete" => "Delete",
            "escape" => "Escape",
            "tab" => "Tab",
            "left" => "Left",
            "right" => "Right",
            "up" => "Up",
            "down" => "Down",
            "home" => "Home",
            "end" => "End",
            "pageup" => "Page_Up",
            "pagedown" => "Page_Down",
            "space" => "space",
            key => key,
        };
        self.command(
            json!({"cmd":if down {"key_down"} else {"key_up"}, "key":key,
            "text":stroke.key_char.as_deref().unwrap_or(""),"mods":stroke.modifiers.wpe_mask()}),
        );
    }
}

pub(super) fn cursor_style(name: &str) -> gpui::CursorStyle {
    use gpui::CursorStyle::*;
    match name {
        "pointer" => PointingHand,
        "text" => IBeam,
        "vertical-text" => IBeamCursorForVerticalLayout,
        "crosshair" | "cell" => Crosshair,
        "grab" => OpenHand,
        "grabbing" | "move" | "all-scroll" => ClosedHand,
        "not-allowed" | "no-drop" => OperationNotAllowed,
        "alias" => DragLink,
        "copy" => DragCopy,
        "context-menu" => ContextualMenu,
        "col-resize" => ResizeColumn,
        "row-resize" => ResizeRow,
        "ew-resize" => ResizeLeftRight,
        "ns-resize" => ResizeUpDown,
        "e-resize" => ResizeRight,
        "w-resize" => ResizeLeft,
        "n-resize" => ResizeUp,
        "s-resize" => ResizeDown,
        "nesw-resize" | "ne-resize" | "sw-resize" => ResizeUpRightDownLeft,
        "nwse-resize" | "nw-resize" | "se-resize" => ResizeUpLeftDownRight,
        // GPUI cannot hide the cursor: `none` keeps the arrow.
        _ => Arrow,
    }
}

impl Page {
    pub(crate) fn caret_bounds(&self) -> Option<gpui::Bounds<Pixels>> {
        let input = self.platform.route.input.lock().unwrap();
        let caret = &input["caret"];
        let x = caret[0].as_f64()? as f32;
        let y = caret[1].as_f64()? as f32;
        let width = caret[2].as_f64().unwrap_or(1.) as f32;
        let height = caret[3].as_f64().unwrap_or(18.) as f32;
        let origin = self.screen_position(gpui::point(gpui::px(x), gpui::px(y)))?;
        let end = self.screen_position(gpui::point(gpui::px(x + width), gpui::px(y + height)))?;
        Some(gpui::Bounds::new(origin, (end - origin).into()))
    }
}

impl State {
    pub(crate) fn replace_input(
        &self,
        range: Option<std::ops::Range<usize>>,
        text: &str,
        marked: bool,
    ) {
        let mut offset = 0i64;
        let mut length = 0usize;
        if let Some(range) = range {
            let input = self.route.input.lock().unwrap();
            let surrounding = input["text"].as_str().unwrap_or("");
            let units: Vec<_> = surrounding.encode_utf16().collect();
            let start = range.start.min(units.len());
            let end = range.end.max(start).min(units.len());
            let cursor_byte = input["cursor"].as_u64().unwrap_or(0) as usize;
            let cursor = surrounding
                .get(..cursor_byte)
                .unwrap_or(surrounding)
                .chars()
                .count();
            let before = String::from_utf16_lossy(&units[..start]).chars().count();
            offset = before as i64 - cursor as i64;
            length = String::from_utf16_lossy(&units[start..end]).chars().count();
        }
        self.command(json!({"cmd":if marked {"preedit"} else {"commit"}, "text":text,"replaceOffset":offset,"replaceLength":length}));
        *self.preedit.borrow_mut() = if marked {
            text.to_owned()
        } else {
            String::new()
        };
    }
}

impl crate::page::input::Input for State {
    fn mouse(
        &self,
        kind: &str,
        position: Point<Pixels>,
        button: u32,
        modifiers: Modifiers,
        count: usize,
    ) {
        self.command(json!({"cmd":kind,"x":f32::from(position.x),"y":f32::from(position.y),"button":button,"count":count,"mods":modifiers.wpe_mask()}));
    }
    fn wheel(
        &self,
        position: Point<Pixels>,
        delta: Point<f32>,
        precise: bool,
        stop: bool,
        modifiers: Modifiers,
    ) {
        self.command(json!({"cmd":"scroll","x":f32::from(position.x),"y":f32::from(position.y),"dx":delta.x/3.,"dy":delta.y/3.,"precise":precise,"stop":stop,"mods":modifiers.wpe_mask()}));
    }
}
