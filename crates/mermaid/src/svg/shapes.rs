//! Node shapes by Mermaid's names: how big each is around its label, how it
//! is drawn, and where edges meet its outline. Pele's `svg/shapes.ts`.

use std::{
    f64::consts::{FRAC_1_SQRT_2, PI},
    fmt::{self, Write as _},
};

use super::{
    builder::{fragment, num, rect},
    theme::RADIUS,
};
use crate::{
    geom::{Point, Side, Size},
    layout::compound::CNode,
};

pub const PAD_X: f64 = 18.0;
pub const PAD_Y: f64 = 12.0;

/// Kept clear at each end of a straight side, for the corner.
const CORNER: f64 = 12.0;

/// A node shape, by Mermaid's name for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    /// `rect`
    Rect,
    /// `rounded`
    Rounded,
    /// `stadium`
    Stadium,
    /// `fr-rect`
    FramedRect,
    /// `cyl`
    Cylinder,
    /// `circle`
    Circle,
    /// `dbl-circ`
    DoubleCircle,
    /// `ellipse`
    Ellipse,
    /// `diam`
    Diamond,
    /// `hex`
    Hexagon,
    /// `odd`
    Odd,
    /// `lean-r`
    LeanRight,
    /// `lean-l`
    LeanLeft,
    /// `trap-b`
    TrapezoidBottom,
    /// `trap-t`
    TrapezoidTop,
    /// `text`
    Text,
    /// `sm-circ`
    SmallCircle,
    /// `f-circ`
    FilledCircle,
    /// `fr-circ`
    FramedCircle,
    /// `fork`
    Fork,
    /// `notch-rect`
    NotchedRect,
    /// `lin-rect`
    LinedRect,
    /// `div-rect`
    DividedRect,
    /// `win-pane`
    WindowPane,
    /// `tri`
    Triangle,
    /// `flip-tri`
    FlippedTriangle,
    /// `sl-rect`
    SlopedRect,
    /// `notch-pent`
    NotchedPentagon,
    /// `hourglass`
    Hourglass,
    /// `delay`
    Delay,
    /// `h-cyl`
    HorizontalCylinder,
    /// `cross-circ`
    CrossedCircle,
    /// `st-rect`
    StackedRect,
    /// `doc`
    Document,
    /// `flag`
    Flag,
    /// `datastore`
    Datastore,
    /// `folder`
    Folder,
    /// `bucket`
    Bucket,
    /// `console`
    Console,
    /// `browser`
    Browser,
    /// `person`
    Person,
    /// `bang`
    Bang,
    /// `cloud`
    Cloud,
    /// `brace`
    BraceLeft,
    /// `brace-r`
    BraceRight,
    /// `braces`
    Braces,
    /// `bolt`
    Bolt,
    /// `lin-cyl`
    LinedCylinder,
    /// `curv-trap`
    CurvedTrapezoid,
    /// `docs`
    Documents,
    /// `bow-rect`
    BowRect,
    /// `tag-rect`
    TaggedRect,
    /// `tag-doc`
    TaggedDocument,
    /// `lin-doc`
    LinedDocument,
    /// `collapsedGroup`
    CollapsedGroup,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSize {
    pub size: Size,
    /// Vertical shift of the label from the shape's centre.
    pub dy: f64,
}

fn sized(w: f64, h: f64, dy: f64) -> ShapeSize {
    ShapeSize {
        size: Size::new(w, h),
        dy,
    }
}

type Sizer = fn(f64, f64) -> ShapeSize;
type Drawer = fn(&mut String, f64, f64, &str, &str);
/// Distance from the bounding box to the outline on a side. `t` is how far
/// along the side from its middle, to the right or down.
type Inset = fn(f64, f64, Side, f64) -> f64;
/// How much of a side edges may spread along, centred on its middle: the top
/// and bottom, or the left and right when `sideways`.
type Span = fn(f64, f64, bool) -> f64;

struct ShapeDef {
    size: Sizer,
    draw: Drawer,
    inset: Option<Inset>,
    span: Option<Span>,
    no_label: bool,
}

const fn def(size: Sizer, draw: Drawer) -> ShapeDef {
    ShapeDef {
        size,
        draw,
        inset: None,
        span: None,
        no_label: false,
    }
}

impl ShapeDef {
    const fn inset(mut self, inset: Inset) -> Self {
        self.inset = Some(inset);
        self
    }

    const fn span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    const fn no_label(mut self) -> Self {
        self.no_label = true;
        self
    }
}

fn flat(w: f64, h: f64, sideways: bool) -> f64 {
    (if sideways { h } else { w }) - 2.0 * CORNER
}

/// Straight along the top and bottom except for `cut(h)` at each end, and
/// not straight at the sides.
fn flat_across(w: f64, _h: f64, sideways: bool, cut: f64) -> f64 {
    if sideways {
        0.0
    } else {
        w - 2.0 * cut - 2.0 * CORNER
    }
}

/// How far an ellipse's outline has fallen away from its bounding box, `t`
/// from the middle of a side `half` long.
fn bulge(t: f64, half: f64, depth: f64) -> f64 {
    let k = (t.abs() / half).min(1.0);
    depth * (1.0 - (1.0 - k * k).sqrt())
}

fn round_inset(w: f64, h: f64, side: Side, t: f64) -> f64 {
    if !side.is_horizontal() {
        bulge(t, h / 2.0, w / 2.0)
    } else {
        bulge(t, w / 2.0, h / 2.0)
    }
}

fn round_span(w: f64, h: f64, sideways: bool) -> f64 {
    (if sideways { h } else { w }) * 0.6
}

fn cap_inset(w: f64, _h: f64, side: Side, t: f64) -> f64 {
    if !side.is_horizontal() {
        0.0
    } else {
        bulge(t, w / 2.0, 12f64.min(6f64.max(w / 14.0)))
    }
}

fn tri_size(tw: f64, th: f64, flip: bool) -> ShapeSize {
    let w = (1.6 * tw + 40.0).max(80.0);
    let h = (th + 10.0) / (1.0 - (tw + 8.0) / w);
    let dy = h / 2.0 - 10.0 - th / 2.0;
    sized(w, h, if flip { -dy } else { dy })
}

fn path(d: impl fmt::Display, a: impl fmt::Display) -> impl fmt::Display {
    fragment(move |f| write!(f, "<path d=\"{d}\"{a}/>"))
}

fn lean_inset(_w: f64, h: f64, side: Side, _t: f64) -> f64 {
    if !side.is_horizontal() { h / 8.0 } else { 0.0 }
}

fn box_size(tw: f64, th: f64) -> ShapeSize {
    sized(
        (tw + 2.0 * PAD_X).max(2.0 * PAD_X + 12.0),
        th + 2.0 * PAD_Y,
        0.0,
    )
}

/// A `<polygon>` through `points`, with `a` after its points.
pub fn poly<'a>(points: &'a [Point], a: impl fmt::Display + 'a) -> impl fmt::Display + 'a {
    fragment(move |f| {
        f.write_str("<polygon points=\"")?;
        for (i, point) in points.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{point}")?;
        }
        write!(f, "\"{a}/>")
    })
}

fn rounded() -> impl fmt::Display {
    fragment(|f| write!(f, " rx=\"{RADIUS}\""))
}

fn padded(h: f64) -> f64 {
    h + 2.0 * PAD_Y
}

fn brace(x: f64, h: f64, dir: f64, line: &str) -> impl fmt::Display + '_ {
    let y = h / 2.0;
    let k = 6.0 * dir;
    fragment(move |f| {
        write!(
            f,
            "<path d=\"M{},{}q{},0 {},6V-6q0,6 {},6q{},0 {},6V{}q0,6 {},6\" fill=\"none\"{line}/>",
            num(x),
            num(-y),
            num(k),
            num(k),
            num(k),
            num(-k),
            num(-k),
            num(y - 6.0),
            num(-k)
        )
    })
}

fn wave(w: f64, h: f64, a: impl fmt::Display) -> impl fmt::Display {
    let x = w / 2.0;
    let top = -h / 2.0;
    let base = h / 2.0 - 10.0;
    let q = w / 4.0;
    fragment(move |f| {
        write!(
            f,
            "<path d=\"M{},{}H{}V{}q{},-12 {},0t{},0Z\"{a}/>",
            num(-x),
            num(top),
            num(x),
            num(base),
            num(-q),
            num(-2.0 * q),
            num(-2.0 * q)
        )
    })
}

impl Shape {
    /// The shape Mermaid's `name` names, or the rectangle for a name it does
    /// not have.
    pub fn named(name: &str) -> Self {
        Self::from_name(name).unwrap_or_default()
    }

    /// The shape Mermaid's `name` names, if any.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "rect" => Shape::Rect,
            "rounded" => Shape::Rounded,
            "stadium" => Shape::Stadium,
            "fr-rect" => Shape::FramedRect,
            "cyl" => Shape::Cylinder,
            "circle" => Shape::Circle,
            "dbl-circ" => Shape::DoubleCircle,
            "ellipse" => Shape::Ellipse,
            "diam" => Shape::Diamond,
            "hex" => Shape::Hexagon,
            "odd" => Shape::Odd,
            "lean-r" => Shape::LeanRight,
            "lean-l" => Shape::LeanLeft,
            "trap-b" => Shape::TrapezoidBottom,
            "trap-t" => Shape::TrapezoidTop,
            "text" => Shape::Text,
            "sm-circ" => Shape::SmallCircle,
            "f-circ" => Shape::FilledCircle,
            "fr-circ" => Shape::FramedCircle,
            "fork" => Shape::Fork,
            "notch-rect" => Shape::NotchedRect,
            "lin-rect" => Shape::LinedRect,
            "div-rect" => Shape::DividedRect,
            "win-pane" => Shape::WindowPane,
            "tri" => Shape::Triangle,
            "flip-tri" => Shape::FlippedTriangle,
            "sl-rect" => Shape::SlopedRect,
            "notch-pent" => Shape::NotchedPentagon,
            "hourglass" => Shape::Hourglass,
            "delay" => Shape::Delay,
            "h-cyl" => Shape::HorizontalCylinder,
            "cross-circ" => Shape::CrossedCircle,
            "st-rect" => Shape::StackedRect,
            "doc" => Shape::Document,
            "flag" => Shape::Flag,
            "datastore" => Shape::Datastore,
            "folder" => Shape::Folder,
            "bucket" => Shape::Bucket,
            "console" => Shape::Console,
            "browser" => Shape::Browser,
            "person" => Shape::Person,
            "bang" => Shape::Bang,
            "cloud" => Shape::Cloud,
            "brace" => Shape::BraceLeft,
            "brace-r" => Shape::BraceRight,
            "braces" => Shape::Braces,
            "bolt" => Shape::Bolt,
            "lin-cyl" => Shape::LinedCylinder,
            "curv-trap" => Shape::CurvedTrapezoid,
            "docs" => Shape::Documents,
            "bow-rect" => Shape::BowRect,
            "tag-rect" => Shape::TaggedRect,
            "tag-doc" => Shape::TaggedDocument,
            "lin-doc" => Shape::LinedDocument,
            "collapsedGroup" => Shape::CollapsedGroup,
            _ => return None,
        })
    }

    fn def(self) -> ShapeDef {
        match self {
            Shape::Rect => def(box_size, |out, w, h, a, _| { let _ = write!(out, "{}", centered_rect(w, h, a, rounded())); }).span(flat),
            Shape::Rounded => def(box_size, |out, w, h, a, _| {
let _ = write!(out, "{}", centered_rect(w, h, a, format_args!(" rx=\"{}\"", num(12f64.min(h / 2.0)))));
})
        .span(flat),
            Shape::Stadium => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + h * 0.5 + 2.0 * PAD_X - 8.0, h, 0.0)
            },
            |out, w, h, a, _| { let _ = write!(out, "{}", centered_rect(w, h, a, format_args!(" rx=\"{}\"", num(h / 2.0)))); })
        .inset(|_w, h, side, t| {
            if !side.is_horizontal() {
                bulge(t, h / 2.0, h / 2.0)
            } else {
                0.0
            }
        })
        .span(|w, h, sideways| if sideways { h * 0.6 } else { w - h }),
            Shape::FramedRect => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 16.0, padded(th), 0.0),
            |out, w, h, a, line| {
                let x = w / 2.0 - 8.0;
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                        "<path d=\"M{},{}V{}M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-x),
                        num(-h / 2.0),
                        num(h / 2.0),
                        num(x),
                        num(-h / 2.0),
                        num(h / 2.0)
                    ));
})
        .span(flat),
            Shape::Cylinder => def(
            |tw, th| {
                let w = (tw + 2.0 * PAD_X).max(56.0);
                let ry = 12f64.min(6f64.max(w / 14.0));
                sized(w, th + 2.0 * PAD_Y + 2.0 * ry, ry / 2.0)
            },
            |out, w, h, a, line| {
                let rx = w / 2.0;
                let ry = 12f64.min(6f64.max(w / 14.0));
                let top = -h / 2.0 + ry;
                let bottom = h / 2.0 - ry;
let _ = write!(out, "{}", format_args!(
                    "<path d=\"M{},{}A{},{} 0 0 1 {},{}V{}A{},{} 0 0 1 {},{}Z\"{a}/><path d=\"M{},{}A{},{} 0 0 0 {},{}\" fill=\"none\"{line}/>",
                    num(-rx),
                    num(top),
                    num(rx),
                    num(ry),
                    num(rx),
                    num(top),
                    num(bottom),
                    num(rx),
                    num(ry),
                    num(-rx),
                    num(bottom),
                    num(-rx),
                    num(top),
                    num(rx),
                    num(ry),
                    num(rx),
                    num(top)
                ));
})
        .inset(cap_inset)
        .span(|w, h, sideways| {
            if sideways {
                h - 2.0 * CORNER - 12.0
            } else {
                w * 0.6
            }
        }),
            Shape::Circle => def(
            |tw, th| {
                let d = ((tw * tw + th * th).sqrt() + 16.0).max(44.0);
                sized(d, d, 0.0)
            },
            |out, w, _h, a, _| { let _ = write!(out, "{}", format_args!("<circle r=\"{}\"{a}/>", num(w / 2.0))); })
        .inset(round_inset)
        .span(round_span),
            Shape::DoubleCircle => def(
            |tw, th| {
                let d = ((tw * tw + th * th).sqrt() + 26.0).max(54.0);
                sized(d, d, 0.0)
            },
            |out, w, _h, a, line| {
let _ = write!(out, "{}", format_args!(
                    "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"none\"{line}/>",
                    num(w / 2.0),
                    num(w / 2.0 - 5.0)
                ));
})
        .inset(round_inset)
        .span(round_span),
            Shape::Ellipse => def(
            |tw, th| sized(tw * 1.3 + 2.0 * PAD_X, th * 1.2 + 2.0 * PAD_Y, 0.0),
            |out, w, h, a, _| {
let _ = write!(out, "{}", format_args!(
                    "<ellipse rx=\"{}\" ry=\"{}\"{a}/>",
                    num(w / 2.0),
                    num(h / 2.0)
                ));
})
        .inset(round_inset)
        .span(round_span),
            Shape::Diamond => def(
            |tw, th| {
                let a = tw + 16.0;
                let b = th + 8.0;
                sized(a + 1.6 * b, a / 1.6 + b, 0.0)
            },
            |out, w, h, a, _| { let _ = write!(out, "{}", poly(&[Point::new(0.0, -h / 2.0), Point::new(w / 2.0, 0.0), Point::new(0.0, h / 2.0), Point::new(-w / 2.0, 0.0)], a)); })
        .inset(|w, h, side, t| {
            if !side.is_horizontal() {
                t.abs() * w / h
            } else {
                t.abs() * h / w
            }
        })
        .span(|w, h, sideways| (if sideways { h } else { w }) * 0.6),
            Shape::Hexagon => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + 2.0 * PAD_X + h / 2.0, h, 0.0)
            },
            |out, w, h, a, _| {
                let m = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0 + m, -h / 2.0), Point::new(w / 2.0 - m, -h / 2.0), Point::new(w / 2.0, 0.0), Point::new(w / 2.0 - m, h / 2.0), Point::new(-w / 2.0 + m, h / 2.0), Point::new(-w / 2.0, 0.0)], a));
})
        .inset(|_w, _h, side, t| if !side.is_horizontal() { t.abs() / 2.0 } else { 0.0 })
        .span(|w, h, sideways| {
            if sideways {
                h * 0.5
            } else {
                w - h / 2.0 - CORNER
            }
        }),
            Shape::Odd => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + 2.0 * PAD_X + h / 4.0, h, 0.0)
            },
            |out, w, h, a, _| {
                let notch = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0), Point::new(-w / 2.0 + notch, 0.0)], a));
})
        .inset(|_w, h, side, _t| if side == Side::Left { h / 4.0 } else { 0.0 })
        .span(|w, h, sideways| flat_across(w, h, sideways, 0.0)),
            Shape::LeanRight => def(lean_size, |out, w, h, a, _| {
            let s = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0 + s, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(w / 2.0 - s, h / 2.0), Point::new(-w / 2.0, h / 2.0)], a));
})
        .inset(lean_inset)
        .span(quarter_across),
            Shape::LeanLeft => def(lean_size, |out, w, h, a, _| {
            let s = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0), Point::new(w / 2.0 - s, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0 + s, h / 2.0)], a));
})
        .inset(lean_inset)
        .span(quarter_across),
            Shape::TrapezoidBottom => def(lean_size, |out, w, h, a, _| {
            let s = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0 + s, -h / 2.0), Point::new(w / 2.0 - s, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0)], a));
})
        .inset(lean_inset)
        .span(quarter_across),
            Shape::TrapezoidTop => def(lean_size, |out, w, h, a, _| {
            let s = h / 4.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(w / 2.0 - s, h / 2.0), Point::new(-w / 2.0 + s, h / 2.0)], a));
})
        .inset(lean_inset)
        .span(quarter_across),
            Shape::Text => def(
            |tw, th| sized(tw + 8.0, th + 8.0, 0.0),
            |out, w, h, _, _| { let _ = write!(out, "{}", centered_rect(w, h, " fill=\"none\" stroke=\"none\"", "")); })
        .span(flat),
            Shape::SmallCircle => def(fixed14, |out, w, _h, a, _| {
let _ = write!(out, "{}", format_args!("<circle r=\"{}\"{a}/>", num(w / 2.0)));
})
        .no_label()
        .inset(round_inset)
        .span(round_span),
            Shape::FilledCircle => def(fixed14, |out, w, _h, _a, line| {
let _ = write!(out, "{}", format_args!("<circle r=\"{}\" fill=\"var(--_l)\"{line}/>", num(w / 2.0)));
})
        .no_label()
        .inset(round_inset)
        .span(round_span),
            Shape::FramedCircle => def(
            |_, _| sized(18.0, 18.0, 0.0),
            |out, w, _h, a, line| {
let _ = write!(out, "{}", format_args!(
                    "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"var(--_l)\"{line}/>",
                    num(w / 2.0),
                    num(w / 2.0 - 4.0)
                ));
})
        .no_label()
        .inset(round_inset)
        .span(round_span),
            Shape::Fork => def(
            |_, _| sized(72.0, 8.0, 0.0),
            |out, w, h, _a, line| { let _ = write!(out, "{}", centered_rect(w, h, format_args!(" fill=\"var(--_l)\"{line}"), " rx=\"2\"")); })
        .no_label()
        .span(|w, h, sideways| (if sideways { h } else { w }) - 8.0),
            Shape::NotchedRect => def(box_size, |out, w, h, a, _| {
            let c = 10.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0 + c, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0), Point::new(-w / 2.0, -h / 2.0 + c)], a));
})
        .span(flat),
            Shape::LinedRect => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, padded(th), 0.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                        "<path d=\"M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0)
                    ));
})
        .span(flat),
            Shape::DividedRect => def(
            |tw, th| sized(tw + 2.0 * PAD_X, th + 2.0 * PAD_Y + 8.0, 4.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                        "<path d=\"M{},{}H{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0),
                        num(-h / 2.0 + 8.0),
                        num(w / 2.0)
                    ));
})
        .span(flat),
            Shape::WindowPane => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, th + 2.0 * PAD_Y + 8.0, 4.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                        "<path d=\"M{},{}V{}M{},{}H{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0),
                        num(-w / 2.0),
                        num(-h / 2.0 + 8.0),
                        num(w / 2.0)
                    ));
})
        .span(flat),
            Shape::Triangle => def(
            |tw, th| tri_size(tw, th, false),
            |out, w, h, a, _| { let _ = write!(out, "{}", poly(&[Point::new(0.0, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0)], a)); })
        .inset(|w, _h, side, _t| if !side.is_horizontal() { w / 4.0 } else { 0.0 }),
            Shape::FlippedTriangle => def(
            |tw, th| tri_size(tw, th, true),
            |out, w, h, a, _| { let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(0.0, h / 2.0)], a)); })
        .inset(|w, _h, side, _t| if !side.is_horizontal() { w / 4.0 } else { 0.0 }),
            Shape::SlopedRect => def(
            |tw, th| sized(tw + 2.0 * PAD_X, th + 2.0 * PAD_Y + 10.0, 5.0),
            |out, w, h, a, _| {
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0 + 10.0), Point::new(w / 2.0, -h / 2.0), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0)], a));
})
        .inset(|_w, _h, side, _t| if side == Side::Top { 5.0 } else { 0.0 }),
            Shape::NotchedPentagon => def(
            |tw, th| sized(tw + 2.0 * PAD_X, padded(th), 0.0),
            |out, w, h, a, _| {
                let c = 10.0;
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0 + c, -h / 2.0), Point::new(w / 2.0 - c, -h / 2.0), Point::new(w / 2.0, -h / 2.0 + c), Point::new(w / 2.0, h / 2.0), Point::new(-w / 2.0, h / 2.0), Point::new(-w / 2.0, -h / 2.0 + c)], a));
})
        .span(flat),
            Shape::Hourglass => def(
            |_, _| sized(32.0, 40.0, 0.0),
            |out, w, h, a, _| {
let _ = write!(out, "{}", poly(&[Point::new(-w / 2.0, -h / 2.0), Point::new(w / 2.0, -h / 2.0), Point::new(-w / 2.0, h / 2.0), Point::new(w / 2.0, h / 2.0)], a));
})
        .no_label()
        .inset(|w, _h, side, _t| if !side.is_horizontal() { w / 2.0 } else { 0.0 }),
            Shape::Delay => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + 2.0 * PAD_X + h / 4.0, h, 0.0)
            },
            |out, w, h, a, _| {
                let r = h / 2.0;
let _ = write!(out, "{}", format_args!(
                    "<path d=\"M{},{}H{}A{},{} 0 0 1 {},{}H{}Z\"{a}/>",
                    num(-w / 2.0),
                    num(-r),
                    num(w / 2.0 - r),
                    num(r),
                    num(r),
                    num(w / 2.0 - r),
                    num(r),
                    num(-w / 2.0)
                ));
}),
            Shape::HorizontalCylinder => def(
            |tw, th| {
                let h = padded(th);
                let rx = 12f64.min(h / 4.0);
                sized(tw + 2.0 * PAD_X + 2.0 * rx, h, 0.0)
            },
            |out, w, h, a, line| {
                let ry = h / 2.0;
                let rx = 12f64.min(h / 4.0);
                let l = -w / 2.0 + rx;
                let r = w / 2.0 - rx;
let _ = write!(out, "{}", format_args!(
                    "<path d=\"M{},{}H{}A{},{} 0 0 1 {},{}H{}A{},{} 0 0 1 {},{}Z\"{a}/><path d=\"M{},{}A{},{} 0 0 0 {},{}\" fill=\"none\"{line}/>",
                    num(l),
                    num(-ry),
                    num(r),
                    num(rx),
                    num(ry),
                    num(r),
                    num(ry),
                    num(l),
                    num(rx),
                    num(ry),
                    num(l),
                    num(-ry),
                    num(r),
                    num(-ry),
                    num(rx),
                    num(ry),
                    num(r),
                    num(ry)
                ));
}),
            Shape::CrossedCircle => def(
            |_, _| sized(44.0, 44.0, 0.0),
            |out, w, _h, a, line| {
                let r = w / 2.0;
                let k = r * FRAC_1_SQRT_2;
let _ = write!(out, "{}", format_args!(
                    "<circle r=\"{}\"{a}/><path d=\"M{},{}L{},{}M{},{}L{},{}\" fill=\"none\"{line}/>",
                    num(r),
                    num(-k),
                    num(-k),
                    num(k),
                    num(k),
                    num(k),
                    num(-k),
                    num(-k),
                    num(k)
                ));
})
        .no_label()
        .inset(round_inset)
        .span(round_span),
            Shape::StackedRect => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, th + 2.0 * PAD_Y + 8.0, 4.0),
            |out, w, h, a, _| {
                let bw = w - 8.0;
                let bh = h - 8.0;
                for i in 0..3 {
                    let x = -w / 2.0 + f64::from(2 - i) * 4.0;
                    let y = -h / 2.0 + f64::from(i) * 4.0;
                    let _ = write!(out,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{a}/>",
                        num(x),
                        num(y),
                        num(bw),
                        num(bh),
                        rounded()
                    );
                }
            })
        .span(|w, h, sideways| (if sideways { h } else { w }) - 2.0 * CORNER - 8.0),
            Shape::Document => def(
            |tw, th| sized(tw + 2.0 * PAD_X, th + 2.0 * PAD_Y + 10.0, -5.0),
            |out, w, h, a, _| { let _ = write!(out, "{}", wave(w, h, a)); })
        .inset(|_w, _h, side, _t| if side == Side::Bottom { 10.0 } else { 0.0 }),
            Shape::Flag => def(
            |tw, th| sized(tw + 2.0 * PAD_X, th + 2.0 * PAD_Y + 16.0, 0.0),
            |out, w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0 - 8.0;
                let q = w / 4.0;
let _ = write!(out, "{}", format_args!(
                    "<path d=\"M{},{}q{},-10 {},0t{},0V{}q{},10 {},0t{},0Z\"{a}/>",
                    num(-x),
                    num(-y),
                    num(q),
                    num(2.0 * q),
                    num(2.0 * q),
                    num(y),
                    num(-q),
                    num(-2.0 * q),
                    num(-2.0 * q)
                ));
})
        .inset(|_w, _h, side, _t| if !side.is_horizontal() { 0.0 } else { 8.0 }),
            Shape::Datastore => def(box_size, |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(
                w,
                h,
                &a.replacen("stroke=\"var(--_b)\"", "stroke=\"none\"", 1),
                "",
            ), format_args!(
                "<path d=\"M{},{}H{}M{},{}H{}\" fill=\"none\"{line}/>",
                num(-w / 2.0),
                num(-h / 2.0),
                num(w / 2.0),
                num(-w / 2.0),
                num(h / 2.0),
                num(w / 2.0)
            ));
})
        .span(flat),
            Shape::Folder => def(
            |tw, th| sized((tw + 2.0 * PAD_X).max(64.0), th + 2.0 * PAD_Y + 8.0, 4.0),
            |out, w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let tab = (w * 0.4).min(48.0);
let _ = write!(out, "{}", path(
                    format_args!(
                        "M{},{}H{}L{},{}H{}V{}H{}Z",
                        num(-x),
                        num(-y),
                        num(-x + tab),
                        num(-x + tab + 8.0),
                        num(-y + 8.0),
                        num(x),
                        num(y),
                        num(-x)
                    ),
                    a,
                ));
}),
            Shape::Bucket => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 20.0, th + 2.0 * PAD_Y + 10.0, 3.0),
            |out, w, h, a, line| {
                let x = w / 2.0;
                let y = h / 2.0;
                let ry = 5.0;
                let taper = h / 6.0;
let _ = write!(out, "{}{}", path(
                    format_args!(
                        "M{},{}A{},{} 0 0 1 {},{}L{},{}A{},{} 0 0 1 {},{}Z",
                        num(-x),
                        num(-y + ry),
                        num(x),
                        num(ry),
                        num(x),
                        num(-y + ry),
                        num(x - taper),
                        num(y - ry),
                        num(x - taper),
                        num(ry),
                        num(-x + taper),
                        num(y - ry)
                    ),
                    a,
                ), format_args!(
                    "<path d=\"M{},{}A{},{} 0 0 0 {},{}\" fill=\"none\"{line}/>",
                    num(-x),
                    num(-y + ry),
                    num(x),
                    num(ry),
                    num(x),
                    num(-y + ry)
                ));
})
        .inset(|_w, h, side, _t| if !side.is_horizontal() { h / 12.0 } else { 0.0 }),
            Shape::Console => def(
            |tw, th| sized((tw + 2.0 * PAD_X).max(72.0), th + 2.0 * PAD_Y + 12.0, 6.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                        "<path d=\"M{},{}H{}M{},{}l3,2.5l-3,2.5M{},{}h4\" fill=\"none\"{line}/>",
                        num(-w / 2.0),
                        num(-h / 2.0 + 12.0),
                        num(w / 2.0),
                        num(-w / 2.0 + 7.0),
                        num(-h / 2.0 + 4.0),
                        num(-w / 2.0 + 13.0),
                        num(-h / 2.0 + 9.0)
                    ));
})
        .span(flat),
            Shape::Browser => def(
            |tw, th| sized((tw + 2.0 * PAD_X).max(72.0), th + 2.0 * PAD_Y + 12.0, 6.0),
            |out, w, h, a, line| {
                let _ = write!(
                    out,
                    "{}<path d=\"M{},{}H{}\" fill=\"none\"{line}/>",
                    centered_rect(w, h, a, rounded()),
                    num(-w / 2.0),
                    num(-h / 2.0 + 12.0),
                    num(w / 2.0)
                );
                for i in 0..3 {
                    let _ = write!(out,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"1.5\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0 + f64::from(i) * 7.0),
                        num(-h / 2.0 + 6.0)
                    );
                }
            })
        .span(flat),
            Shape::Person => def(
            |tw, th| sized((tw + 2.0 * PAD_X).max(56.0), th + 2.0 * PAD_Y + 26.0, 13.0),
            |out, w, h, a, _| {
                let top = -h / 2.0;
let _ = write!(out, "{}", format_args!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\"{a}/><circle cy=\"{}\" r=\"13\"{a}/>",
                    num(-w / 2.0),
                    num(top + 26.0),
                    num(w),
                    num(h - 26.0),
                    num(top + 14.0)
                ));
}),
            Shape::Bang => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 28.0, th + 2.0 * PAD_Y + 24.0, 0.0),
            |out, w, h, a, _| {
                let spikes = 14.0;
                let mut pts = Vec::new();
                for i in 0..28 {
                    let i = f64::from(i);
                    let angle = PI * i / spikes - PI / 2.0;
                    let k = if i % 2.0 == 0.0 { 1.0 } else { 0.82 };
                    pts.push(Point::new(
                        angle.cos() * (w / 2.0) * k,
                        angle.sin() * (h / 2.0) * k,
                    ));
                }
                let _ = write!(out, "{}", poly(&pts, a));
            })
        .inset(|_, _, _, _| 3.0),
            Shape::Cloud => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 32.0, th + 2.0 * PAD_Y + 24.0, 0.0),
            |out, w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let r = y * 0.55;
let _ = write!(out, "{}", path(
                    format_args!(
                        "M{},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}A{},{} 0 0 1 {},{}Z",
                        num(-x + r),
                        num(y),
                        num(r),
                        num(r),
                        num(-x + r * 0.6),
                        num(-y * 0.1),
                        num(x * 0.42),
                        num(y * 0.7),
                        num(-x * 0.1),
                        num(-y * 0.72),
                        num(x * 0.4),
                        num(y * 0.62),
                        num(x * 0.62),
                        num(-y * 0.42),
                        num(r * 1.05),
                        num(r * 1.05),
                        num(x - r),
                        num(y)
                    ),
                    a,
                ));
})
        .inset(|_w, _h, side, _t| if !side.is_horizontal() { 2.0 } else { 1.0 }),
            Shape::BraceLeft => def(one_brace, |out, w, h, _a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, " fill=\"none\" stroke=\"none\"", ""), brace(-w / 2.0 + 12.0, h, -1.0, line));
}),
            Shape::BraceRight => def(one_brace, |out, w, h, _a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, " fill=\"none\" stroke=\"none\"", ""), brace(w / 2.0 - 12.0, h, 1.0, line));
}),
            Shape::Braces => def(two_braces, |out, w, h, _a, line| {
let _ = write!(out, "{}{}{}", centered_rect(w, h, " fill=\"none\" stroke=\"none\"", ""), brace(-w / 2.0 + 12.0, h, -1.0, line), brace(w / 2.0 - 12.0, h, 1.0, line));
}),
            Shape::Bolt => def(
            |_, _| sized(36.0, 56.0, 0.0),
            |out, w, h, a, _| {
let _ = write!(out, "{}", poly(&[Point::new(w * 0.15, -h / 2.0), Point::new(-w / 2.0, h * 0.08), Point::new(-w * 0.05, h * 0.08), Point::new(-w * 0.15, h / 2.0), Point::new(w / 2.0, -h * 0.08), Point::new(w * 0.05, -h * 0.08)], a));
})
        .no_label()
        .inset(|w, _h, side, _t| if !side.is_horizontal() { w / 4.0 } else { 0.0 }),
            Shape::LinedCylinder => def(
            |tw, th| {
                let w = (tw + 2.0 * PAD_X).max(56.0);
                let ry = 12f64.min(6f64.max(w / 14.0));
                sized(w, th + 2.0 * PAD_Y + 2.0 * ry + 8.0, ry / 2.0 + 4.0)
            },
            |out, w, h, a, line| {
                let rx = w / 2.0;
                let ry = 12f64.min(6f64.max(w / 14.0));
                let top = -h / 2.0 + ry;
                let bottom = h / 2.0 - ry;
                let arc = |y: f64| {
                    format!(
                        "M{},{}A{},{} 0 0 0 {},{}",
                        num(-rx),
                        num(y),
                        num(rx),
                        num(ry),
                        num(rx),
                        num(y)
                    )
                };
let _ = write!(out, "{}{}", path(
                    format_args!(
                        "M{},{}A{},{} 0 0 1 {},{}V{}A{},{} 0 0 1 {},{}Z",
                        num(-rx),
                        num(top),
                        num(rx),
                        num(ry),
                        num(rx),
                        num(top),
                        num(bottom),
                        num(rx),
                        num(ry),
                        num(-rx),
                        num(bottom)
                    ),
                    a,
                ), format_args!(
                    "<path d=\"{}{}\" fill=\"none\"{line}/>",
                    arc(top),
                    arc(top + 8.0)
                ));
})
        .inset(cap_inset)
        .span(|w, h, sideways| {
            if sideways {
                h - 2.0 * CORNER - 20.0
            } else {
                w * 0.6
            }
        }),
            Shape::CurvedTrapezoid => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + 2.0 * PAD_X + h * 0.6, h, 0.0)
            },
            |out, w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let k = h * 0.3;
let _ = write!(out, "{}", path(
                    format_args!(
                        "M{},{}H{}A{},{} 0 0 1 {},{}H{}L{},0Z",
                        num(-x + k),
                        num(-y),
                        num(x - k),
                        num(k),
                        num(y),
                        num(x - k),
                        num(y),
                        num(-x + k),
                        num(-x)
                    ),
                    a,
                ));
}),
            Shape::Documents => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, th + 2.0 * PAD_Y + 18.0, -1.0),
            |out, w, h, a, _| {
                for i in 0..3 {
                    let _ = write!(out,
                        "<g transform=\"translate({},{})\">{}</g>",
                        (1 - i) * 4,
                        (i - 1) * 4,
                        wave(w - 8.0, h - 8.0, a)
                    );
                }
            })
        .inset(|_w, _h, side, _t| if side == Side::Bottom { 10.0 } else { 0.0 }),
            Shape::BowRect => def(
            |tw, th| {
                let h = padded(th);
                sized(tw + 2.0 * PAD_X + h * 0.4, h, 0.0)
            },
            |out, w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let k = h * 0.2;
let _ = write!(out, "{}", path(
                    format_args!(
                        "M{},{}H{}A{},{} 0 0 0 {},{}H{}A{},{} 0 0 1 {},{}Z",
                        num(-x + k),
                        num(-y),
                        num(x),
                        num(k),
                        num(y),
                        num(x),
                        num(y),
                        num(-x + k),
                        num(k),
                        num(y),
                        num(-x + k),
                        num(-y)
                    ),
                    a,
                ));
})
        .inset(|_w, h, side, _t| if side == Side::Right { h * 0.2 } else { 0.0 }),
            Shape::TaggedRect => def(box_size, |out, w, h, a, line| {
let _ = write!(out, "{}{}", centered_rect(w, h, a, rounded()), format_args!(
                    "<path d=\"M{},{}L{},{}\" fill=\"none\"{line}/>",
                    num(w / 2.0 - 12.0),
                    num(h / 2.0),
                    num(w / 2.0),
                    num(h / 2.0 - 12.0)
                ));
})
        .span(flat),
            Shape::TaggedDocument => def(
            |tw, th| sized(tw + 2.0 * PAD_X, th + 2.0 * PAD_Y + 10.0, -5.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", wave(w, h, a), format_args!(
                        "<path d=\"M{},{}L{},{}\" fill=\"none\"{line}/>",
                        num(w / 2.0 - 12.0),
                        num(h / 2.0 - 14.0),
                        num(w / 2.0),
                        num(h / 2.0 - 26.0)
                    ));
})
        .inset(|_w, _h, side, _t| if side == Side::Bottom { 10.0 } else { 0.0 }),
            Shape::LinedDocument => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, th + 2.0 * PAD_Y + 10.0, -5.0),
            |out, w, h, a, line| {
let _ = write!(out, "{}{}", wave(w, h, a), format_args!(
                        "<path d=\"M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0 - 6.0)
                    ));
})
        .inset(|_w, _h, side, _t| if side == Side::Bottom { 10.0 } else { 0.0 }),
            Shape::CollapsedGroup => def(
            |tw, th| sized(tw + 2.0 * PAD_X + 8.0, th + 2.0 * PAD_Y + 8.0, -2.0),
            |out, w, h, a, _| {
let _ = write!(out, "{}", format_args!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{a}/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}{a}/>",
                    num(-w / 2.0 + 6.0),
                    num(-h / 2.0 + 8.0),
                    num(w - 12.0),
                    num(h - 8.0),
                    rounded(),
                    num(-w / 2.0),
                    num(-h / 2.0),
                    num(w),
                    num(h - 4.0),
                    rounded()
                ));
}),
        }
    }

    /// The size of the shape around a label of `label`.
    pub fn size(self, label: Size) -> ShapeSize {
        (self.def().size)(label.w, label.h)
    }

    /// The shape at `size` around the origin, appended to `out`.
    pub fn draw(self, out: &mut String, size: Size, a: &str, line: &str) {
        (self.def().draw)(out, size.w, size.h, a, line)
    }

    /// How far inside its bounding box of `size` the outline is on `side`,
    /// `t` along the side from its middle.
    pub fn inset(self, size: Size, side: Side, t: f64) -> f64 {
        self.def()
            .inset
            .map_or(0.0, |inset| inset(size.w, size.h, side, t))
    }

    /// How much of a side of the shape at `size` edges may spread along, when
    /// the flow runs down or, if `sideways`, across.
    pub fn span(self, size: Size, sideways: bool) -> f64 {
        0f64.max(
            self.def()
                .span
                .map_or(0.0, |span| span(size.w, size.h, sideways)),
        )
    }

    pub fn has_label(self) -> bool {
        !self.def().no_label
    }

    /// Mermaid's name for the shape.
    pub fn name(self) -> &'static str {
        match self {
            Shape::Rect => "rect",
            Shape::Rounded => "rounded",
            Shape::Stadium => "stadium",
            Shape::FramedRect => "fr-rect",
            Shape::Cylinder => "cyl",
            Shape::Circle => "circle",
            Shape::DoubleCircle => "dbl-circ",
            Shape::Ellipse => "ellipse",
            Shape::Diamond => "diam",
            Shape::Hexagon => "hex",
            Shape::Odd => "odd",
            Shape::LeanRight => "lean-r",
            Shape::LeanLeft => "lean-l",
            Shape::TrapezoidBottom => "trap-b",
            Shape::TrapezoidTop => "trap-t",
            Shape::Text => "text",
            Shape::SmallCircle => "sm-circ",
            Shape::FilledCircle => "f-circ",
            Shape::FramedCircle => "fr-circ",
            Shape::Fork => "fork",
            Shape::NotchedRect => "notch-rect",
            Shape::LinedRect => "lin-rect",
            Shape::DividedRect => "div-rect",
            Shape::WindowPane => "win-pane",
            Shape::Triangle => "tri",
            Shape::FlippedTriangle => "flip-tri",
            Shape::SlopedRect => "sl-rect",
            Shape::NotchedPentagon => "notch-pent",
            Shape::Hourglass => "hourglass",
            Shape::Delay => "delay",
            Shape::HorizontalCylinder => "h-cyl",
            Shape::CrossedCircle => "cross-circ",
            Shape::StackedRect => "st-rect",
            Shape::Document => "doc",
            Shape::Flag => "flag",
            Shape::Datastore => "datastore",
            Shape::Folder => "folder",
            Shape::Bucket => "bucket",
            Shape::Console => "console",
            Shape::Browser => "browser",
            Shape::Person => "person",
            Shape::Bang => "bang",
            Shape::Cloud => "cloud",
            Shape::BraceLeft => "brace",
            Shape::BraceRight => "brace-r",
            Shape::Braces => "braces",
            Shape::Bolt => "bolt",
            Shape::LinedCylinder => "lin-cyl",
            Shape::CurvedTrapezoid => "curv-trap",
            Shape::Documents => "docs",
            Shape::BowRect => "bow-rect",
            Shape::TaggedRect => "tag-rect",
            Shape::TaggedDocument => "tag-doc",
            Shape::LinedDocument => "lin-doc",
            Shape::CollapsedGroup => "collapsedGroup",
        }
    }
}

fn lean_size(tw: f64, th: f64) -> ShapeSize {
    let h = padded(th);
    sized(tw + 2.0 * PAD_X + h / 2.0, h, 0.0)
}

fn quarter_across(w: f64, h: f64, sideways: bool) -> f64 {
    flat_across(w, h, sideways, h / 4.0)
}

fn fixed14(_: f64, _: f64) -> ShapeSize {
    sized(14.0, 14.0, 0.0)
}

fn one_brace(tw: f64, th: f64) -> ShapeSize {
    sized(tw + 2.0 * PAD_X + 12.0, th + 2.0 * PAD_Y, 0.0)
}

fn two_braces(tw: f64, th: f64) -> ShapeSize {
    sized(tw + 2.0 * PAD_X + 24.0, th + 2.0 * PAD_Y, 0.0)
}

/// What a node is drawn as inside its layout box.
pub struct ShapeView {
    pub shape: Shape,
    pub size: Size,
}

impl ShapeView {
    /// Moves `end`, a route end on the layout box of `c`, onto the outline
    /// of the shape drawn in it.
    pub fn inset_end(&self, c: &CNode, end: &mut Point) {
        if c.is_group {
            return;
        }
        let Some(side) = c.side_of(*end) else {
            return;
        };
        if side.is_horizontal() {
            let amount = self.shape.inset(self.size, side, end.x - c.center.x)
                + (c.size.h - self.size.h) / 2.0;
            end.y += if side == Side::Top { amount } else { -amount };
        } else {
            let amount = self.shape.inset(self.size, side, end.y - c.center.y)
                + (c.size.w - self.size.w) / 2.0;
            end.x += if side == Side::Left { amount } else { -amount };
        }
    }
}

/// A `w` by `h` `<rect>` centred on the origin, with `rx` then `a`.
fn centered_rect(w: f64, h: f64, a: impl fmt::Display, rx: impl fmt::Display) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "{}",
            rect(
                Point::new(-w / 2.0, -h / 2.0),
                Size::new(w, h),
                format_args!("{rx}{a}")
            )
        )
    })
}
