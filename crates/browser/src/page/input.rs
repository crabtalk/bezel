use crate::page::Page;
use gpui::{
    self, App, DispatchPhase, Hitbox, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, Pixels, Point, ScrollEvent, Window,
};
#[cfg(target_os = "linux")]
use gpui::{KeyDownEvent, KeyUpEvent};
use std::rc::Rc;

trait PointerButton {
    fn number(self) -> u32;
}
impl PointerButton for MouseButton {
    fn number(self) -> u32 {
        match self {
            Self::Left => 1,
            Self::Middle => 2,
            Self::Right => 3,
            Self::Navigate(NavigationDirection::Back) => 4,
            Self::Navigate(NavigationDirection::Forward) => 5,
        }
    }
}

pub(crate) trait Input {
    fn mouse(
        &self,
        kind: &str,
        position: Point<Pixels>,
        button: u32,
        modifiers: Modifiers,
        count: usize,
    );
    fn wheel(
        &self,
        position: Point<Pixels>,
        delta: Point<f32>,
        precise: bool,
        stop: bool,
        modifiers: Modifiers,
    );
}

impl Page {
    pub(super) fn sync_pointer_capture(&self, hitbox: &Hitbox, window: &mut Window) {
        if self.platform.pressed.get() != 0 {
            if self.platform.capture.get() == window.captured_hitbox() {
                window.capture_pointer(hitbox.id);
                self.platform.capture.set(Some(hitbox.id));
            }
        } else if let Some(capture) = self.platform.capture.take() {
            if window.captured_hitbox() == Some(capture) {
                window.release_pointer();
            }
        }
    }
}

impl Page {
    pub(crate) fn page_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let bounds = self.placed.get()?;
        let viewport = self
            .presentation
            .get()
            .map(|p| p.viewport)
            .or_else(|| gpui::native_view::Viewport::try_from(bounds.size).ok())?;
        if bounds.size.width <= gpui::px(0.) || bounds.size.height <= gpui::px(0.) {
            return None;
        }
        let local = position - bounds.origin;
        Some(gpui::point(
            local.x * (viewport.width / bounds.size.width),
            local.y * (viewport.height / bounds.size.height),
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
        let button = button.map(PointerButton::number).unwrap_or(0);
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
        self.platform.mouse(kind, local, button, modifiers, count);
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
                && page.platform.pressed.get() & (1 << event.button.number()) != 0
                && page.window.get() == Some(window.window_handle())
            {
                page.pointer("up", event.position, Some(event.button), event.modifiers, 0);
                cx.stop_propagation();
            }
        });
        let page = Rc::downgrade(this);
        let hitbox = scroll_hitbox;
        window.on_mouse_event(move |event: &ScrollEvent, phase, window, cx| {
            let Some(page) = page.upgrade() else {
                return;
            };
            if phase != DispatchPhase::Bubble
                || !page.input.enabled.get()
                || page.window.get() != Some(window.window_handle())
                || !hitbox.is_hovered(window)
            {
                return;
            }
            let Some(position) = page.page_position(event.position) else {
                return;
            };
            let (delta, precise) = match event.delta {
                gpui::ScrollDelta::Pixels(delta) => {
                    (gpui::point(f32::from(delta.x), f32::from(delta.y)), true)
                }
                gpui::ScrollDelta::Lines(delta) => (delta, false),
            };
            page.platform.wheel(
                position,
                delta,
                precise,
                event.touch_phase == gpui::TouchPhase::Ended,
                event.modifiers,
            );
            cx.stop_propagation();
        });
        #[cfg(target_os = "linux")]
        {
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
}

impl Page {
    pub(crate) fn screen_position(&self, position: Point<Pixels>) -> Option<Point<Pixels>> {
        let bounds = self.placed.get()?;
        let viewport = self
            .presentation
            .get()
            .map(|p| p.viewport)
            .or_else(|| gpui::native_view::Viewport::try_from(bounds.size).ok())?;
        Some(
            bounds.origin
                + gpui::point(
                    position.x * (bounds.size.width / viewport.width),
                    position.y * (bounds.size.height / viewport.height),
                ),
        )
    }
}
