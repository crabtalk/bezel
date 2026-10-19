//! A diagram title above content laid out without one. Pele's
//! `svg/title.ts`.

use super::builder::{label_svg, num};
use crate::text::{Measure, Style, layout_label};

pub struct Titled {
    pub width: f64,
    pub height: f64,
    pub content: String,
}

pub fn with_title(
    title: Option<&str>,
    measure: &dyn Measure,
    size: f64,
    pad: f64,
    width: f64,
    height: f64,
    content: String,
) -> Titled {
    let label = layout_label(title, false, measure, size, 4000.0, Style::BOLD);
    if label.height == 0.0 {
        return Titled {
            width,
            height,
            content,
        };
    }
    let shift = label.height + 12.0;
    let full = width.max((label.width + 2.0 * pad).ceil());
    let p = crate::PREFIX;
    Titled {
        width: full,
        height: (height + shift).ceil(),
        content: format!(
            "{}<g transform=\"translate({},{})\">{content}</g>",
            label_svg(
                &label,
                full / 2.0,
                pad + label.height / 2.0,
                &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\"")
            ),
            num((full - width) / 2.0),
            num(shift),
        ),
    }
}
