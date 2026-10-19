//! Node shapes by Mermaid's names: how big each is around its label, how it
//! is drawn, and where edges meet its outline. Pele's `svg/shapes.ts`.

use std::{
    f64::consts::{FRAC_1_SQRT_2, PI},
    fmt::Write as _,
};

use super::{
    builder::{num, push_num},
    theme::RADIUS,
};
use crate::mermaid::layout::compound::CNode;

pub const PAD_X: f64 = 18.0;
pub const PAD_Y: f64 = 12.0;

/// Kept clear at each end of a straight side, for the corner.
const CORNER: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeSize {
    pub w: f64,
    pub h: f64,
    /// Vertical shift of the label from the shape's centre.
    pub dy: f64,
}

type Sizer = fn(f64, f64) -> ShapeSize;
type Drawer = fn(f64, f64, &str, &str) -> String;
/// Distance from the bounding box to the outline on a side: 0 top, 1 right,
/// 2 bottom, 3 left. `t` is how far along the side from its middle, to the
/// right or down.
type Inset = fn(f64, f64, i32, f64) -> f64;
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

fn round_inset(w: f64, h: f64, side: i32, t: f64) -> f64 {
    if side & 1 != 0 {
        bulge(t, h / 2.0, w / 2.0)
    } else {
        bulge(t, w / 2.0, h / 2.0)
    }
}

fn round_span(w: f64, h: f64, sideways: bool) -> f64 {
    (if sideways { h } else { w }) * 0.6
}

fn cap_inset(w: f64, _h: f64, side: i32, t: f64) -> f64 {
    if side & 1 != 0 {
        0.0
    } else {
        bulge(t, w / 2.0, 12f64.min(6f64.max(w / 14.0)))
    }
}

fn tri_size(tw: f64, th: f64, flip: bool) -> ShapeSize {
    let w = (1.6 * tw + 40.0).max(80.0);
    let h = (th + 10.0) / (1.0 - (tw + 8.0) / w);
    let dy = h / 2.0 - 10.0 - th / 2.0;
    ShapeSize {
        w,
        h,
        dy: if flip { -dy } else { dy },
    }
}

fn path(d: &str, a: &str) -> String {
    format!("<path d=\"{d}\"{a}/>")
}

fn lean_inset(_w: f64, h: f64, side: i32, _t: f64) -> f64 {
    if side & 1 != 0 { h / 8.0 } else { 0.0 }
}

fn box_size(tw: f64, th: f64) -> ShapeSize {
    ShapeSize {
        w: (tw + 2.0 * PAD_X).max(2.0 * PAD_X + 12.0),
        h: th + 2.0 * PAD_Y,
        dy: 0.0,
    }
}

fn poly(points: &[f64], a: &str) -> String {
    let mut d = String::new();
    for (i, pair) in points.chunks(2).enumerate() {
        if i > 0 {
            d.push(' ');
        }
        push_num(&mut d, pair[0]);
        d.push(',');
        push_num(&mut d, pair[1]);
    }
    format!("<polygon points=\"{d}\"{a}/>")
}

fn rect(w: f64, h: f64, a: &str, rx: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx}{a}/>",
        num(-w / 2.0),
        num(-h / 2.0),
        num(w),
        num(h)
    )
}

fn rounded() -> String {
    format!(" rx=\"{RADIUS}\"")
}

fn padded(h: f64) -> f64 {
    h + 2.0 * PAD_Y
}

fn brace(x: f64, h: f64, dir: f64, line: &str) -> String {
    let y = h / 2.0;
    let k = 6.0 * dir;
    format!(
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
}

fn wave(w: f64, h: f64, a: &str) -> String {
    let x = w / 2.0;
    let top = -h / 2.0;
    let base = h / 2.0 - 10.0;
    let q = w / 4.0;
    format!(
        "<path d=\"M{},{}H{}V{}q{},-12 {},0t{},0Z\"{a}/>",
        num(-x),
        num(top),
        num(x),
        num(base),
        num(-q),
        num(-2.0 * q),
        num(-2.0 * q)
    )
}

fn lookup(name: &str) -> Option<ShapeDef> {
    Some(match name {
        "rect" => def(box_size, |w, h, a, _| rect(w, h, a, &rounded())).span(flat),
        "rounded" => def(box_size, |w, h, a, _| {
            rect(w, h, a, &format!(" rx=\"{}\"", num(12f64.min(h / 2.0))))
        })
        .span(flat),
        "stadium" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + h * 0.5 + 2.0 * PAD_X - 8.0,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| rect(w, h, a, &format!(" rx=\"{}\"", num(h / 2.0))),
        )
        .inset(|_w, h, side, t| {
            if side & 1 != 0 {
                bulge(t, h / 2.0, h / 2.0)
            } else {
                0.0
            }
        })
        .span(|w, h, sideways| if sideways { h * 0.6 } else { w - h }),
        "fr-rect" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 16.0,
                h: padded(th),
                dy: 0.0,
            },
            |w, h, a, line| {
                let x = w / 2.0 - 8.0;
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}V{}M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-x),
                        num(-h / 2.0),
                        num(h / 2.0),
                        num(x),
                        num(-h / 2.0),
                        num(h / 2.0)
                    )
            },
        )
        .span(flat),
        "cyl" => def(
            |tw, th| {
                let w = (tw + 2.0 * PAD_X).max(56.0);
                let ry = 12f64.min(6f64.max(w / 14.0));
                ShapeSize {
                    w,
                    h: th + 2.0 * PAD_Y + 2.0 * ry,
                    dy: ry / 2.0,
                }
            },
            |w, h, a, line| {
                let rx = w / 2.0;
                let ry = 12f64.min(6f64.max(w / 14.0));
                let top = -h / 2.0 + ry;
                let bottom = h / 2.0 - ry;
                format!(
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
                )
            },
        )
        .inset(cap_inset)
        .span(|w, h, sideways| {
            if sideways {
                h - 2.0 * CORNER - 12.0
            } else {
                w * 0.6
            }
        }),
        "circle" => def(
            |tw, th| {
                let d = ((tw * tw + th * th).sqrt() + 16.0).max(44.0);
                ShapeSize { w: d, h: d, dy: 0.0 }
            },
            |w, _h, a, _| format!("<circle r=\"{}\"{a}/>", num(w / 2.0)),
        )
        .inset(round_inset)
        .span(round_span),
        "dbl-circ" => def(
            |tw, th| {
                let d = ((tw * tw + th * th).sqrt() + 26.0).max(54.0);
                ShapeSize { w: d, h: d, dy: 0.0 }
            },
            |w, _h, a, line| {
                format!(
                    "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"none\"{line}/>",
                    num(w / 2.0),
                    num(w / 2.0 - 5.0)
                )
            },
        )
        .inset(round_inset)
        .span(round_span),
        "ellipse" => def(
            |tw, th| ShapeSize {
                w: tw * 1.3 + 2.0 * PAD_X,
                h: th * 1.2 + 2.0 * PAD_Y,
                dy: 0.0,
            },
            |w, h, a, _| {
                format!(
                    "<ellipse rx=\"{}\" ry=\"{}\"{a}/>",
                    num(w / 2.0),
                    num(h / 2.0)
                )
            },
        )
        .inset(round_inset)
        .span(round_span),
        "diam" => def(
            |tw, th| {
                let a = tw + 16.0;
                let b = th + 8.0;
                ShapeSize {
                    w: a + 1.6 * b,
                    h: a / 1.6 + b,
                    dy: 0.0,
                }
            },
            |w, h, a, _| poly(&[0.0, -h / 2.0, w / 2.0, 0.0, 0.0, h / 2.0, -w / 2.0, 0.0], a),
        )
        .inset(|w, h, side, t| {
            if side & 1 != 0 {
                t.abs() * w / h
            } else {
                t.abs() * h / w
            }
        })
        .span(|w, h, sideways| (if sideways { h } else { w }) * 0.6),
        "hex" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + h / 2.0,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| {
                let m = h / 4.0;
                poly(
                    &[
                        -w / 2.0 + m,
                        -h / 2.0,
                        w / 2.0 - m,
                        -h / 2.0,
                        w / 2.0,
                        0.0,
                        w / 2.0 - m,
                        h / 2.0,
                        -w / 2.0 + m,
                        h / 2.0,
                        -w / 2.0,
                        0.0,
                    ],
                    a,
                )
            },
        )
        .inset(|_w, _h, side, t| if side & 1 != 0 { t.abs() / 2.0 } else { 0.0 })
        .span(|w, h, sideways| {
            if sideways {
                h * 0.5
            } else {
                w - h / 2.0 - CORNER
            }
        }),
        "odd" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + h / 4.0,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| {
                let notch = h / 4.0;
                poly(
                    &[
                        -w / 2.0,
                        -h / 2.0,
                        w / 2.0,
                        -h / 2.0,
                        w / 2.0,
                        h / 2.0,
                        -w / 2.0,
                        h / 2.0,
                        -w / 2.0 + notch,
                        0.0,
                    ],
                    a,
                )
            },
        )
        .inset(|_w, h, side, _t| if side == 3 { h / 4.0 } else { 0.0 })
        .span(|w, h, sideways| flat_across(w, h, sideways, 0.0)),
        "lean-r" => def(lean_size, |w, h, a, _| {
            let s = h / 4.0;
            poly(
                &[
                    -w / 2.0 + s,
                    -h / 2.0,
                    w / 2.0,
                    -h / 2.0,
                    w / 2.0 - s,
                    h / 2.0,
                    -w / 2.0,
                    h / 2.0,
                ],
                a,
            )
        })
        .inset(lean_inset)
        .span(quarter_across),
        "lean-l" => def(lean_size, |w, h, a, _| {
            let s = h / 4.0;
            poly(
                &[
                    -w / 2.0,
                    -h / 2.0,
                    w / 2.0 - s,
                    -h / 2.0,
                    w / 2.0,
                    h / 2.0,
                    -w / 2.0 + s,
                    h / 2.0,
                ],
                a,
            )
        })
        .inset(lean_inset)
        .span(quarter_across),
        "trap-b" => def(lean_size, |w, h, a, _| {
            let s = h / 4.0;
            poly(
                &[
                    -w / 2.0 + s,
                    -h / 2.0,
                    w / 2.0 - s,
                    -h / 2.0,
                    w / 2.0,
                    h / 2.0,
                    -w / 2.0,
                    h / 2.0,
                ],
                a,
            )
        })
        .inset(lean_inset)
        .span(quarter_across),
        "trap-t" => def(lean_size, |w, h, a, _| {
            let s = h / 4.0;
            poly(
                &[
                    -w / 2.0,
                    -h / 2.0,
                    w / 2.0,
                    -h / 2.0,
                    w / 2.0 - s,
                    h / 2.0,
                    -w / 2.0 + s,
                    h / 2.0,
                ],
                a,
            )
        })
        .inset(lean_inset)
        .span(quarter_across),
        "text" => def(
            |tw, th| ShapeSize {
                w: tw + 8.0,
                h: th + 8.0,
                dy: 0.0,
            },
            |w, h, _, _| rect(w, h, " fill=\"none\" stroke=\"none\"", ""),
        )
        .span(flat),
        "sm-circ" => def(fixed14, |w, _h, a, _| {
            format!("<circle r=\"{}\"{a}/>", num(w / 2.0))
        })
        .no_label()
        .inset(round_inset)
        .span(round_span),
        "f-circ" => def(fixed14, |w, _h, _a, line| {
            format!("<circle r=\"{}\" fill=\"var(--_l)\"{line}/>", num(w / 2.0))
        })
        .no_label()
        .inset(round_inset)
        .span(round_span),
        "fr-circ" => def(
            |_, _| ShapeSize {
                w: 18.0,
                h: 18.0,
                dy: 0.0,
            },
            |w, _h, a, line| {
                format!(
                    "<circle r=\"{}\"{a}/><circle r=\"{}\" fill=\"var(--_l)\"{line}/>",
                    num(w / 2.0),
                    num(w / 2.0 - 4.0)
                )
            },
        )
        .no_label()
        .inset(round_inset)
        .span(round_span),
        "fork" => def(
            |_, _| ShapeSize {
                w: 72.0,
                h: 8.0,
                dy: 0.0,
            },
            |w, h, _a, line| rect(w, h, &format!(" fill=\"var(--_l)\"{line}"), " rx=\"2\""),
        )
        .no_label()
        .span(|w, h, sideways| (if sideways { h } else { w }) - 8.0),
        "notch-rect" => def(box_size, |w, h, a, _| {
            let c = 10.0;
            poly(
                &[
                    -w / 2.0 + c,
                    -h / 2.0,
                    w / 2.0,
                    -h / 2.0,
                    w / 2.0,
                    h / 2.0,
                    -w / 2.0,
                    h / 2.0,
                    -w / 2.0,
                    -h / 2.0 + c,
                ],
                a,
            )
        })
        .span(flat),
        "lin-rect" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: padded(th),
                dy: 0.0,
            },
            |w, h, a, line| {
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0)
                    )
            },
        )
        .span(flat),
        "div-rect" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: th + 2.0 * PAD_Y + 8.0,
                dy: 4.0,
            },
            |w, h, a, line| {
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}H{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0),
                        num(-h / 2.0 + 8.0),
                        num(w / 2.0)
                    )
            },
        )
        .span(flat),
        "win-pane" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: th + 2.0 * PAD_Y + 8.0,
                dy: 4.0,
            },
            |w, h, a, line| {
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}V{}M{},{}H{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0),
                        num(-w / 2.0),
                        num(-h / 2.0 + 8.0),
                        num(w / 2.0)
                    )
            },
        )
        .span(flat),
        "tri" => def(
            |tw, th| tri_size(tw, th, false),
            |w, h, a, _| poly(&[0.0, -h / 2.0, w / 2.0, h / 2.0, -w / 2.0, h / 2.0], a),
        )
        .inset(|w, _h, side, _t| if side & 1 != 0 { w / 4.0 } else { 0.0 }),
        "flip-tri" => def(
            |tw, th| tri_size(tw, th, true),
            |w, h, a, _| poly(&[-w / 2.0, -h / 2.0, w / 2.0, -h / 2.0, 0.0, h / 2.0], a),
        )
        .inset(|w, _h, side, _t| if side & 1 != 0 { w / 4.0 } else { 0.0 }),
        "sl-rect" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: th + 2.0 * PAD_Y + 10.0,
                dy: 5.0,
            },
            |w, h, a, _| {
                poly(
                    &[
                        -w / 2.0,
                        -h / 2.0 + 10.0,
                        w / 2.0,
                        -h / 2.0,
                        w / 2.0,
                        h / 2.0,
                        -w / 2.0,
                        h / 2.0,
                    ],
                    a,
                )
            },
        )
        .inset(|_w, _h, side, _t| if side == 0 { 5.0 } else { 0.0 }),
        "notch-pent" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: padded(th),
                dy: 0.0,
            },
            |w, h, a, _| {
                let c = 10.0;
                poly(
                    &[
                        -w / 2.0 + c,
                        -h / 2.0,
                        w / 2.0 - c,
                        -h / 2.0,
                        w / 2.0,
                        -h / 2.0 + c,
                        w / 2.0,
                        h / 2.0,
                        -w / 2.0,
                        h / 2.0,
                        -w / 2.0,
                        -h / 2.0 + c,
                    ],
                    a,
                )
            },
        )
        .span(flat),
        "hourglass" => def(
            |_, _| ShapeSize {
                w: 32.0,
                h: 40.0,
                dy: 0.0,
            },
            |w, h, a, _| {
                poly(
                    &[
                        -w / 2.0,
                        -h / 2.0,
                        w / 2.0,
                        -h / 2.0,
                        -w / 2.0,
                        h / 2.0,
                        w / 2.0,
                        h / 2.0,
                    ],
                    a,
                )
            },
        )
        .no_label()
        .inset(|w, _h, side, _t| if side & 1 != 0 { w / 2.0 } else { 0.0 }),
        "delay" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + h / 4.0,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| {
                let r = h / 2.0;
                format!(
                    "<path d=\"M{},{}H{}A{},{} 0 0 1 {},{}H{}Z\"{a}/>",
                    num(-w / 2.0),
                    num(-r),
                    num(w / 2.0 - r),
                    num(r),
                    num(r),
                    num(w / 2.0 - r),
                    num(r),
                    num(-w / 2.0)
                )
            },
        ),
        "h-cyl" => def(
            |tw, th| {
                let h = padded(th);
                let rx = 12f64.min(h / 4.0);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + 2.0 * rx,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, line| {
                let ry = h / 2.0;
                let rx = 12f64.min(h / 4.0);
                let l = -w / 2.0 + rx;
                let r = w / 2.0 - rx;
                format!(
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
                )
            },
        ),
        "cross-circ" => def(
            |_, _| ShapeSize {
                w: 44.0,
                h: 44.0,
                dy: 0.0,
            },
            |w, _h, a, line| {
                let r = w / 2.0;
                let k = r * FRAC_1_SQRT_2;
                format!(
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
                )
            },
        )
        .no_label()
        .inset(round_inset)
        .span(round_span),
        "st-rect" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: th + 2.0 * PAD_Y + 8.0,
                dy: 4.0,
            },
            |w, h, a, _| {
                let bw = w - 8.0;
                let bh = h - 8.0;
                let mut out = String::new();
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
                out
            },
        )
        .span(|w, h, sideways| (if sideways { h } else { w }) - 2.0 * CORNER - 8.0),
        "doc" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: th + 2.0 * PAD_Y + 10.0,
                dy: -5.0,
            },
            |w, h, a, _| wave(w, h, a),
        )
        .inset(|_w, _h, side, _t| if side == 2 { 10.0 } else { 0.0 }),
        "flag" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: th + 2.0 * PAD_Y + 16.0,
                dy: 0.0,
            },
            |w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0 - 8.0;
                let q = w / 4.0;
                format!(
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
                )
            },
        )
        .inset(|_w, _h, side, _t| if side & 1 != 0 { 0.0 } else { 8.0 }),
        "datastore" => def(box_size, |w, h, a, line| {
            rect(
                w,
                h,
                &a.replacen("stroke=\"var(--_b)\"", "stroke=\"none\"", 1),
                "",
            ) + &format!(
                "<path d=\"M{},{}H{}M{},{}H{}\" fill=\"none\"{line}/>",
                num(-w / 2.0),
                num(-h / 2.0),
                num(w / 2.0),
                num(-w / 2.0),
                num(h / 2.0),
                num(w / 2.0)
            )
        })
        .span(flat),
        "folder" => def(
            |tw, th| ShapeSize {
                w: (tw + 2.0 * PAD_X).max(64.0),
                h: th + 2.0 * PAD_Y + 8.0,
                dy: 4.0,
            },
            |w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let tab = (w * 0.4).min(48.0);
                path(
                    &format!(
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
                )
            },
        ),
        "bucket" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 20.0,
                h: th + 2.0 * PAD_Y + 10.0,
                dy: 3.0,
            },
            |w, h, a, line| {
                let x = w / 2.0;
                let y = h / 2.0;
                let ry = 5.0;
                let taper = h / 6.0;
                path(
                    &format!(
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
                ) + &format!(
                    "<path d=\"M{},{}A{},{} 0 0 0 {},{}\" fill=\"none\"{line}/>",
                    num(-x),
                    num(-y + ry),
                    num(x),
                    num(ry),
                    num(x),
                    num(-y + ry)
                )
            },
        )
        .inset(|_w, h, side, _t| if side & 1 != 0 { h / 12.0 } else { 0.0 }),
        "console" => def(
            |tw, th| ShapeSize {
                w: (tw + 2.0 * PAD_X).max(72.0),
                h: th + 2.0 * PAD_Y + 12.0,
                dy: 6.0,
            },
            |w, h, a, line| {
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}H{}M{},{}l3,2.5l-3,2.5M{},{}h4\" fill=\"none\"{line}/>",
                        num(-w / 2.0),
                        num(-h / 2.0 + 12.0),
                        num(w / 2.0),
                        num(-w / 2.0 + 7.0),
                        num(-h / 2.0 + 4.0),
                        num(-w / 2.0 + 13.0),
                        num(-h / 2.0 + 9.0)
                    )
            },
        )
        .span(flat),
        "browser" => def(
            |tw, th| ShapeSize {
                w: (tw + 2.0 * PAD_X).max(72.0),
                h: th + 2.0 * PAD_Y + 12.0,
                dy: 6.0,
            },
            |w, h, a, line| {
                let mut dots = String::new();
                for i in 0..3 {
                    let _ = write!(dots,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"1.5\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0 + f64::from(i) * 7.0),
                        num(-h / 2.0 + 6.0)
                    );
                }
                rect(w, h, a, &rounded())
                    + &format!(
                        "<path d=\"M{},{}H{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0),
                        num(-h / 2.0 + 12.0),
                        num(w / 2.0)
                    )
                    + &dots
            },
        )
        .span(flat),
        "person" => def(
            |tw, th| ShapeSize {
                w: (tw + 2.0 * PAD_X).max(56.0),
                h: th + 2.0 * PAD_Y + 26.0,
                dy: 13.0,
            },
            |w, h, a, _| {
                let top = -h / 2.0;
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"12\"{a}/><circle cy=\"{}\" r=\"13\"{a}/>",
                    num(-w / 2.0),
                    num(top + 26.0),
                    num(w),
                    num(h - 26.0),
                    num(top + 14.0)
                )
            },
        ),
        "bang" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 28.0,
                h: th + 2.0 * PAD_Y + 24.0,
                dy: 0.0,
            },
            |w, h, a, _| {
                let spikes = 14.0;
                let mut pts = Vec::new();
                for i in 0..28 {
                    let i = f64::from(i);
                    let angle = PI * i / spikes - PI / 2.0;
                    let k = if i % 2.0 == 0.0 { 1.0 } else { 0.82 };
                    pts.push(angle.cos() * (w / 2.0) * k);
                    pts.push(angle.sin() * (h / 2.0) * k);
                }
                poly(&pts, a)
            },
        )
        .inset(|_, _, _, _| 3.0),
        "cloud" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 32.0,
                h: th + 2.0 * PAD_Y + 24.0,
                dy: 0.0,
            },
            |w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let r = y * 0.55;
                path(
                    &format!(
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
                )
            },
        )
        .inset(|_w, _h, side, _t| if side & 1 != 0 { 2.0 } else { 1.0 }),
        "brace" => def(one_brace, |w, h, _a, line| {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "") + &brace(-w / 2.0 + 12.0, h, -1.0, line)
        }),
        "brace-r" => def(one_brace, |w, h, _a, line| {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "") + &brace(w / 2.0 - 12.0, h, 1.0, line)
        }),
        "braces" => def(two_braces, |w, h, _a, line| {
            rect(w, h, " fill=\"none\" stroke=\"none\"", "")
                + &brace(-w / 2.0 + 12.0, h, -1.0, line)
                + &brace(w / 2.0 - 12.0, h, 1.0, line)
        }),
        "bolt" => def(
            |_, _| ShapeSize {
                w: 36.0,
                h: 56.0,
                dy: 0.0,
            },
            |w, h, a, _| {
                poly(
                    &[
                        w * 0.15,
                        -h / 2.0,
                        -w / 2.0,
                        h * 0.08,
                        -w * 0.05,
                        h * 0.08,
                        -w * 0.15,
                        h / 2.0,
                        w / 2.0,
                        -h * 0.08,
                        w * 0.05,
                        -h * 0.08,
                    ],
                    a,
                )
            },
        )
        .no_label()
        .inset(|w, _h, side, _t| if side & 1 != 0 { w / 4.0 } else { 0.0 }),
        "lin-cyl" => def(
            |tw, th| {
                let w = (tw + 2.0 * PAD_X).max(56.0);
                let ry = 12f64.min(6f64.max(w / 14.0));
                ShapeSize {
                    w,
                    h: th + 2.0 * PAD_Y + 2.0 * ry + 8.0,
                    dy: ry / 2.0 + 4.0,
                }
            },
            |w, h, a, line| {
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
                path(
                    &format!(
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
                ) + &format!(
                    "<path d=\"{}{}\" fill=\"none\"{line}/>",
                    arc(top),
                    arc(top + 8.0)
                )
            },
        )
        .inset(cap_inset)
        .span(|w, h, sideways| {
            if sideways {
                h - 2.0 * CORNER - 20.0
            } else {
                w * 0.6
            }
        }),
        "curv-trap" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + h * 0.6,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let k = h * 0.3;
                path(
                    &format!(
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
                )
            },
        ),
        "docs" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: th + 2.0 * PAD_Y + 18.0,
                dy: -1.0,
            },
            |w, h, a, _| {
                let mut out = String::new();
                for i in 0..3 {
                    let _ = write!(out,
                        "<g transform=\"translate({},{})\">{}</g>",
                        (1 - i) * 4,
                        (i - 1) * 4,
                        wave(w - 8.0, h - 8.0, a)
                    );
                }
                out
            },
        )
        .inset(|_w, _h, side, _t| if side == 2 { 10.0 } else { 0.0 }),
        "bow-rect" => def(
            |tw, th| {
                let h = padded(th);
                ShapeSize {
                    w: tw + 2.0 * PAD_X + h * 0.4,
                    h,
                    dy: 0.0,
                }
            },
            |w, h, a, _| {
                let x = w / 2.0;
                let y = h / 2.0;
                let k = h * 0.2;
                path(
                    &format!(
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
                )
            },
        )
        .inset(|_w, h, side, _t| if side == 1 { h * 0.2 } else { 0.0 }),
        "tag-rect" => def(box_size, |w, h, a, line| {
            rect(w, h, a, &rounded())
                + &format!(
                    "<path d=\"M{},{}L{},{}\" fill=\"none\"{line}/>",
                    num(w / 2.0 - 12.0),
                    num(h / 2.0),
                    num(w / 2.0),
                    num(h / 2.0 - 12.0)
                )
        })
        .span(flat),
        "tag-doc" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X,
                h: th + 2.0 * PAD_Y + 10.0,
                dy: -5.0,
            },
            |w, h, a, line| {
                wave(w, h, a)
                    + &format!(
                        "<path d=\"M{},{}L{},{}\" fill=\"none\"{line}/>",
                        num(w / 2.0 - 12.0),
                        num(h / 2.0 - 14.0),
                        num(w / 2.0),
                        num(h / 2.0 - 26.0)
                    )
            },
        )
        .inset(|_w, _h, side, _t| if side == 2 { 10.0 } else { 0.0 }),
        "lin-doc" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: th + 2.0 * PAD_Y + 10.0,
                dy: -5.0,
            },
            |w, h, a, line| {
                wave(w, h, a)
                    + &format!(
                        "<path d=\"M{},{}V{}\" fill=\"none\"{line}/>",
                        num(-w / 2.0 + 8.0),
                        num(-h / 2.0),
                        num(h / 2.0 - 6.0)
                    )
            },
        )
        .inset(|_w, _h, side, _t| if side == 2 { 10.0 } else { 0.0 }),
        "collapsedGroup" => def(
            |tw, th| ShapeSize {
                w: tw + 2.0 * PAD_X + 8.0,
                h: th + 2.0 * PAD_Y + 8.0,
                dy: -2.0,
            },
            |w, h, a, _| {
                format!(
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
                )
            },
        ),
        _ => return None,
    })
}

fn lean_size(tw: f64, th: f64) -> ShapeSize {
    let h = padded(th);
    ShapeSize {
        w: tw + 2.0 * PAD_X + h / 2.0,
        h,
        dy: 0.0,
    }
}

fn quarter_across(w: f64, h: f64, sideways: bool) -> f64 {
    flat_across(w, h, sideways, h / 4.0)
}

fn fixed14(_: f64, _: f64) -> ShapeSize {
    ShapeSize {
        w: 14.0,
        h: 14.0,
        dy: 0.0,
    }
}

fn one_brace(tw: f64, th: f64) -> ShapeSize {
    ShapeSize {
        w: tw + 2.0 * PAD_X + 12.0,
        h: th + 2.0 * PAD_Y,
        dy: 0.0,
    }
}

fn two_braces(tw: f64, th: f64) -> ShapeSize {
    ShapeSize {
        w: tw + 2.0 * PAD_X + 24.0,
        h: th + 2.0 * PAD_Y,
        dy: 0.0,
    }
}

/// `name`'s definition, or the rectangle's for a name it does not have.
fn of(name: &str) -> ShapeDef {
    lookup(name)
        .or_else(|| lookup("rect"))
        .expect("rect is a shape")
}

pub fn shape_size(shape: &str, tw: f64, th: f64) -> ShapeSize {
    (of(shape).size)(tw, th)
}

pub fn draw_shape(shape: &str, w: f64, h: f64, a: &str, line: &str) -> String {
    (of(shape).draw)(w, h, a, line)
}

pub fn shape_inset(shape_name: &str, w: f64, h: f64, side: i32, t: f64) -> f64 {
    lookup(shape_name)
        .and_then(|def| def.inset)
        .map_or(0.0, |inset| inset(w, h, side, t))
}

/// How much of a side of the shape edges may spread along, when the flow
/// runs down or, if `sideways`, across.
pub fn shape_span(shape_name: &str, w: f64, h: f64, sideways: bool) -> f64 {
    0f64.max(
        lookup(shape_name)
            .and_then(|def| def.span)
            .map_or(0.0, |span| span(w, h, sideways)),
    )
}

pub fn shape_has_label(shape_name: &str) -> bool {
    lookup(shape_name).is_none_or(|def| !def.no_label)
}

/// The side of a node's layout box a route end lies on: 0 top, 1 right, 2
/// bottom, 3 left, or -1 for none.
pub fn end_side(route: &[f64], at: usize, c: &CNode) -> i32 {
    let dx = route[at] - c.x;
    let dy = route[at + 1] - c.y;
    if (dy.abs() - c.h / 2.0).abs() < 0.5 {
        return if dy < 0.0 { 0 } else { 2 };
    }
    if (dx.abs() - c.w / 2.0).abs() < 0.5 {
        return if dx < 0.0 { 3 } else { 1 };
    }
    -1
}

/// What a node is drawn as inside its layout box.
pub struct ShapeView<'a> {
    pub shape: &'a str,
    pub w: f64,
    pub h: f64,
}

/// Moves a route end from a node's layout box onto the outline of the shape
/// drawn in it.
pub fn inset_route(route: &mut [f64], at: usize, view: &ShapeView, c: &CNode) {
    if c.is_group {
        return;
    }
    let side = end_side(route, at, c);
    if side == 0 || side == 2 {
        let amount =
            shape_inset(view.shape, view.w, view.h, side, route[at] - c.x) + (c.h - view.h) / 2.0;
        route[at + 1] += if side == 0 { amount } else { -amount };
    } else if side >= 0 {
        let amount = shape_inset(view.shape, view.w, view.h, side, route[at + 1] - c.y)
            + (c.w - view.w) / 2.0;
        route[at] += if side == 3 { amount } else { -amount };
    }
}
