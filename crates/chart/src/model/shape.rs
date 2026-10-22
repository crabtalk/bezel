//! The point mark's symbols, as Vega draws them.

use gpui::SharedString;
use lyon::{
    extra::parser::{ParserOptions, PathParser, Source},
    path::{Event, Path, iterator::PathIterator},
};
use std::{
    f32::consts::TAU,
    hash::{Hash, Hasher},
    str::FromStr,
    sync::Arc,
};

/// How far, in a path symbol's own units, its curves' flattened outlines
/// stray.
const TOLERANCE: f32 = 0.001;

/// A point's symbol, by its Vega name, or an SVG path.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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
    Path(Arc<SymbolPath>),
}

/// SVG path data as a symbol: Vega draws its coordinates times half the
/// square root of the symbol's size, about its position.
#[derive(Clone, Debug)]
pub struct SymbolPath {
    pub source: SharedString,
    /// Each subpath, its curves flattened, in the path's own units.
    rings: Vec<Vec<[f32; 2]>>,
}

impl PartialEq for SymbolPath {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

impl Eq for SymbolPath {}

impl Hash for SymbolPath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
    }
}

impl FromStr for SymbolPath {
    type Err = ();

    /// `Err` for text that is not SVG path data.
    fn from_str(source: &str) -> Result<Self, ()> {
        let mut builder = Path::builder_with_attributes(0);
        PathParser::new()
            .parse(
                &ParserOptions::DEFAULT,
                &mut Source::new(source.chars()),
                &mut builder,
            )
            .map_err(|_| ())?;
        let mut rings = Vec::new();
        let mut ring: Vec<[f32; 2]> = Vec::new();
        for event in builder.build().iter().flattened(TOLERANCE) {
            match event {
                Event::Begin { at } => ring = vec![[at.x, at.y]],
                Event::Line { to, .. } => ring.push([to.x, to.y]),
                Event::End { .. } => rings.push(std::mem::take(&mut ring)),
                _ => {}
            }
        }
        if rings.is_empty() {
            return Err(());
        }
        Ok(Self {
            source: source.to_string().into(),
            rings,
        })
    }
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
        Shape::SCHEME[index % Shape::SCHEME.len()].clone()
    }

    /// Its outlines at `size` square pixels, about the origin with y down,
    /// turned `angle` degrees clockwise: one for a named symbol, one per
    /// subpath for a path. [`Shape::Stroke`] is two points, an open line;
    /// every other outline is closed.
    pub fn outlines(&self, size: f32, angle: f32) -> Vec<Vec<[f32; 2]>> {
        let r = size.max(0.0).sqrt() / 2.0;
        let turn = |points: Vec<[f32; 2]>| -> Vec<[f32; 2]> {
            if angle == 0.0 {
                return points;
            }
            let (sin, cos) = angle.to_radians().sin_cos();
            points
                .into_iter()
                .map(|[x, y]| [x * cos - y * sin, x * sin + y * cos])
                .collect()
        };
        if let Shape::Path(path) = self {
            return path
                .rings
                .iter()
                .map(|ring| turn(ring.iter().map(|[x, y]| [x * r, y * r]).collect()))
                .collect();
        }
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
            Shape::Path(_) => Vec::new(),
        };
        vec![turn(points)]
    }
}

impl FromStr for Shape {
    type Err = ();

    /// A Vega symbol name, or SVG path data.
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
            path => Shape::Path(Arc::new(path.parse()?)),
        })
    }
}
