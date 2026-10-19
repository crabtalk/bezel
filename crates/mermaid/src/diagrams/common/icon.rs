//! Pele's `diagrams/common/icon.ts`.

use crate::{
    svg::builder::{esc_text, num},
    text::{Host, icon_markup},
};

/// A square for a named icon, in the form labels use. The host's resolver
/// supplies the drawing, if any.
pub fn icon_svg(host: &impl Host, name: &str, x: f64, y: f64, side: f64) -> String {
    format!(
        "<svg class=\"{p}-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</svg>",
        esc_text(name),
        num(x),
        num(y),
        num(side),
        num(side),
        icon_markup(host, name),
        p = crate::PREFIX,
    )
}
