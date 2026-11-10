use crate::model;
use gpui::{AnyElement, App, canvas, div, prelude::*};
use markdown::{AppExt as _, Typography};
use std::rc::Rc;
use theme::TextStyle;
pub fn typography(cx: &App, zoom: f32) -> Typography {
    text_typography(cx, model::TEXT_SIZE, zoom)
}

pub fn text_typography(cx: &App, size: f32, zoom: f32) -> Typography {
    cx.typography().scaled(size / TextStyle::Body.size() * zoom)
}

#[derive(Clone)]
pub struct Measure {
    pub zoom: f32,
    pub report: Rc<dyn Fn(i64)>,
}

impl Measure {
    pub fn wrap(&self, content: AnyElement, padding: f32) -> AnyElement {
        let zoom = self.zoom;
        let report = self.report.clone();
        div()
            .relative()
            .w_full()
            .flex_none()
            .child(content)
            .child(
                canvas(
                    move |bounds, _, _| {
                        report((bounds.size.height.as_f32() / zoom + padding).ceil() as i64);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full(),
            )
            .into_any_element()
    }
}
