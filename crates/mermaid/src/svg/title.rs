//! A diagram title above content laid out without one. Pele's
//! `svg/title.ts`.

use super::builder::num;
use crate::geom::Point;
use crate::svg::builder::TITLE;
use crate::text::Label;
use crate::text::{Host, Style};
use std::fmt::Write as _;

pub struct Titled {
    pub width: f64,
    pub height: f64,
    pub content: String,
}

pub fn with_title(
    title: Option<&str>,
    measure: &impl Host,
    size: f64,
    pad: f64,
    width: f64,
    height: f64,
    content: String,
) -> Titled {
    let label = Label::layout(title, false, measure, size, 4000.0, Style::BOLD);
    if label.height == 0.0 {
        return Titled {
            width,
            height,
            content,
        };
    }
    let shift = label.height + 12.0;
    let full = width.max((label.width + 2.0 * pad).ceil());
    let mut out = String::with_capacity(content.len() + 512);
    let _ = write!(
        out,
        "{}<g transform=\"translate({},{})\">{content}</g>",
        label.svg(Point::new(full / 2.0, pad + label.height / 2.0), TITLE),
        num((full - width) / 2.0),
        num(shift),
    );
    Titled {
        width: full,
        height: (height + shift).ceil(),
        content: out,
    }
}
