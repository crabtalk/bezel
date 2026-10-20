//! Pele's `diagrams/class/markers.ts`.

use super::types::RelationType;
use crate::geom::Point;
use crate::svg::builder::{fragment, num};
use crate::svg::edges::Marker;
use std::fmt;

const TRIANGLE: f64 = 12.0;
const DIAMOND: f64 = 16.0;
const HALF: f64 = 6.0;
const RING: f64 = 5.0;

impl RelationType {
    /// How far the line stops short of the node so that it meets the back of
    /// the marker.
    pub fn trim(self) -> f64 {
        match self {
            Self::Extension => TRIANGLE,
            Self::Composition | Self::Aggregation => DIAMOND,
            Self::Lollipop => 2.0 * RING,
            Self::Dependency => Marker::Arrow.trim(),
        }
    }

    /// The end marker with its tip at `tip`, pointing along `dir`.
    pub fn draw(self, tip: Point, dir: Point, color: &str) -> impl fmt::Display + '_ {
        let (Point { x, y }, Point { x: dx, y: dy }) = (tip, dir);
        fragment(move |f| {
            let p = crate::PREFIX;
            let nx = -dy;
            let ny = dx;
            match self {
                Self::Extension => {
                    let bx = x - dx * TRIANGLE;
                    let by = y - dy * TRIANGLE;
                    write!(
                        f,
                        "<path class=\"{p}-marker\" d=\"M{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"/>",
                        num(x),
                        num(y),
                        num(bx + nx * HALF),
                        num(by + ny * HALF),
                        num(bx - nx * HALF),
                        num(by - ny * HALF)
                    )
                }
                Self::Composition | Self::Aggregation => {
                    let mx = x - (dx * DIAMOND) / 2.0;
                    let my = y - (dy * DIAMOND) / 2.0;
                    let half = HALF - 1.0;
                    write!(
                        f,
                        "<path class=\"{p}-marker\" d=\"M{},{}L{},{}L{},{}L{},{}Z\" stroke-linejoin=\"round\"",
                        num(x),
                        num(y),
                        num(mx + nx * half),
                        num(my + ny * half),
                        num(x - dx * DIAMOND),
                        num(y - dy * DIAMOND),
                        num(mx - nx * half),
                        num(my - ny * half),
                    )?;
                    if self == Self::Composition {
                        write!(f, " fill=\"{color}\"")?;
                    }
                    f.write_str("/>")
                }
                Self::Lollipop => write!(
                    f,
                    "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                    num(x - dx * RING),
                    num(y - dy * RING),
                    num(RING)
                ),
                Self::Dependency => write!(f, "{}", Marker::Arrow.draw(tip, dir, color)),
            }
        })
    }
}
