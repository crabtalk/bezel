//! The point mark's symbols, as Vega draws them.

use std::{f32::consts::TAU, str::FromStr};

/// A point's symbol, by its Vega name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shape {
    #[default]
    Circle,
    Square,
    Cross,
    Diamond,
    TriangleUp,
    TriangleDown,
    TriangleRight,
    TriangleLeft,
    /// Pointing up, its centroid at the origin.
    Triangle,
    /// A narrow triangle pointing up.
    Wedge,
    /// Pointing up.
    Arrow,
    /// A horizontal line.
    Stroke,
}

/// `sqrt(3) / 2`.
const HALF_SQRT3: f32 = 0.866_025_4;
const TAN30: f32 = 0.577_350_26;
/// Straight segments in a circle's outline.
const CIRCLE_SEGMENTS: usize = 24;

impl Shape {
    /// Vega's `symbol` range, which a discrete shape field's values take in
    /// turn.
    pub const SCHEME: [Shape; 8] = [
        Shape::Circle,
        Shape::Square,
        Shape::TriangleUp,
        Shape::Cross,
        Shape::Diamond,
        Shape::TriangleRight,
        Shape::TriangleDown,
        Shape::TriangleLeft,
    ];

    /// The `index`-th of [`Shape::SCHEME`], round and round.
    pub fn nth(index: usize) -> Shape {
        Shape::SCHEME[index % Shape::SCHEME.len()]
    }

    /// Its outline at `size` square pixels, about the origin with y down,
    /// turned `angle` degrees clockwise. [`Shape::Stroke`] is two points, an
    /// open line; every other outline is closed.
    pub fn outline(self, size: f32, angle: f32) -> Vec<[f32; 2]> {
        let r = size.max(0.0).sqrt() / 2.0;
        let h = HALF_SQRT3 * r;
        let o = h - r * TAN30;
        let points: Vec<[f32; 2]> = match self {
            Shape::Circle => (0..CIRCLE_SEGMENTS)
                .map(|step| {
                    let a = TAU * step as f32 / CIRCLE_SEGMENTS as f32;
                    [r * a.cos(), r * a.sin()]
                })
                .collect(),
            Shape::Square => vec![[-r, -r], [r, -r], [r, r], [-r, r]],
            Shape::Cross => {
                let s = r / 2.5;
                vec![
                    [-r, -s],
                    [-r, s],
                    [-s, s],
                    [-s, r],
                    [s, r],
                    [s, s],
                    [r, s],
                    [r, -s],
                    [s, -s],
                    [s, -r],
                    [-s, -r],
                    [-s, -s],
                ]
            }
            Shape::Diamond => vec![[-r, 0.0], [0.0, -r], [r, 0.0], [0.0, r]],
            Shape::TriangleUp => vec![[0.0, -h], [-r, h], [r, h]],
            Shape::TriangleDown => vec![[0.0, h], [-r, -h], [r, -h]],
            Shape::TriangleRight => vec![[h, 0.0], [-h, -r], [-h, r]],
            Shape::TriangleLeft => vec![[-h, 0.0], [h, -r], [h, r]],
            Shape::Triangle => vec![[0.0, -h - o], [-r, h - o], [r, h - o]],
            Shape::Wedge => vec![[0.0, -h - o], [-r / 4.0, h - o], [r / 4.0, h - o]],
            Shape::Arrow => {
                let (s, t, v) = (r / 7.0, r / 2.5, r / 8.0);
                vec![
                    [-s, r],
                    [s, r],
                    [s, -v],
                    [t, -v],
                    [0.0, -r],
                    [-t, -v],
                    [-s, -v],
                ]
            }
            Shape::Stroke => vec![[-r, 0.0], [r, 0.0]],
        };
        if angle == 0.0 {
            return points;
        }
        let (sin, cos) = angle.to_radians().sin_cos();
        points
            .into_iter()
            .map(|[x, y]| [x * cos - y * sin, x * sin + y * cos])
            .collect()
    }
}

impl FromStr for Shape {
    type Err = ();

    /// A Vega symbol name. An SVG path is `Err`.
    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "circle" => Shape::Circle,
            "square" => Shape::Square,
            "cross" => Shape::Cross,
            "diamond" => Shape::Diamond,
            "triangle-up" => Shape::TriangleUp,
            "triangle-down" => Shape::TriangleDown,
            "triangle-right" => Shape::TriangleRight,
            "triangle-left" => Shape::TriangleLeft,
            "triangle" => Shape::Triangle,
            "wedge" => Shape::Wedge,
            "arrow" => Shape::Arrow,
            "stroke" => Shape::Stroke,
            _ => return Err(()),
        })
    }
}
