//! [`ColorPicker`]: a saturation/value field, a hue strip, an optional alpha
//! strip and a hex field, drawn in gpui on every platform.
//!
//! The picker is the body of a card, not the popover around it. It emits
//! [`ColorPickerEvent::Changed`] on every drag step and on every hex entry
//! that parses; [`ColorPicker::set_color`] moves it without emitting.
//!
//! ```ignore
//! let picker = cx.new(|cx| ColorPicker::new(color, true, cx));
//! cx.subscribe(&picker, |view, _, event, cx| match event {
//!     ColorPickerEvent::Changed(color) => view.set_tint(*color, cx),
//! })
//! .detach();
//! ```

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Axis, Bounds, Context, DragMoveEvent, Entity, EntityId, EventEmitter, Hsla, MouseButton,
    MouseDownEvent, Pixels, Point, Rgba, Subscription, Window, canvas, div, linear_color_stop,
    linear_gradient, prelude::*, px,
};
use theme::{TextStyle, Theme, Typeset};

use crate::input::{FieldEvent, TextField};
use crate::{stack, widgets};

const FIELD_HEIGHT: f32 = 96.0;
const STRIP_HEIGHT: f32 = 12.0;
const KNOB: f32 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorPickerEvent {
    Changed(Hsla),
}

/// Hue, saturation, value and alpha, each in `0..=1`. Hue is kept apart from
/// the colour so a grey keeps the hue it was dragged at.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hsva {
    h: f32,
    s: f32,
    v: f32,
    a: f32,
}

impl Hsva {
    fn from_hsla(color: Hsla) -> Self {
        let Rgba { r, g, b, a } = color.to_rgb();
        let max = r.max(g).max(b);
        let delta = max - r.min(g).min(b);
        let h = if delta <= f32::EPSILON {
            color.h
        } else if max == r {
            ((g - b) / delta).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / delta + 2.0) / 6.0
        } else {
            ((r - g) / delta + 4.0) / 6.0
        };
        let s = if max <= f32::EPSILON {
            0.0
        } else {
            delta / max
        };
        Self { h, s, v: max, a }
    }

    fn to_hsla(self) -> Hsla {
        let Self { h, s, v, a } = self;
        let sector = (h.rem_euclid(1.0)) * 6.0;
        let chroma = v * s;
        let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match sector as u32 {
            0 => (chroma, x, 0.0),
            1 => (x, chroma, 0.0),
            2 => (0.0, chroma, x),
            3 => (0.0, x, chroma),
            4 => (x, 0.0, chroma),
            _ => (chroma, 0.0, x),
        };
        let m = v - chroma;
        Rgba {
            r: r + m,
            g: g + m,
            b: b + m,
            a,
        }
        .into()
    }
}

/// `#rrggbb`, or `#rrggbbaa` when `alpha` is set.
pub fn to_hex(color: Hsla, alpha: bool) -> String {
    let Rgba { r, g, b, a } = color.to_rgb();
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    match alpha {
        true => format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            byte(r),
            byte(g),
            byte(b),
            byte(a)
        ),
        false => format!("#{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b)),
    }
}

/// Six or eight hex digits, with or without a leading `#`.
pub fn parse_hex(text: &str) -> Option<Hsla> {
    let hex = text.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if !matches!(hex.len(), 6 | 8) || !hex.is_ascii() {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
    let alpha = match hex.len() {
        8 => byte(6)?,
        _ => 255,
    };
    let channel = |value: u8| value as f32 / 255.0;
    Some(
        Rgba {
            r: channel(byte(0)?),
            g: channel(byte(2)?),
            b: channel(byte(4)?),
            a: channel(alpha),
        }
        .into(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Area {
    Field,
    Hue,
    Alpha,
}

/// The drag payload. Carries the picker's entity so two pickers on screen do
/// not both follow one drag.
struct PickerDrag(EntityId, Area);

pub struct ColorPicker {
    hsva: Hsva,
    alpha: bool,
    hex: Entity<TextField>,
    bounds: [Rc<Cell<Bounds<Pixels>>>; 3],
    _hex: Subscription,
}

impl EventEmitter<ColorPickerEvent> for ColorPicker {}

impl ColorPicker {
    /// `alpha` shows the alpha strip and writes the hex with eight digits.
    /// Without it, every emitted colour is opaque.
    pub fn new(color: Hsla, alpha: bool, cx: &mut Context<Self>) -> Self {
        let mut hsva = Hsva::from_hsla(color);
        if !alpha {
            hsva.a = 1.0;
        }
        let text = to_hex(hsva.to_hsla(), alpha);
        let hex = cx.new(|cx| {
            let mut field = TextField::new(cx).with_placeholder(match alpha {
                true => "#rrggbbaa",
                false => "#rrggbb",
            });
            field.set_content(text, cx);
            field
        });
        let subscription = cx.subscribe(&hex, |this, field, event, cx| {
            if !matches!(event, FieldEvent::Changed(_)) {
                return;
            }
            // The picker's own writes come back here too, and change nothing.
            let Some(color) = parse_hex(field.read(cx).content())
                .filter(|color| to_hex(*color, this.alpha) != to_hex(this.color(), this.alpha))
            else {
                return;
            };
            let mut hsva = Hsva::from_hsla(color);
            if !this.alpha {
                hsva.a = 1.0;
            }
            hsva.h = match hsva.s <= f32::EPSILON {
                true => this.hsva.h,
                false => hsva.h,
            };
            this.hsva = hsva;
            cx.emit(ColorPickerEvent::Changed(hsva.to_hsla()));
            cx.notify();
        });
        Self {
            hsva,
            alpha,
            hex,
            bounds: Default::default(),
            _hex: subscription,
        }
    }

    pub fn color(&self) -> Hsla {
        self.hsva.to_hsla()
    }

    /// Move to `color` without emitting. A colour equal to the current one
    /// keeps the current hue.
    pub fn set_color(&mut self, color: Hsla, cx: &mut Context<Self>) {
        if to_hex(color, self.alpha) == to_hex(self.color(), self.alpha) {
            return;
        }
        let mut hsva = Hsva::from_hsla(color);
        if !self.alpha {
            hsva.a = 1.0;
        }
        self.hsva = hsva;
        self.sync_hex(cx);
        cx.notify();
    }

    fn sync_hex(&mut self, cx: &mut Context<Self>) {
        let text = to_hex(self.color(), self.alpha);
        self.hex.update(cx, |field, cx| field.set_content(text, cx));
    }

    fn point(&mut self, area: Area, position: Point<Pixels>, cx: &mut Context<Self>) {
        let mut bounds = self.bounds[area as usize].get();
        if area != Area::Field {
            // A strip's knob travels inside it, so its centre runs half a knob
            // in from either end.
            bounds.origin.x += px(KNOB / 2.0);
            bounds.size.width -= px(KNOB);
        }
        let x = widgets::axis_fraction(position, bounds, Axis::Horizontal, 0.0);
        match area {
            Area::Field => {
                self.hsva.s = x;
                self.hsva.v = 1.0 - widgets::axis_fraction(position, bounds, Axis::Vertical, 0.0);
            }
            Area::Hue => self.hsva.h = x,
            Area::Alpha => self.hsva.a = x,
        }
        self.sync_hex(cx);
        cx.emit(ColorPickerEvent::Changed(self.color()));
        cx.notify();
    }

    /// One draggable area: records its bounds each paint, jumps on press and
    /// follows the drag.
    fn area(
        &self,
        area: Area,
        body: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let entity = cx.entity_id();
        let bounds = self.bounds[area as usize].clone();
        body.id(("color-picker", area as usize))
            .relative()
            .cursor_pointer()
            .child(
                canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                    .absolute()
                    .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.point(area, event.position, cx)
                }),
            )
            .on_drag(PickerDrag(entity, area), |_, _, _, cx| {
                cx.new(|_| gpui::Empty)
            })
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<PickerDrag>, _, cx| {
                    let PickerDrag(owner, dragged) = *event.drag(cx);
                    if owner == entity && dragged == area {
                        this.point(area, event.event.position, cx);
                    }
                }),
            )
    }
}

fn knob_face() -> gpui::Div {
    div()
        .absolute()
        .size(px(KNOB))
        .rounded_full()
        .border_2()
        .border_color(gpui::white())
        .shadow_sm()
}

/// The field's knob, centred on the point it marks.
fn field_knob(left: f32, top: f32) -> gpui::Div {
    knob_face()
        .left(gpui::relative(left))
        .top(gpui::relative(top))
        .ml(px(-KNOB / 2.0))
        .mt(px(-KNOB / 2.0))
}

/// A strip's knob, kept inside the strip at either end.
fn strip_knob(at: f32) -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left_0()
        .right(px(KNOB))
        .child(knob_face().top_0().left(gpui::relative(at)))
}

fn strip() -> gpui::Div {
    div().w_full().h(px(STRIP_HEIGHT)).rounded_full().flex()
}

impl Render for ColorPicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let Hsva { h, s, v, a } = self.hsva;
        let pure = Hsva {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        }
        .to_hsla();
        let opaque = Hsva {
            a: 1.0,
            ..self.hsva
        }
        .to_hsla();

        let field = div()
            .w_full()
            .h(px(FIELD_HEIGHT))
            .rounded(px(4.0))
            .bg(pure)
            .child(
                div()
                    .absolute()
                    .size_full()
                    .rounded(px(4.0))
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(gpui::white(), 0.0),
                        linear_color_stop(gpui::white().opacity(0.0), 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .size_full()
                    .rounded(px(4.0))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(gpui::black().opacity(0.0), 0.0),
                        linear_color_stop(gpui::black(), 1.0),
                    )),
            )
            .child(field_knob(s, 1.0 - v));

        // gpui gradients take two stops, so the hue runs as six segments.
        let hue_at = |at: f32| {
            Hsva {
                h: at,
                s: 1.0,
                v: 1.0,
                a: 1.0,
            }
            .to_hsla()
        };
        let hue = strip()
            .children((0..6).map(|segment| {
                let from = segment as f32 / 6.0;
                div()
                    .flex_1()
                    .h_full()
                    .when(segment == 0, |div| div.rounded_l_full())
                    .when(segment == 5, |div| div.rounded_r_full())
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(hue_at(from), 0.0),
                        linear_color_stop(hue_at(from + 1.0 / 6.0), 1.0),
                    ))
            }))
            .child(strip_knob(h));

        let alpha = self.alpha.then(|| {
            let strip = strip()
                .bg(theme.ink(0.12))
                .child(
                    div()
                        .absolute()
                        .size_full()
                        .rounded_full()
                        .bg(linear_gradient(
                            90.0,
                            linear_color_stop(opaque.opacity(0.0), 0.0),
                            linear_color_stop(opaque, 1.0),
                        )),
                )
                .child(strip_knob(a));
            self.area(Area::Alpha, strip, cx)
        });

        stack::column()
            .w_full()
            .gap(px(8.0))
            .child(self.area(Area::Field, field, cx))
            .child(self.area(Area::Hue, hue, cx))
            .children(alpha)
            .child(
                stack::row()
                    .gap(px(8.0))
                    .items_center()
                    .child(
                        div()
                            .flex_none()
                            .size(px(20.0))
                            .rounded_full()
                            .border_1()
                            .border_color(theme.ink(0.25))
                            .bg(self.color()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_style(TextStyle::Callout)
                            .font_family(theme.font_mono.clone())
                            .child(self.hex.clone()),
                    ),
            )
    }
}
