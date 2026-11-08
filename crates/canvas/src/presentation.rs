use gpui::{AnyElement, App, canvas, div, prelude::*};
use markdown::{AppExt as _, Typography};
use std::rc::Rc;
use theme::TextStyle;

const BODY: f32 = 16.;
const READABLE: f32 = 10.;
const SUMMARY_WIDTH: f32 = 160.;
const SUMMARY_HEIGHT: f32 = 64.;
const LABEL_WIDTH: f32 = 48.;
const LABEL_HEIGHT: f32 = 28.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    Full,
    Summary,
    Silhouette,
}

impl Detail {
    pub fn at(width: f32, height: f32, text: f32, full: bool) -> Self {
        if full {
            Self::Full
        } else if width < LABEL_WIDTH || height < LABEL_HEIGHT {
            Self::Silhouette
        } else if text < READABLE || width < SUMMARY_WIDTH || height < SUMMARY_HEIGHT {
            Self::Summary
        } else {
            Self::Full
        }
    }
}

pub fn typography(cx: &App, zoom: f32) -> Typography {
    cx.typography().scaled(BODY / TextStyle::Body.size() * zoom)
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
