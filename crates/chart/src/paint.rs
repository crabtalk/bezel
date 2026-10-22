//! gpui paths from points: lines, their dashes, and the points themselves.

use gpui::{Path, PathBuilder, Pixels, Point, point, px};

pub(crate) fn points(run: &[[f32; 2]]) -> Vec<Point<Pixels>> {
    run.iter().map(|&[x, y]| point(px(x), px(y))).collect()
}

/// `dash` begun `offset` into its pattern, as one pattern from its start.
pub(crate) fn dashed(dash: &[f32], offset: f32) -> Vec<f32> {
    let mut dash = dash.to_vec();
    if dash.len() % 2 == 1 {
        dash.extend_from_within(..);
    }
    let period: f32 = dash.iter().sum();
    if offset == 0.0 || period <= 0.0 {
        return dash;
    }
    let mut into = offset.rem_euclid(period);
    let mut index = 0;
    while index < dash.len() - 1 && into >= dash[index] {
        into -= dash[index];
        index += 1;
    }
    // A gap begun part way leads with an empty dash, so dashes stay dashes.
    let mut out = Vec::with_capacity(dash.len() + 3);
    if index % 2 == 1 {
        out.push(0.0);
    }
    out.push(dash[index] - into);
    out.extend(&dash[index + 1..]);
    out.extend(&dash[..index]);
    out.push(into);
    if out.len() % 2 == 1 {
        out.push(0.0);
    }
    out
}

/// A plain line along `run`.
pub(crate) fn polyline(run: &[[f32; 2]], width: f32, dash: Option<&[f32]>) -> Option<Path<Pixels>> {
    let mut builder = PathBuilder::stroke(px(width));
    if let Some(dash) = dash {
        builder = builder.dash_array(&dash.iter().map(|d| px(*d)).collect::<Vec<_>>());
    }
    builder.add_polygon(&points(run), false);
    builder.build().ok()
}
