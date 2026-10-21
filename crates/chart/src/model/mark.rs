//! A layer's mark: its type, and the properties of a Vega-Lite mark
//! definition that no channel carries.

use std::str::FromStr;

use gpui::{FontStyle, FontWeight, SharedString};
use js::curve::Curve;

use crate::model::Encoding;

/// Mark types by their Vega-Lite names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark {
    Bar,
    Line,
    Area,
    /// A symbol per row, its shape from the shape channel.
    Point,
    /// Pie and donut slices, sized by `theta`.
    Arc,
    Rule,
    Rect,
    Text,
    /// A short line across its position.
    Tick,
    /// A line whose width follows `size` from point to point.
    Trail,
    Image,
    Circle,
    Square,
}

impl Mark {
    /// Whether its colour fills it when no `filled` property says: every
    /// mark but point, line and rule.
    pub fn filled(self) -> bool {
        !matches!(self, Mark::Point | Mark::Line | Mark::Rule)
    }

    /// Point, circle and square.
    pub fn is_symbol(self) -> bool {
        matches!(self, Mark::Point | Mark::Circle | Mark::Square)
    }
}

/// A mark definition's properties past its type and its channels' values.
/// `None` is the mark's own default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties {
    /// Whether `color` fills the mark rather than outlining it.
    pub filled: Option<bool>,
    /// How a line, area or trail runs between its points.
    pub interpolate: Interpolate,
    /// A cardinal curve's tension, a bundle's beta or a Catmull-Rom
    /// curve's alpha.
    pub tension: Option<f32>,
    pub orient: Option<Orient>,
    /// Top-left, top-right, bottom-right, bottom-left, in pixels. An arc
    /// rounds each of its corners by the top-left's.
    pub corner_radius: [Option<f32>; 4],
    /// A bar's corners at the end its value reaches, over `corner_radius`.
    pub corner_radius_end: Option<f32>,
    pub stroke_cap: Cap,
    pub stroke_join: Join,
    pub stroke_miter_limit: Option<f32>,
    /// How far into its pattern a dash starts, in pixels.
    pub stroke_dash_offset: f32,
    /// A tick's thickness, in pixels.
    pub thickness: Option<f32>,
    /// A bar's, rect's or tick's extent across its band; an image's width.
    pub width: Option<Extent>,
    pub height: Option<Extent>,
    /// Whether an image keeps its aspect ratio.
    pub aspect: Option<bool>,
    /// Which edge of a text or image its x is: Vega-Lite's `align`.
    pub align: Option<Align>,
    /// Which edge of a text or image its y is: Vega-Lite's `baseline`.
    pub baseline: Option<Baseline>,
    /// A text's offset from its position, in pixels.
    pub dx: f32,
    pub dy: f32,
    pub font: Option<SharedString>,
    pub font_size: Option<f32>,
    pub font_style: FontStyle,
    pub font_weight: Option<FontWeight>,
    /// A text's widest, in pixels, past which it ends in an ellipsis. `0`
    /// for no limit.
    pub limit: f32,
    /// What a text cut short at `limit` ends in, not empty. `None` for `"…"`.
    pub ellipsis: Option<SharedString>,
    /// Which end of a text cut short at `limit` keeps its characters.
    pub dir: Dir,
    /// Where a text's lines break.
    pub line_break: Option<LineBreak>,
    /// The distance between a text's lines, in pixels.
    pub line_height: Option<f32>,
    /// The angle an arc leaves between slices, in radians.
    pub pad_angle: f32,
    /// Pixels `x2` is drawn past its place. Without an `x2`, a mark's far
    /// end along x moves with `xOffset` instead.
    pub x2_offset: f32,
    pub y2_offset: f32,
    /// Radians clockwise a slice or text is turned past its `theta`, and a
    /// slice's end past its own without a `theta2`.
    pub theta_offset: f32,
    /// Radians clockwise `theta2` is turned past its place.
    pub theta2_offset: f32,
    /// Pixels a slice's outer edge or a text stands out past its `radius`.
    pub radius_offset: f32,
    /// Pixels `radius2` stands out past its place.
    pub radius2_offset: f32,
    /// Whether the marks are cut off at the plot's edge.
    pub clip: bool,
    /// Points drawn over a line or area's points.
    pub point: Option<Box<Overlay>>,
    /// A line drawn along an area's top.
    pub line: Option<Box<Overlay>>,
}

/// A mark drawn over a layer's own at the same positions: its own
/// properties, and channel values over the layer's channels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overlay {
    pub properties: Properties,
    pub encoding: Encoding,
}

/// A size in pixels, or a share of the band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extent {
    Pixels(f32),
    Band(f32),
}

impl Extent {
    /// In pixels, in a band `band` wide.
    pub fn of(self, band: f32) -> f32 {
        match self {
            Extent::Pixels(pixels) => pixels,
            Extent::Band(share) => share * band,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orient {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// A text's direction, Vega's `dir`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dir {
    /// A text cut short keeps its start.
    #[default]
    Ltr,
    /// A text cut short keeps its end.
    Rtl,
}

/// What a text's lines break at, as JavaScript's `split` takes it.
#[derive(Clone, Debug, PartialEq)]
pub enum LineBreak {
    /// Not empty.
    Text(SharedString),
    /// A regular expression's matches, its groups between the lines.
    Pattern {
        source: SharedString,
        flags: SharedString,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Baseline {
    Top,
    Middle,
    Bottom,
    Alphabetic,
    LineTop,
    LineBottom,
}

/// Vega's interpolation methods.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolate {
    #[default]
    Linear,
    LinearClosed,
    Step,
    StepBefore,
    StepAfter,
    Basis,
    BasisOpen,
    BasisClosed,
    Bundle,
    Cardinal,
    CardinalOpen,
    CardinalClosed,
    CatmullRom,
    Monotone,
    Natural,
}

impl Interpolate {
    /// The d3 curve Vega draws it with, `tension` its parameter, for a mark
    /// whose values run along x when `horizontal`.
    pub fn curve(self, tension: Option<f32>, horizontal: bool) -> Curve {
        let or = |default: f64| tension.map_or(default, f64::from);
        match self {
            Interpolate::Linear => Curve::Linear,
            Interpolate::LinearClosed => Curve::LinearClosed,
            Interpolate::Step => Curve::Step,
            Interpolate::StepBefore => Curve::StepBefore,
            Interpolate::StepAfter => Curve::StepAfter,
            Interpolate::Basis => Curve::Basis,
            Interpolate::BasisOpen => Curve::BasisOpen,
            Interpolate::BasisClosed => Curve::BasisClosed,
            Interpolate::Bundle => Curve::Bundle(or(0.85)),
            Interpolate::Cardinal => Curve::Cardinal(or(0.0)),
            Interpolate::CardinalOpen => Curve::CardinalOpen(or(0.0)),
            Interpolate::CardinalClosed => Curve::CardinalClosed(or(0.0)),
            Interpolate::CatmullRom => Curve::CatmullRom(or(0.5)),
            Interpolate::Monotone if horizontal => Curve::MonotoneY,
            Interpolate::Monotone => Curve::MonotoneX,
            Interpolate::Natural => Curve::Natural,
        }
    }
}

impl FromStr for Interpolate {
    type Err = ();

    /// A Vega interpolation name.
    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "linear" => Interpolate::Linear,
            "linear-closed" => Interpolate::LinearClosed,
            "step" => Interpolate::Step,
            "step-before" => Interpolate::StepBefore,
            "step-after" => Interpolate::StepAfter,
            "basis" => Interpolate::Basis,
            "basis-open" => Interpolate::BasisOpen,
            "basis-closed" => Interpolate::BasisClosed,
            "bundle" => Interpolate::Bundle,
            "cardinal" => Interpolate::Cardinal,
            "cardinal-open" => Interpolate::CardinalOpen,
            "cardinal-closed" => Interpolate::CardinalClosed,
            "catmull-rom" => Interpolate::CatmullRom,
            "monotone" => Interpolate::Monotone,
            "natural" => Interpolate::Natural,
            _ => return Err(()),
        })
    }
}
