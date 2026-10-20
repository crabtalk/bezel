//! Pele's `diagrams/er/markers.ts`.

use crate::geom::Point;
use std::fmt;

use super::types::Cardinality;
use crate::{
    js,
    svg::builder::{fragment, num},
};

/// How far a marker reaches back along its line from the box it touches.
pub const MARKER_LENGTH: f64 = 18.0;
const BAR: f64 = 5.0;
const FOOT: f64 = 6.0;
const FOOT_LENGTH: f64 = 10.0;
const RING: f64 = 3.5;

impl Cardinality {
    /// A crow's foot marker over the end of a line that meets a box at `tip`
    /// while travelling along the unit vector `dir`.
    pub fn marker(self, tip: Point, dir: Point) -> impl fmt::Display {
        let (Point { x, y }, Point { x: dx, y: dy }) = (tip, dir);
        fragment(move |f| {
            let p = crate::PREFIX;
            let at = |back: f64, side: f64| {
                Point::new(x - dx * back - dy * side, y - dy * back + dx * side)
            };
            let bar = |f: &mut fmt::Formatter<'_>, back: f64| {
                write!(f, "M{}L{}", at(back, -BAR), at(back, BAR))
            };
            let foot = |f: &mut fmt::Formatter<'_>| {
                write!(
                    f,
                    "M{}L{}L{}",
                    at(0.0, -FOOT),
                    at(FOOT_LENGTH, 0.0),
                    at(0.0, FOOT)
                )
            };
            let ring = |f: &mut fmt::Formatter<'_>, back: f64| {
                write!(
                    f,
                    "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\"/>",
                    num(x - dx * back),
                    num(y - dy * back),
                    js::num_str(RING)
                )
            };
            let open = |f: &mut fmt::Formatter<'_>| write!(f, "<path class=\"{p}-marker\" d=\"");
            let close = "\"/>";
            match self {
                Cardinality::OnlyOne => {
                    open(f)?;
                    bar(f, 6.0)?;
                    bar(f, 10.0)?;
                    f.write_str(close)
                }
                Cardinality::ZeroOrOne => {
                    open(f)?;
                    bar(f, 6.0)?;
                    f.write_str(close)?;
                    ring(f, 13.5)
                }
                Cardinality::OneOrMore => {
                    open(f)?;
                    foot(f)?;
                    bar(f, FOOT_LENGTH + 3.5)?;
                    f.write_str(close)
                }
                Cardinality::ZeroOrMore => {
                    open(f)?;
                    foot(f)?;
                    f.write_str(close)?;
                    ring(f, FOOT_LENGTH + RING + 0.5)
                }
                Cardinality::MdParent => write!(
                    f,
                    "<path class=\"{p}-marker\" d=\"M{}L{}L{}L{}Z\" fill=\"var(--_bg)\"/>",
                    at(0.0, 0.0),
                    at(6.0, 4.0),
                    at(12.0, 0.0),
                    at(6.0, -4.0)
                ),
            }
        })
    }
}
