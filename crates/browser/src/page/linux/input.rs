use crate::page::{Page, platform::State};
use gpui::{
    self, App, DispatchPhase, Hitbox, KeyDownEvent, KeyUpEvent, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, NavigationDirection, Pixels, Point,
    ScrollWheelEvent, Window,
};
use serde_json::json;
use std::rc::Rc;

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

trait WpeButton {
    fn wpe_number(self) -> u32;
}
impl WpeButton for MouseButton {
    fn wpe_number(self) -> u32 {
        match self {
            Self::Left => 1,
            Self::Middle => 2,
            Self::Right => 3,
            Self::Navigate(NavigationDirection::Back) => 4,
            Self::Navigate(NavigationDirection::Forward) => 5,
        }
    }
}

impl Page {
    pub(crate) fn page_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let bounds = self.placed.get()?;
        let configuration = (*self.platform.route.configuration.lock().unwrap())?;
        if bounds.size.width <= gpui::px(0.) || bounds.size.height <= gpui::px(0.) {
            return None;
        }
        let local = position - bounds.origin;
        Some(gpui::point(
            local.x * (configuration.viewport.width / bounds.size.width),
            local.y * (configuration.viewport.height / bounds.size.height),
        ))
    }

    fn pointer(
        &self,
        kind: &str,
        position: Point<Pixels>,
        button: Option<MouseButton>,
        modifiers: Modifiers,
        count: usize,
    ) {
        let Some(local) = self.page_position(position) else {
            return;
        };
        let button = button.map(WpeButton::wpe_number).unwrap_or(0);
        if kind == "down" {
            self.platform
                .pressed
                .set(self.platform.pressed.get() | (1 << button));
        }
        if kind == "up" {
            if self.platform.pressed.get() & (1 << button) == 0 {
                return;
            }
            self.platform
                .pressed
                .set(self.platform.pressed.get() & !(1 << button));
        }
        self.platform.command(json!({"cmd":kind,"x":f32::from(local.x),"y":f32::from(local.y),"button":button,"count":count,"mods":modifiers.wpe_mask()}));
    }

    pub(super) fn install_input(this: &Rc<Self>, hitbox: &Hitbox, window: &mut Window) {
        let page = Rc::downgrade(this);
        let hitbox = hitbox.clone();
        let down_hitbox = hitbox.clone();
        let scroll_hitbox = hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase == DispatchPhase::Bubble
                && page.input.enabled.get()
                && down_hitbox.is_hovered(window)
                && page.window.get() == Some(window.window_handle())
            {
                page.platform.share_clipboard(cx);
                window.focus(&page.focus, cx);
                window.capture_pointer(down_hitbox.id);
                page.platform.capture.set(Some(down_hitbox.id));
                page.pointer(
                    "down",
                    event.position,
                    Some(event.button),
                    event.modifiers,
                    event.click_count,
                );
                cx.stop_propagation();
            }
        });
        let page = Rc::downgrade(this);
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase == DispatchPhase::Bubble
                && page.input.enabled.get()
                && page.window.get() == Some(window.window_handle())
                && (hitbox.is_hovered(window) || page.platform.pressed.get() != 0)
            {
                if hitbox.is_hovered(window) && !page.platform.hovered.replace(true) {
                    page.pointer("enter", event.position, None, event.modifiers, 0);
                }
                page.pointer("move", event.position, None, event.modifiers, 0);
                cx.stop_propagation();
            } else if phase == DispatchPhase::Bubble && page.platform.hovered.replace(false) {
                page.pointer("leave", event.position, None, event.modifiers, 0);
            }
        });
        let page = Rc::downgrade(this);
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase == DispatchPhase::Bubble
                && page.platform.pressed.get() & (1 << event.button.wpe_number()) != 0
                && page.window.get() == Some(window.window_handle())
            {
                page.pointer("up", event.position, Some(event.button), event.modifiers, 0);
                cx.stop_propagation();
            }
        });
        let page = Rc::downgrade(this);
        let hitbox = scroll_hitbox;
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else { return; };
            if phase != DispatchPhase::Bubble || !page.input.enabled.get()
                || page.window.get() != Some(window.window_handle()) || !hitbox.is_hovered(window) { return; }
            let Some(position) = page.page_position(event.position) else { return; };
            let (delta, precise) = match event.delta {
                gpui::ScrollDelta::Pixels(delta) => (gpui::point(f32::from(delta.x), f32::from(delta.y)), true),
                gpui::ScrollDelta::Lines(delta) => (delta, false),
            };
            page.platform.command(json!({"cmd":"scroll","x":f32::from(position.x),"y":f32::from(position.y),
                "dx":delta.x / 3.,"dy":delta.y / 3.,"precise":precise,"stop":event.touch_phase == gpui::TouchPhase::Ended,"mods":event.modifiers.wpe_mask()}));
            cx.stop_propagation();
        });
        let page = Rc::downgrade(this);
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase == DispatchPhase::Bubble
                && page.focus.is_focused(window)
                && page.input.enabled.get()
            {
                page.platform.share_clipboard(cx);
                page.platform.key(&event.keystroke, true);
                cx.stop_propagation();
            }
        });
        let page = Rc::downgrade(this);
        window.on_key_event(move |event: &KeyUpEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase == DispatchPhase::Bubble
                && page.focus.is_focused(window)
                && page.input.enabled.get()
            {
                page.platform.key(&event.keystroke, false);
                cx.stop_propagation();
            }
        });
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

    fn key(&self, stroke: &gpui::Keystroke, down: bool) {
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
    pub(crate) fn screen_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let bounds = self.placed.get()?;
        let configuration = (*self.platform.route.configuration.lock().unwrap())?;
        Some(
            bounds.origin
                + gpui::point(
                    position.x * (bounds.size.width / configuration.viewport.width),
                    position.y * (bounds.size.height / configuration.viewport.height),
                ),
        )
    }

    pub(crate) fn caret_bounds(&self) -> Option<gpui::Bounds<Pixels>> {
        let input = self.platform.route.input.lock().unwrap();
        let configuration = (*self.platform.route.configuration.lock().unwrap())?;
        let caret = &input["caret"];
        let x = caret[0].as_f64()? as f32 / configuration.scale;
        let y = caret[1].as_f64()? as f32 / configuration.scale;
        let width = caret[2].as_f64().unwrap_or(1.) as f32 / configuration.scale;
        let height = caret[3].as_f64().unwrap_or(18.) as f32 / configuration.scale;
        let origin = self.screen_position(gpui::point(gpui::px(x), gpui::px(y)))?;
        let end = self.screen_position(gpui::point(gpui::px(x + width), gpui::px(y + height)))?;
        Some(gpui::Bounds::new(origin, end - origin))
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
