//! Painting the format's own parts — a node's [`Shape`], an edge's line and
//! its ends — in window coordinates, for any view that draws a canvas.

use std::f32::consts::{FRAC_PI_2, PI};

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Rgba, Window, point, px};
use theme::Theme;

use crate::{
    model::{End, Shape, Stroke},
    path::Path,
};

/// How long a dash runs, and a dot, against the line's weight.
const DASH: f32 = 4.0;
const DOT: f32 = 1.0;
/// How much heavier a [`Stroke::Thick`] line is.
const THICK: f32 = 2.0;
/// How far a [`Shape::Cylinder`]'s rim dips, against its height.
const RIM: f32 = 0.12;

/// A JSON Canvas colour: a preset, or hex. Yellow and cyan have no token, so
/// they turn the hue of the token beside them.
pub fn color(theme: &Theme, color: &str) -> Option<Hsla> {
    match color {
        "1" => Some(theme.danger),
        "2" => Some(theme.warning),
        "3" => Some(Hsla {
            h: 50.0 / 360.0,
            ..theme.warning
        }),
        "4" => Some(theme.success),
        "5" => Some(Hsla {
            h: 185.0 / 360.0,
            ..theme.success
        }),
        "6" => Some(theme.accent),
        hex => Rgba::try_from(hex).ok().map(Into::into),
    }
}

/// `shape` filled with `fill` and outlined in `border`, filling `bounds`.
/// [`Shape::Rect`] and [`Shape::Round`] corners are `radius`.
pub fn shape(
    window: &mut Window,
    shape: Shape,
    bounds: Bounds<Pixels>,
    radius: f32,
    fill: Hsla,
    border: Hsla,
) {
    let mut filled = PathBuilder::fill();
    trace(&mut filled, shape, bounds, radius);
    if let Ok(path) = filled.build() {
        window.paint_path(path, fill);
    }
    let mut outline = PathBuilder::stroke(px(1.0));
    trace(&mut outline, shape, bounds, radius);
    if shape == Shape::Cylinder {
        // The front of the top rim, which the closed outline leaves out.
        let (x, y, w, h) = parts(bounds);
        let rim = h * RIM;
        let (cx, rx) = (x + w / 2.0, w / 2.0);
        outline.move_to(pt(point(x + w, y + rim)));
        ellipse_arc(&mut outline, (cx, y + rim), (rx, rim), 0.0, PI);
    }
    if let Ok(path) = outline.build() {
        window.paint_path(path, border);
    }
}

/// `path`, mapped to the window by `screen`, in `stroke` at `weight`, with the
/// marks its ends wear `mark` across.
pub fn edge(
    window: &mut Window,
    path: &Path,
    screen: impl Fn(Point<f32>) -> Point<f32>,
    stroke: Stroke,
    weight: f32,
    mark: f32,
    color: Hsla,
) {
    let weight = match stroke {
        Stroke::Thick => weight * THICK,
        _ => weight,
    };
    let mut line = PathBuilder::stroke(px(weight));
    line = match stroke {
        Stroke::Dashed => line.dash_array(&[px(DASH * weight), px(DASH * weight)]),
        Stroke::Dotted => line.dash_array(&[px(DOT * weight), px(DASH / 2.0 * weight)]),
        Stroke::Solid | Stroke::Thick => line,
    };
    for (ix, (a, c, b)) in path.segments.iter().enumerate() {
        if ix == 0 {
            line.move_to(pt(screen(*a)));
        }
        line.curve_to(pt(screen(*b)), pt(screen(*c)));
    }
    if let Ok(line) = line.build() {
        window.paint_path(line, color);
    }
    end(
        window,
        path.to_end,
        screen(path.end()),
        path.to_out,
        mark,
        color,
    );
    end(
        window,
        path.from_end,
        screen(path.start()),
        path.from_out,
        mark,
        color,
    );
}

/// The mark `end` puts at `tip`, `size` across, set back along `out`.
pub fn end(
    window: &mut Window,
    end: End,
    tip: Point<f32>,
    out: Point<f32>,
    size: f32,
    color: Hsla,
) {
    match end {
        End::None => {}
        End::Arrow => {
            let base = point(tip.x + out.x * size, tip.y + out.y * size);
            let half = point(-out.y * size / 2.0, out.x * size / 2.0);
            let mut path = PathBuilder::fill();
            path.move_to(pt(tip));
            path.line_to(pt(point(base.x + half.x, base.y + half.y)));
            path.line_to(pt(point(base.x - half.x, base.y - half.y)));
            path.close();
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
        End::Circle => {
            let r = size / 2.0;
            let mut path = PathBuilder::fill();
            ellipse(&mut path, (tip.x + out.x * r, tip.y + out.y * r), (r, r));
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
        End::Cross => {
            let at = point(tip.x + out.x * size, tip.y + out.y * size);
            let r = size / 2.0;
            let mut path = PathBuilder::stroke(px(1.5));
            for (dx, dy) in [(1.0, 1.0), (1.0, -1.0)] {
                path.move_to(pt(point(at.x - dx * r, at.y - dy * r)));
                path.line_to(pt(point(at.x + dx * r, at.y + dy * r)));
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, color);
            }
        }
    }
}

/// The outline of `shape` in `bounds`, closed.
fn trace(path: &mut PathBuilder, shape: Shape, bounds: Bounds<Pixels>, radius: f32) {
    let (x, y, w, h) = parts(bounds);
    let polygon = |path: &mut PathBuilder, corners: &[(f32, f32)]| {
        for (ix, &(cx, cy)) in corners.iter().enumerate() {
            match ix {
                0 => path.move_to(pt(point(cx, cy))),
                _ => path.line_to(pt(point(cx, cy))),
            }
        }
        path.close();
    };
    match shape {
        Shape::Rect => rounded(path, x, y, w, h, radius),
        Shape::Round => rounded(path, x, y, w, h, (radius * 2.0).min(h / 2.0)),
        Shape::Stadium => rounded(path, x, y, w, h, w.min(h) / 2.0),
        Shape::Circle => ellipse(path, (x + w / 2.0, y + h / 2.0), (w / 2.0, h / 2.0)),
        Shape::Diamond => polygon(
            path,
            &[
                (x + w / 2.0, y),
                (x + w, y + h / 2.0),
                (x + w / 2.0, y + h),
                (x, y + h / 2.0),
            ],
        ),
        Shape::Hexagon => {
            let inset = (h / 2.0).min(w / 4.0);
            polygon(
                path,
                &[
                    (x + inset, y),
                    (x + w - inset, y),
                    (x + w, y + h / 2.0),
                    (x + w - inset, y + h),
                    (x + inset, y + h),
                    (x, y + h / 2.0),
                ],
            )
        }
        Shape::Parallelogram => {
            let lean = (h / 2.0).min(w / 4.0);
            polygon(
                path,
                &[(x + lean, y), (x + w, y), (x + w - lean, y + h), (x, y + h)],
            )
        }
        Shape::Cylinder => {
            let rim = h * RIM;
            let (cx, rx) = (x + w / 2.0, w / 2.0);
            path.move_to(pt(point(x, y + rim)));
            ellipse_arc(path, (cx, y + rim), (rx, rim), PI, 2.0 * PI);
            path.line_to(pt(point(x + w, y + h - rim)));
            ellipse_arc(path, (cx, y + h - rim), (rx, rim), 0.0, PI);
            path.close();
        }
    }
}

/// A rectangle with corners `radius` round, closed.
fn rounded(path: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    path.move_to(pt(point(x + r, y)));
    path.line_to(pt(point(x + w - r, y)));
    ellipse_arc(path, (x + w - r, y + r), (r, r), -FRAC_PI_2, 0.0);
    path.line_to(pt(point(x + w, y + h - r)));
    ellipse_arc(path, (x + w - r, y + h - r), (r, r), 0.0, FRAC_PI_2);
    path.line_to(pt(point(x + r, y + h)));
    ellipse_arc(path, (x + r, y + h - r), (r, r), FRAC_PI_2, PI);
    path.line_to(pt(point(x, y + r)));
    ellipse_arc(path, (x + r, y + r), (r, r), PI, 1.5 * PI);
    path.close();
}

/// The whole ellipse at `center` with radii `radii`, closed.
fn ellipse(path: &mut PathBuilder, center: (f32, f32), radii: (f32, f32)) {
    path.move_to(pt(point(center.0 + radii.0, center.1)));
    ellipse_arc(path, center, radii, 0.0, 2.0 * PI);
    path.close();
}

/// An arc of the ellipse at `center` with radii `radii`, from angle `from` to
/// `to`, joined on from wherever the path is.
fn ellipse_arc(path: &mut PathBuilder, center: (f32, f32), radii: (f32, f32), from: f32, to: f32) {
    /// Quadratic pieces per quarter turn.
    const PER_QUARTER: f32 = 4.0;
    let at = |angle: f32| {
        point(
            center.0 + radii.0 * angle.cos(),
            center.1 + radii.1 * angle.sin(),
        )
    };
    let steps = ((to - from).abs() / FRAC_PI_2 * PER_QUARTER)
        .ceil()
        .max(1.0) as usize;
    let step = (to - from) / steps as f32;
    path.line_to(pt(at(from)));
    for ix in 0..steps {
        let (a, b) = (from + step * ix as f32, from + step * (ix + 1) as f32);
        // The control point of a quadratic that meets both ends' tangents.
        let mid = (a + b) / 2.0;
        let stretch = 1.0 / (step / 2.0).cos();
        let control = point(
            center.0 + radii.0 * stretch * mid.cos(),
            center.1 + radii.1 * stretch * mid.sin(),
        );
        path.curve_to(pt(at(b)), pt(control));
    }
}

fn parts(bounds: Bounds<Pixels>) -> (f32, f32, f32, f32) {
    (
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
    )
}

fn pt(p: Point<f32>) -> Point<Pixels> {
    point(px(p.x), px(p.y))
}
