//! The blob avatar: one silhouette, two eyes and a little life.
//!
//! The shape is a radial profile rather than a pick from a roster, so a preset,
//! a name and [`Shape::random`] are the same kind of value and there is no
//! vocabulary to outgrow. Colour comes from the theme.
//!
//! [`mascot`] draws the same face as cells — one identity at two resolutions,
//! since a rail and a chat header showing one name should show one being.
//!
//! ```ignore
//! agent::avatar(Face::from("Sara").pose(t)).w(px(48.)).h(px(48.))
//! ```

mod eyes;
mod motion;
mod pixels;
mod shape;

pub use eyes::{Eye, Eyes};
pub use motion::{Beat, Motion};
pub use pixels::{GRID, mascot};
pub use shape::{Lobe, SAMPLES, Shape, seed};

use gpui::{
    AnyElement, Bounds, Hsla, IntoElement, PathBuilder, Pixels, Point, Styled, Window, canvas,
    hsla, point, px,
};
use theme::{Theme, contrast_ratio, flatten};

/// The share of the box the body fills at rest.
const FILL: f32 = 0.4;

/// A face: what to draw, and what to draw it in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub shape: Shape,
    pub eyes: Eyes,
    pub motion: Motion,
    /// `None` follows `theme.accent`.
    pub color: Option<Hsla>,
}

impl Face {
    pub fn new(shape: Shape) -> Self {
        Self {
            shape,
            eyes: Eyes::default(),
            motion: Motion::default(),
            color: None,
        }
    }

    pub fn eyes(mut self, eyes: Eyes) -> Self {
        self.eyes = eyes;
        self
    }

    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

impl From<u64> for Face {
    fn from(seed: u64) -> Self {
        Self::new(Shape::from(seed)).eyes(Eyes::from(seed))
    }
}

impl From<&str> for Face {
    fn from(name: &str) -> Self {
        Self::from(seed(name))
    }
}

impl Face {
    /// The face at `t`, motion already spent — everything the painter needs and
    /// the only form two faces can meet in.
    pub fn pose(&self, t: f32) -> Pose {
        let beat = self.motion.beat(t);
        let shape = self.motion.shape(self.shape, t);
        let breath = |(x, y): (f32, f32)| (x * beat.scale, y * beat.scale);
        Pose {
            outline: shape.outline().map(breath),
            eyes: self.eyes.place(&shape, self.motion.drift).map(|e| {
                let (cx, cy) = breath((e.cx + beat.gaze.0, e.cy + beat.gaze.1));
                Eye {
                    cx,
                    cy,
                    rx: e.rx * beat.scale,
                    ry: e.ry * beat.scale * (1.0 - beat.lid * 0.92),
                    ..e
                }
            }),
            color: self.color,
        }
    }
}

/// One face at one instant. A [`Shape`]'s harmonics are not interpolable — a
/// count of lobes has no half — so blending happens here, where the outline is
/// already sampled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub outline: [(f32, f32); SAMPLES],
    pub eyes: [Eye; 2],
    pub color: Option<Hsla>,
}

impl Pose {
    /// Point for point, which is only sound because every outline is sampled at
    /// the same angles — the property the whole representation is chosen for.
    pub fn lerp(a: &Self, b: &Self, k: f32) -> Self {
        let k = k.clamp(0.0, 1.0);
        let f = |x: f32, y: f32| x + (y - x) * k;
        Self {
            outline: std::array::from_fn(|i| {
                (
                    f(a.outline[i].0, b.outline[i].0),
                    f(a.outline[i].1, b.outline[i].1),
                )
            }),
            eyes: std::array::from_fn(|i| Eye {
                cx: f(a.eyes[i].cx, b.eyes[i].cx),
                cy: f(a.eyes[i].cy, b.eyes[i].cy),
                rx: f(a.eyes[i].rx, b.eyes[i].rx),
                ry: f(a.eyes[i].ry, b.eyes[i].ry),
                rot: f(a.eyes[i].rot, b.eyes[i].rot),
                n: f(a.eyes[i].n, b.eyes[i].n),
            }),
            // Through sRGB rather than around the hue wheel, which would sweep
            // a whole rainbow between two palette entries.
            color: match (a.color, b.color) {
                (Some(x), Some(y)) => Some(flatten(hsla(y.h, y.s, y.l, k), x)),
                (x, y) => {
                    if k < 0.5 {
                        x
                    } else {
                        y
                    }
                }
            },
        }
    }
}

/// The element, filling its layout bounds.
pub fn avatar(pose: Pose) -> AnyElement {
    canvas(
        move |_: Bounds<Pixels>, _, _| (),
        move |bounds, (), window, cx| {
            let theme = Theme::of(cx);
            let head = pose.color.unwrap_or(theme.accent);
            // Whichever end of the theme reads as a hole in this body.
            let ink = if contrast_ratio(head, theme.bg) >= contrast_ratio(head, theme.text) {
                theme.bg
            } else {
                theme.text
            };
            paint(window, bounds, &pose, head, ink);
        },
    )
    .size_full()
    .into_any_element()
}

fn paint(window: &mut Window, bounds: Bounds<Pixels>, pose: &Pose, head: Hsla, ink: Hsla) {
    let span = bounds.size.width.min(bounds.size.height).to_f64() as f32;
    let unit = span * FILL;
    let mid = bounds.center();
    let map = |x: f32, y: f32| point(mid.x + px(x * unit), mid.y + px(y * unit));

    window.paint_layer(bounds, |window| {
        fill(
            window,
            map(pose.outline[0].0, pose.outline[0].1),
            spline(&pose.outline, &map),
            head,
        );
        for eye in &pose.eyes {
            let (start, segs) = superellipse(eye, &map);
            fill(window, start, segs, ink);
        }
    });
}

type Cubic = (Point<Pixels>, Point<Pixels>, Point<Pixels>);

fn fill(window: &mut Window, start: Point<Pixels>, segs: Vec<Cubic>, color: Hsla) {
    let mut b = PathBuilder::fill();
    b.move_to(start);
    for (end, c1, c2) in segs {
        b.cubic_bezier_to(end, c1, c2);
    }
    b.close();
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

/// A closed Catmull-Rom through the sampled outline, as cubic Béziers — which
/// is what rounds the corners a polygon profile lands between samples.
fn spline(pts: &[(f32, f32); SAMPLES], map: &impl Fn(f32, f32) -> Point<Pixels>) -> Vec<Cubic> {
    let at = |i: isize| pts[i.rem_euclid(SAMPLES as isize) as usize];
    (0..SAMPLES as isize)
        .map(|i| {
            let ((x0, y0), (x1, y1), (x2, y2), (x3, y3)) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            (
                map(x2, y2),
                map(x1 + (x2 - x0) / 6.0, y1 + (y2 - y0) / 6.0),
                map(x2 - (x3 - x1) / 6.0, y2 - (y3 - y1) / 6.0),
            )
        })
        .collect()
}

/// `|x/rx|^n + |y/ry|^n = 1`, each quadrant one cubic whose control offset puts
/// the curve through the 45° point.
fn superellipse(e: &Eye, map: &impl Fn(f32, f32) -> Point<Pixels>) -> (Point<Pixels>, Vec<Cubic>) {
    // Past n ≈ 5.55 the offset exceeds the radius and the curve bulges outside
    // its own bounding box.
    let k = ((8.0 * 2f32.powf(-1.0 / e.n) - 4.0) / 3.0).min(1.0);
    let (ak, bk) = (e.rx * k, e.ry * k);
    let (rx, ry) = (e.rx, e.ry);
    let pts = [
        (rx, 0.0),
        (rx, bk),
        (ak, ry),
        (0.0, ry),
        (-ak, ry),
        (-rx, bk),
        (-rx, 0.0),
        (-rx, -bk),
        (-ak, -ry),
        (0.0, -ry),
        (ak, -ry),
        (rx, -bk),
        (rx, 0.0),
    ];
    let (sin, cos) = e.rot.to_radians().sin_cos();
    let at = |(x, y): (f32, f32)| map(e.cx + x * cos - y * sin, e.cy + x * sin + y * cos);
    (
        at(pts[0]),
        (1..13)
            .step_by(3)
            .map(|i| (at(pts[i + 2]), at(pts[i]), at(pts[i + 1])))
            .collect(),
    )
}
