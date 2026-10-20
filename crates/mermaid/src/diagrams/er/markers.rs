//! Pele's `diagrams/er/markers.ts`.

use crate::geom::Point;
use std::fmt;

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

/// A crow's foot cardinality marker over the end of a line that meets a box
/// at (`x`, `y`) while travelling along the unit vector (`dx`, `dy`). Types
/// are Mermaid's lower-cased cardinalities.
pub fn er_marker(kind: &str, x: f64, y: f64, dx: f64, dy: f64) -> impl fmt::Display + '_ {
    fragment(move |f| {
        let p = crate::PREFIX;
        let at =
            |back: f64, side: f64| Point::new(x - dx * back - dy * side, y - dy * back + dx * side);
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
        match kind {
            "only_one" => {
                open(f)?;
                bar(f, 6.0)?;
                bar(f, 10.0)?;
                f.write_str(close)
            }
            "zero_or_one" => {
                open(f)?;
                bar(f, 6.0)?;
                f.write_str(close)?;
                ring(f, 13.5)
            }
            "one_or_more" => {
                open(f)?;
                foot(f)?;
                bar(f, FOOT_LENGTH + 3.5)?;
                f.write_str(close)
            }
            "zero_or_more" => {
                open(f)?;
                foot(f)?;
                f.write_str(close)?;
                ring(f, FOOT_LENGTH + RING + 0.5)
            }
            "md_parent" => write!(
                f,
                "<path class=\"{p}-marker\" d=\"M{}L{}L{}L{}Z\" fill=\"var(--_bg)\"/>",
                at(0.0, 0.0),
                at(6.0, 4.0),
                at(12.0, 0.0),
                at(6.0, -4.0)
            ),
            _ => Ok(()),
        }
    })
}
