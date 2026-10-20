//! Pele's `diagrams/common/icon.ts`.

use crate::geom::Point;
use std::fmt;

use crate::{
    svg::builder::{fragment, icon_element},
    text::{Host, icon_markup},
};

/// A square for a named icon, in the form labels use, its top left corner at
/// `at`. The host's resolver supplies the drawing, if any.
pub fn icon_svg<'a>(
    host: &'a impl Host,
    name: &'a str,
    at: Point,
    side: f64,
) -> impl fmt::Display + 'a {
    icon_element(
        name,
        at,
        side,
        fragment(move |f| f.write_str(&icon_markup(host, name))),
    )
}
