//! What a position's axis, another channel's legend and the chart's title
//! draw, by Vega-Lite property. Each `None` is Vega-Lite's default for the
//! channel, field type and mark.

use crate::model::{Align, Baseline, Cap, Datum, Orient, Shape};
use d3::{
    format::{NumberFormat, Specifier},
    time::{Unit, date::LocalZone},
};
use gpui::{FontStyle, FontWeight, Hsla, SharedString};
use std::{fmt, sync::Arc};

/// A side of the plot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

impl Side {
    /// Top or bottom, whose guides run across.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Top | Side::Bottom)
    }
}

/// Where along its run a title stands: Vega's `anchor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    /// How far along its run, from 0 at the start to 1 at the end.
    pub fn fraction(self) -> f32 {
        match self {
            Anchor::Start => 0.0,
            Anchor::Middle => 0.5,
            Anchor::End => 1.0,
        }
    }
}

/// How labels a guide writes read: a d3 number format, or a d3 time format
/// on the UTC calendar, which is this crate's local one.
#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    Number(Specifier),
    Time(SharedString),
}

impl Format {
    /// `value` as it reads.
    pub fn write(&self, value: f64) -> String {
        match self {
            Format::Number(specifier) => NumberFormat::from(*specifier).format(value),
            Format::Time(specifier) => {
                let zone = LocalZone::utc();
                zone.date(value).format(specifier)
            }
        }
    }
}

/// Rewrites a guide's label: given its value, the text its format writes
/// and its place among the guide's labels from 0 to 1, the text shown.
#[derive(Clone)]
pub struct Relabel(pub Arc<Relabeling>);

pub type Relabeling = dyn Fn(&Datum, &str, f64) -> SharedString + Send + Sync;

/// The same function.
impl PartialEq for Relabel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Relabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Relabel")
    }
}

/// Whether a tick's property takes a value: given the tick's value and
/// its label's text.
#[derive(Clone)]
pub struct Test(pub Arc<Testing>);

pub type Testing = dyn Fn(&Datum, &str) -> bool + Send + Sync;

/// The same function.
impl PartialEq for Test {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Test")
    }
}

/// How a guide hides labels that overlap: Vega's `labelOverlap`. `Parity`
/// drops every other label, `Greedy` each that overlaps the last one kept,
/// until none overlap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlap {
    Off,
    Parity,
    Greedy,
}

/// How many ticks a guide asks its scale for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TickCount {
    /// About this many.
    Count(f64),
    /// One every `step` of `unit`, on a time scale.
    Every(Unit, f64),
}

/// How a guide's text is set. Angles are in degrees.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lettering {
    pub color: Option<Hsla>,
    pub opacity: Option<f32>,
    pub font: Option<SharedString>,
    /// In pixels.
    pub size: Option<f32>,
    pub weight: Option<FontWeight>,
    pub style: Option<FontStyle>,
    /// The widest, in pixels, past which the text ends in an ellipsis. `0`
    /// for no limit.
    pub limit: Option<f32>,
    pub line_height: Option<f32>,
    pub align: Option<Align>,
    pub baseline: Option<Baseline>,
    pub angle: Option<f32>,
}

impl Lettering {
    /// Its properties over `under`'s.
    pub fn over(&self, under: &Lettering) -> Lettering {
        Lettering {
            color: self.color.or(under.color),
            opacity: self.opacity.or(under.opacity),
            font: self.font.clone().or_else(|| under.font.clone()),
            size: self.size.or(under.size),
            weight: self.weight.or(under.weight),
            style: self.style.or(under.style),
            limit: self.limit.or(under.limit),
            line_height: self.line_height.or(under.line_height),
            align: self.align.or(under.align),
            baseline: self.baseline.or(under.baseline),
            angle: self.angle.or(under.angle),
        }
    }
}

/// A guide line: whether it is drawn, and how.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub show: Option<bool>,
    pub color: Option<Hsla>,
    pub opacity: Option<f32>,
    pub width: Option<f32>,
    /// Lengths of dash and gap, in turn.
    pub dash: Option<Vec<f32>>,
    pub dash_offset: Option<f32>,
    pub cap: Option<Cap>,
}

impl Line {
    /// Its properties over `under`'s.
    pub fn over(&self, under: &Line) -> Line {
        Line {
            show: self.show.or(under.show),
            color: self.color.or(under.color),
            opacity: self.opacity.or(under.opacity),
            width: self.width.or(under.width),
            dash: self.dash.clone().or_else(|| under.dash.clone()),
            dash_offset: self.dash_offset.or(under.dash_offset),
            cap: self.cap.or(under.cap),
        }
    }
}

/// How an axis draws each tick, its grid line and its label.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickStyle {
    pub tick: Line,
    pub size: Option<f32>,
    pub grid: Line,
    pub label: Lettering,
    /// Pixels a label moves along the run.
    pub label_offset: Option<f32>,
    /// Pixels between a tick and its label.
    pub label_padding: Option<f32>,
}

impl TickStyle {
    /// Its properties over `under`'s.
    pub fn over(&self, under: &TickStyle) -> TickStyle {
        TickStyle {
            tick: self.tick.over(&under.tick),
            size: self.size.or(under.size),
            grid: self.grid.over(&under.grid),
            label: self.label.over(&under.label),
            label_offset: self.label_offset.or(under.label_offset),
            label_padding: self.label_padding.or(under.label_padding),
        }
    }
}

/// A tick's style where `test` holds for it, over the axis's own.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub test: Test,
    pub style: TickStyle,
}

/// Where ticks and grid lines stand on a band scale: Vega's `tickBand`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickBand {
    Center,
    /// At the bands' edges, with one past the last band.
    Extent,
}

/// An x or y axis.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Axis {
    pub orient: Option<Side>,
    /// Pixels out from the plot's edge.
    pub offset: Option<f32>,
    /// Pixels the axis moves along its run.
    pub position: Option<f32>,
    /// Pixels the axis moves both ways, over Vega's 0.5.
    pub translate: Option<f32>,
    /// The least and most room, in pixels, its ticks and labels take from
    /// the plot's edge out.
    pub min_extent: Option<f32>,
    pub max_extent: Option<f32>,
    /// Where in its band a tick, grid line and label stands, 0 to 1.
    pub band_position: Option<f32>,
    /// Above 0, drawn over the marks.
    pub zindex: Option<f32>,
    pub domain: Line,
    /// Each tick's own, where no case of `cases` holds for it.
    pub style: TickStyle,
    /// The first that holds for a tick sets each property it sets.
    pub cases: Vec<Case>,
    /// Pixels ticks and grid lines move along the run.
    pub tick_offset: Option<f32>,
    /// Whether ticks and grid lines stand on whole pixels.
    pub tick_round: Option<bool>,
    pub tick_band: Option<TickBand>,
    /// Whether one more tick stands at the first band's start.
    pub tick_extra: Option<bool>,
    pub tick_count: Option<TickCount>,
    /// The least step between ticks, in the scale's units.
    pub tick_min_step: Option<f64>,
    /// The values ticked, in place of the scale's.
    pub values: Option<Vec<Datum>>,
    /// Whether labels are drawn.
    pub labels: Option<bool>,
    pub format: Option<Format>,
    pub relabel: Option<Relabel>,
    /// Hides a label past either end of the run by more than this many
    /// pixels.
    pub label_bound: Option<f32>,
    /// Aligns a label within this many pixels of either end of the run
    /// inside it. `Some(None)` turns it off.
    pub label_flush: Option<Option<f32>>,
    /// Pixels a flushed label moves out past its end.
    pub label_flush_offset: Option<f32>,
    pub label_overlap: Option<Overlap>,
    /// The least room, in pixels, between labels before they overlap.
    pub label_separation: Option<f32>,
    pub title: Lettering,
    pub title_anchor: Option<Anchor>,
    /// Pixels between the labels and the title.
    pub title_padding: Option<f32>,
    /// Where the title stands, in pixels from the axis's origin.
    pub title_x: Option<f32>,
    pub title_y: Option<f32>,
}

/// Which corner or side of the plot a legend stands at, or `None` for its
/// own `legend_x` and `legend_y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendOrient {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    None,
}

/// A symbol legend lists entries; a gradient one draws a continuous colour
/// scale's ramp, or a discretizing one's steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegendKind {
    Symbol,
    Gradient,
}

/// How a symbol legend's columns line up: each column as wide as its own
/// widest entry, every column as wide as the widest, or not at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridAlign {
    Each,
    All,
    None,
}

/// A symbol legend's symbols.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Symbols {
    pub shape: Option<Shape>,
    /// In square pixels.
    pub size: Option<f32>,
    pub stroke_width: Option<f32>,
    pub fill: Option<Hsla>,
    pub stroke: Option<Hsla>,
    pub opacity: Option<f32>,
    pub dash: Option<Vec<f32>>,
    pub dash_offset: Option<f32>,
    /// Pixels a symbol and its label move out from the legend's edge.
    pub offset: Option<f32>,
    /// The most entries listed; past it, the last says how many more.
    pub limit: Option<usize>,
}

/// A gradient legend's ramp.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ramp {
    /// Along its direction, in pixels.
    pub length: Option<f32>,
    /// Across, in pixels.
    pub thickness: Option<f32>,
    pub stroke: Option<Hsla>,
    pub stroke_width: Option<f32>,
    pub opacity: Option<f32>,
}

/// A colour, size, shape, opacity, stroke width or dash channel's legend.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Legend {
    pub kind: Option<LegendKind>,
    pub orient: Option<LegendOrient>,
    /// Where a legend of orient `None` stands, in pixels from the plot's
    /// top-left corner.
    pub legend_x: Option<f32>,
    pub legend_y: Option<f32>,
    /// Which way entries flow, or the ramp runs.
    pub direction: Option<Orient>,
    /// Pixels between the legend and the plot, or its axes.
    pub offset: Option<f32>,
    /// Pixels inside the legend's box.
    pub padding: Option<f32>,
    pub corner_radius: Option<f32>,
    /// The legend's box.
    pub fill: Option<Hsla>,
    pub stroke: Option<Hsla>,
    /// Entries to a row, `0` for all in one.
    pub columns: Option<usize>,
    pub column_padding: Option<f32>,
    pub row_padding: Option<f32>,
    pub grid_align: Option<GridAlign>,
    /// Each entry's height, its symbol cut off to it.
    pub clip_height: Option<f32>,
    pub tick_count: Option<TickCount>,
    pub tick_min_step: Option<f64>,
    /// The values listed, in place of the scale's.
    pub values: Option<Vec<Datum>>,
    pub format: Option<Format>,
    pub relabel: Option<Relabel>,
    pub symbols: Symbols,
    pub ramp: Ramp,
    pub label: Lettering,
    /// Pixels between a symbol or ramp and its label.
    pub label_offset: Option<f32>,
    pub label_overlap: Option<Overlap>,
    pub label_separation: Option<f32>,
    pub title: Lettering,
    pub title_anchor: Option<Anchor>,
    /// Which side of the entries the title stands on.
    pub title_orient: Option<Side>,
    pub title_padding: Option<f32>,
    pub zindex: Option<f32>,
}

/// Which box a chart's title is anchored in: the plot alone, or everything
/// drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Group,
    Bounds,
}

/// The chart's title, and its subtitle under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    /// Lines split at `\n`.
    pub text: SharedString,
    pub subtitle: Option<SharedString>,
    pub anchor: Option<Anchor>,
    pub orient: Option<Side>,
    /// Pixels between the title and what it stands beside.
    pub offset: Option<f32>,
    pub frame: Option<Frame>,
    /// Pixels the title moves right and down.
    pub dx: f32,
    pub dy: f32,
    pub lettering: Lettering,
    pub subtitle_lettering: Lettering,
    /// Pixels between the title and its subtitle.
    pub subtitle_padding: Option<f32>,
    pub zindex: Option<f32>,
}

impl Title {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            subtitle: None,
            anchor: None,
            orient: None,
            offset: None,
            frame: None,
            dx: 0.0,
            dy: 0.0,
            lettering: Lettering::default(),
            subtitle_lettering: Lettering::default(),
            subtitle_padding: None,
            zindex: None,
        }
    }
}

impl From<&str> for Title {
    fn from(text: &str) -> Self {
        Self::new(text.to_string())
    }
}

impl From<String> for Title {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<SharedString> for Title {
    fn from(text: SharedString) -> Self {
        Self::new(text)
    }
}
