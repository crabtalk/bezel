//! Pele's `diagrams/er/markers.ts`.

use crate::{js, svg::builder::num};

/// How far a marker reaches back along its line from the box it touches.
pub const MARKER_LENGTH: f64 = 18.0;
const BAR: f64 = 5.0;
const FOOT: f64 = 6.0;
const FOOT_LENGTH: f64 = 10.0;
const RING: f64 = 3.5;

/// A crow's foot cardinality marker over the end of a line that meets a box
/// at (`x`, `y`) while travelling along the unit vector (`dx`, `dy`). Types
/// are Mermaid's lower-cased cardinalities.
pub fn er_marker(kind: &str, x: f64, y: f64, dx: f64, dy: f64) -> String {
    let p = crate::PREFIX;
    let at = |back: f64, side: f64| -> String {
        format!(
            "{},{}",
            num(x - dx * back - dy * side),
            num(y - dy * back + dx * side)
        )
    };
    let bar = |back: f64| format!("M{}L{}", at(back, -BAR), at(back, BAR));
    let foot = format!(
        "M{}L{}L{}",
        at(0.0, -FOOT),
        at(FOOT_LENGTH, 0.0),
        at(0.0, FOOT)
    );
    let ring = |back: f64| {
        format!(
            "<circle class=\"{p}-marker\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_bg)\"/>",
            num(x - dx * back),
            num(y - dy * back),
            js::num_str(RING)
        )
    };
    let path = |d: String| format!("<path class=\"{p}-marker\" d=\"{d}\"/>");

    match kind {
        "only_one" => path(bar(6.0) + &bar(10.0)),
        "zero_or_one" => path(bar(6.0)) + &ring(13.5),
        "one_or_more" => path(foot + &bar(FOOT_LENGTH + 3.5)),
        "zero_or_more" => path(foot) + &ring(FOOT_LENGTH + RING + 0.5),
        "md_parent" => format!(
            "<path class=\"{p}-marker\" d=\"M{}L{}L{}L{}Z\" fill=\"var(--_bg)\"/>",
            at(0.0, 0.0),
            at(6.0, 4.0),
            at(12.0, 0.0),
            at(6.0, -4.0)
        ),
        _ => String::new(),
    }
}
