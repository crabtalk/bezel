//! A button with stable focus and one activation callback.

use std::rc::Rc;

use gpui::{
    App, ElementId, IntoElement, Refineable, RenderOnce, SharedString, StyleRefinement, Window,
    div, prelude::*, px,
};
use icons::Icon;
use motion::Fade;
use theme::{ControlSize, Sizing, Theme};

use super::{ButtonRole, ButtonStyle, buttons::appearance};

/// A semantic button. Initialize [`crate::focus`] and mount its traversal
/// handler on the window root; identity retains focus across renders.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<Icon>,
    label_hidden: bool,
    style: ButtonStyle,
    size: ControlSize,
    role: Option<ButtonRole>,
    fade: Option<Fade>,
    enabled: bool,
    refinement: StyleRefinement,
    activate: Option<Rc<Activation>>,
}

type Activation = dyn Fn(&(), &mut Window, &mut App);

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            label_hidden: false,
            style: ButtonStyle::Ghost,
            size: ControlSize::Regular,
            role: None,
            fade: None,
            enabled: true,
            refinement: StyleRefinement::default(),
            activate: None,
        }
    }

    pub fn button_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    /// Resolve label and icon geometry together after configuration.
    pub fn control_size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Hide the visual label while retaining its accessible name.
    pub fn label_hidden(mut self) -> Self {
        self.label_hidden = true;
        self
    }

    pub fn hover_fade(mut self, fade: Fade) -> Self {
        self.fade = Some(fade);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Accepts `cx.listener(...)`; both Enter/Space and clicks call it once.
    pub fn on_press(mut self, activate: impl Fn(&(), &mut Window, &mut App) + 'static) -> Self {
        self.activate = Some(Rc::new(activate));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.refinement
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let focus = window.use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle());
        let focus = focus.read(cx).clone();
        let theme = Theme::of(cx);
        let enabled = self.enabled && self.activate.is_some();
        let frame = div()
            .control_size(self.size)
            // The permanent focus border occupies one point on each edge.
            .py(px((self.size.pad_y() - 1.0).max(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(Theme::SPACE))
            .border_1()
            .border_color(super::RING_SLOT)
            .when(self.label_hidden, |el| {
                el.px(px(0.0)).w(px(self.size.height()))
            });
        let fade = if enabled { self.fade } else { None };
        let plain_ghost = self.style == ButtonStyle::Ghost && fade.is_none();
        let (mut frame, tint) = appearance(theme, frame, self.style, self.role, fade, enabled);
        if enabled && plain_ghost {
            frame = frame.hover(|style| style.bg(theme.element_hover));
        }
        let tint = self.refinement.text.color.unwrap_or(tint);
        frame.style().refine(&self.refinement);
        let frame = frame
            .when_some(self.icon, |el, icon| {
                el.child(crate::icons::icon(icon).size(px(14.0)).text_color(tint))
            })
            .when(!self.label_hidden, |el| el.child(self.label.clone()))
            .when(enabled, |el| el.cursor_pointer())
            .when(!enabled, |el| el.opacity(0.5))
            .id(self.id)
            .role(gpui::Role::Button)
            .aria_label(self.label);
        crate::focus::pressable(theme, &focus, frame, enabled, move |event, window, cx| {
            if let Some(activate) = &self.activate {
                activate(event, window, cx);
            }
        })
    }
}
