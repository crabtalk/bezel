//! Pele's `diagrams/railroad/render.ts`.

use crate::config::{capped, capped_option};
use crate::geom::Point;
use crate::text::Label;
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::{collections::HashMap, fmt::Write as _};

use super::{
    common::fold,
    db::RailroadDb,
    types::{NodeId, Nodes, RailroadNode},
};
use crate::{Config, Ctx, Error};
use crate::{
    Rendered,
    diagrams::Render,
    js,
    svg::{
        builder::{Esc, EscText, fragment, num},
        root::svg_document,
        theme::{FONT_MONO, RADIUS},
    },
    text::{Host, Style, decode_entities},
};

const LEAD: f64 = 12.0;
const HEAD_GAP: f64 = 6.0;
const RULE_GAP: f64 = 18.0;
const COUNT_SIZE: f64 = 12.0;
const ARROW: f64 = 7.0;

/// What a laid-out element draws.
enum Part {
    Terminal(String),
    /// `text` as shown, and the rule `name` refers to.
    NonTerminal {
        text: String,
        name: String,
    },
    /// Written `? text ?`.
    Special(String),
    Sequence,
    Choice,
    /// A track over the element, rising `reach` above the baseline.
    Bypass {
        reach: f64,
    },
    /// A track back under the element, dropping `reach` below the baseline,
    /// and the count written under it, if any.
    Loop {
        reach: f64,
        count: String,
    },
    /// A sequence in rows, joined by rails.
    Folded,
}

impl Part {
    /// The text of a terminal, non-terminal or special.
    fn text(&self) -> &str {
        match self {
            Self::Terminal(text) | Self::NonTerminal { text, .. } | Self::Special(text) => text,
            _ => "",
        }
    }
}

/// A laid-out element. The track enters at the left edge and leaves at the
/// right edge on one baseline, with `up` above it and `down` below it.
struct Laid {
    part: Part,
    width: f64,
    up: f64,
    down: f64,
    /// Places in the list of laid-out elements.
    kids: Vec<usize>,
}

impl Laid {
    fn branch(part: Part, width: f64, up: f64, down: f64, kids: Vec<usize>) -> Self {
        Self {
            part,
            width,
            up,
            down,
            kids,
        }
    }
}

struct Metrics<'a, H> {
    measure: &'a H,
    size: f64,
    padding: f64,
    height: f64,
    /// Space between elements in a sequence, and between stacked branches.
    gap: f64,
    sep: f64,
    arc: f64,
}

/// Text is literal here: a terminal such as "<br>" or "*" is itself, not
/// markup. Mermaid's `#35;` codes still stand for characters, and escapes that
/// would be invisible are written out.
fn shown(text: &str) -> String {
    let s = if text.contains("¶ß") {
        decode_entities(&text.replace('&', "&amp;"))
    } else {
        text.to_string()
    };
    if !s.contains(['\n', '\r', '\t']) {
        return s;
    }
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn even(value: f64) -> f64 {
    (value / 2.0).ceil() * 2.0
}

fn count(min: f64, max: f64) -> String {
    if max == f64::INFINITY && min <= 1.0 {
        return String::new();
    }
    let text = |value: f64| {
        if value.is_finite() {
            js::num_str(value)
        } else {
            "∞".to_string()
        }
    };
    if min == max {
        return text(min) + "×";
    }
    if max == f64::INFINITY {
        return "≥".to_string() + &text(min) + "×";
    }
    if min == 0.0 {
        "≤".to_string() + &text(max) + "×"
    } else {
        text(min) + "–" + &text(max) + "×"
    }
}

fn add(boxes: &mut Vec<Laid>, laid: Laid) -> usize {
    boxes.push(laid);
    boxes.len() - 1
}

impl<H: Host> Metrics<'_, H> {
    /// A terminal, non-terminal or special, its text in `style`.
    fn leaf(&self, part: Part, style: u8) -> Laid {
        let half = self.height / 2.0;
        let width = even(self.measure.width(part.text(), self.size, style) + 2.0 * self.padding)
            .max(self.height);
        Laid {
            part,
            width,
            up: half,
            down: half,
            kids: Vec::new(),
        }
    }

    /// Where the next branch of a choice sits below the baseline, given the
    /// one above it.
    fn below(&self, offset: f64, above: &Laid, branch: &Laid) -> f64 {
        (offset + above.down + self.sep + branch.up).max(2.0 * self.arc)
    }

    fn bypass(&self, kid: usize, boxes: &[Laid]) -> Laid {
        let k = &boxes[kid];
        let reach = (k.up + self.sep).max(2.0 * self.arc);
        Laid::branch(
            Part::Bypass { reach },
            k.width + 4.0 * self.arc,
            reach,
            k.down,
            vec![kid],
        )
    }

    /// Lays out the definition under `root` into `boxes`, folding sequences
    /// wider than `room`, and answers its place there.
    fn lay_out(&self, root: NodeId, nodes: &Nodes, room: f64, boxes: &mut Vec<Laid>) -> usize {
        let m = self;
        let mut budgets: HashMap<NodeId, f64> = HashMap::new();
        let mut pending: Vec<(NodeId, f64)> = vec![(root, room)];
        let mut i = 0;
        while room != f64::INFINITY && i < pending.len() {
            let (node, width) = pending[i];
            budgets.insert(node, width);
            // A sequence that folds puts its rows inside rails, so its parts
            // are sized for those rows.
            let around = match nodes.get(node) {
                RailroadNode::Choice { .. } | RailroadNode::Optional { .. } => 4.0 * m.arc,
                RailroadNode::Repetition { min, .. } => {
                    (if *min == 0.0 { 6.0 } else { 2.0 }) * m.arc
                }
                RailroadNode::Sequence { .. } => 6.0 * m.arc,
                _ => 0.0,
            };
            for &child in nodes.get(node).parts() {
                pending.push((child, 40f64.max(width - around)));
            }
            i += 1;
        }
        fold(
            root,
            |node| nodes.get(node).parts(),
            |node, kids: Vec<usize>| {
                let room = budgets.get(&node).copied().unwrap_or(f64::INFINITY);
                let laid = match nodes.get(node) {
                    RailroadNode::Terminal { value } => {
                        let text = shown(value);
                        let text = if !text.is_empty() && text.bytes().all(|b| b == b' ') {
                            "␣".repeat(text.len())
                        } else {
                            text
                        };
                        m.leaf(Part::Terminal(text), Style::MONO)
                    }
                    RailroadNode::NonTerminal { name } => m.leaf(
                        Part::NonTerminal {
                            text: shown(name),
                            name: name.clone(),
                        },
                        0,
                    ),
                    RailroadNode::Special { text } => {
                        m.leaf(Part::Special(format!("? {} ?", shown(text))), Style::ITALIC)
                    }
                    RailroadNode::Sequence { .. } => {
                        let mut width = (kids.len() as f64 - 1.0).max(0.0) * m.gap;
                        let mut up = 0f64;
                        let mut down = 0f64;
                        for &kid in &kids {
                            width += boxes[kid].width;
                            up = up.max(boxes[kid].up);
                            down = down.max(boxes[kid].down);
                        }
                        if width <= room || kids.len() < 2 {
                            return add(boxes, Laid::branch(Part::Sequence, width, up, down, kids));
                        }
                        let mut rows: Vec<Laid> = Vec::new();
                        let mut row: Vec<usize> = Vec::new();
                        let mut row_w = 0.0;
                        let flush =
                            |row: &mut Vec<usize>, row_w: &mut f64, rows: &mut Vec<Laid>| {
                                let mut up = 0f64;
                                let mut down = 0f64;
                                for &kid in row.iter() {
                                    up = up.max(boxes[kid].up);
                                    down = down.max(boxes[kid].down);
                                }
                                rows.push(Laid::branch(
                                    Part::Sequence,
                                    *row_w,
                                    up,
                                    down,
                                    std::mem::take(row),
                                ));
                                *row_w = 0.0;
                            };
                        for &kid in &kids {
                            let kw = boxes[kid].width;
                            if !row.is_empty() && row_w + m.gap + kw > 40f64.max(room - 6.0 * m.arc)
                            {
                                flush(&mut row, &mut row_w, &mut rows);
                            }
                            row_w += (if row.is_empty() { 0.0 } else { m.gap }) + kw;
                            row.push(kid);
                        }
                        flush(&mut row, &mut row_w, &mut rows);
                        if rows.len() < 2 {
                            return add(boxes, Laid::branch(Part::Sequence, width, up, down, kids));
                        }
                        let mut folded_width = 0f64;
                        let mut offset = 0.0;
                        for i in 0..rows.len() {
                            folded_width = folded_width.max(rows[i].width);
                            if i > 0 {
                                offset += rows[i - 1].down + m.sep.max(2.0 * m.arc) + rows[i].up;
                            }
                        }
                        let first_up = rows[0].up;
                        let last_down = rows[rows.len() - 1].down;
                        let ids: Vec<usize> = rows.into_iter().map(|row| add(boxes, row)).collect();
                        Laid::branch(
                            Part::Folded,
                            folded_width + 6.0 * m.arc,
                            first_up,
                            offset + last_down,
                            ids,
                        )
                    }
                    RailroadNode::Choice { .. } => {
                        let mut inner = 0f64;
                        let mut offset = 0.0;
                        for i in 0..kids.len() {
                            inner = inner.max(boxes[kids[i]].width);
                            if i > 0 {
                                offset = m.below(offset, &boxes[kids[i - 1]], &boxes[kids[i]]);
                            }
                        }
                        let up = kids.first().map_or(0.0, |&kid| boxes[kid].up);
                        let down = kids.last().map_or(0.0, |&kid| offset + boxes[kid].down);
                        Laid::branch(Part::Choice, inner + 4.0 * m.arc, up, down, kids)
                    }
                    RailroadNode::Optional { .. } => m.bypass(kids[0], boxes),
                    RailroadNode::Repetition { min, max, .. } => {
                        let (min, max) = (*min, *max);
                        let kid = kids[0];
                        if min == 1.0 && max == 1.0 {
                            return kid;
                        }
                        let text = count(min, max);
                        let k = &boxes[kid];
                        let reach = (k.down + m.sep).max(2.0 * m.arc);
                        let label = if text.is_empty() {
                            0.0
                        } else {
                            even(m.measure.width(&text, COUNT_SIZE, 0))
                        };
                        let down = reach
                            + if text.is_empty() {
                                ARROW / 2.0 + 1.0
                            } else {
                                COUNT_SIZE + 6.0
                            };
                        let laid = Laid::branch(
                            Part::Loop { reach, count: text },
                            k.width.max(label) + 2.0 * m.arc,
                            k.up,
                            down,
                            vec![kid],
                        );
                        if min == 0.0 {
                            let looped = add(boxes, laid);
                            m.bypass(looped, boxes)
                        } else {
                            laid
                        }
                    }
                };
                add(boxes, laid)
            },
        )
    }
}

/// Mermaid's `railroad` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RailroadConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub arc_radius: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub font_size: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub horizontal_separation: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub marker_radius: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub padding: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub stroke_width: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub vertical_separation: Option<f64>,
    #[serde_as(as = "DefaultOnError")]
    pub show_markers: Option<bool>,
}

impl Render for RailroadDb {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.tighten(|tight| self.draw(&config.railroad, ctx, tight)))
    }
}

/// What one rule's drawing is gathered into.
struct Drawn<'a, H> {
    m: &'a Metrics<'a, H>,
    track: String,
    arrows: String,
    nodes: String,
    east_to_south: String,
    south_to_east: String,
    east_to_north: String,
    north_to_east: String,
    loop_down: String,
    loop_up: String,
    box_top: f64,
    text_shift: f64,
    stroke: String,
}

impl<H: Host> Drawn<'_, H> {
    fn line(&mut self, x1: f64, y: f64, x2: f64) {
        if x2 > x1 {
            let _ = write!(self.track, "M{},{}H{}", num(x1), num(y), num(x2));
        }
    }

    /// Draws `root` with the left end of its baseline at `start`.
    fn place(&mut self, boxes: &[Laid], root: usize, start: Point) {
        let m = self.m;
        let arc = m.arc;
        let p = crate::PREFIX;
        // Each element still to draw, and where its baseline starts.
        let mut stack: Vec<(usize, Point)> = vec![(root, start)];
        while let Some((index, Point { x, y })) = stack.pop() {
            let laid = &boxes[index];
            let kids = &laid.kids;
            let right = x + laid.width;
            match &laid.part {
                Part::Sequence => {
                    let mut kx = right;
                    for i in (0..kids.len()).rev() {
                        kx -= boxes[kids[i]].width;
                        stack.push((kids[i], Point::new(kx, y)));
                        if i > 0 {
                            self.line(kx - m.gap, y, kx);
                        }
                        kx -= m.gap;
                    }
                }
                Part::Folded => {
                    let start = x + 2.0 * arc;
                    let from = stack.len();
                    let mut offset = 0.0;
                    self.line(x, y, start);
                    for i in 0..kids.len() {
                        let kid = &boxes[kids[i]];
                        let ky = y + offset;
                        stack.push((kids[i], Point::new(start, ky)));
                        if i + 1 < kids.len() {
                            let next = &boxes[kids[i + 1]];
                            let next_offset = offset + kid.down + m.sep.max(2.0 * arc) + next.up;
                            let mid = y + offset + kid.down + m.sep.max(2.0 * arc) / 2.0;
                            let _ = write!(
                                self.track,
                                "M{},{}H{}V{}H{}V{}H{}",
                                num(start + kid.width),
                                num(ky),
                                num(right - 2.0 * arc),
                                num(mid),
                                num(x + arc),
                                num(y + next_offset),
                                num(start)
                            );
                            let _ = write!(
                                self.arrows,
                                "M{},{}l{},{}l{},{}z",
                                num(start - ARROW),
                                num(y + next_offset - ARROW / 2.0),
                                js::Number(ARROW),
                                js::Number(ARROW / 2.0),
                                js::Number(-ARROW),
                                js::Number(ARROW / 2.0)
                            );
                            offset = next_offset;
                        } else {
                            let _ = write!(
                                self.track,
                                "M{},{}H{}V{}H{}",
                                num(start + kid.width),
                                num(ky),
                                num(right - arc),
                                num(y),
                                num(right)
                            );
                        }
                    }
                    // Reversed, so that the rows come off the stack top to
                    // bottom.
                    stack[from..].reverse();
                }
                Part::Choice => {
                    let inner = laid.width - 4.0 * arc;
                    let rail_in = x + arc;
                    let rail_out = right - arc;
                    let from = stack.len();
                    let mut offset = 0.0;
                    for i in 0..kids.len() {
                        let kid = &boxes[kids[i]];
                        if i > 0 {
                            offset = m.below(offset, &boxes[kids[i - 1]], kid);
                        }
                        let ky = y + offset;
                        let kx = x + 2.0 * arc + (inner - kid.width) / 2.0;
                        let last = i == kids.len() - 1;
                        if i == 0 {
                            self.line(x, y, kx);
                            self.line(kx + kid.width, y, right);
                        } else {
                            let d = &mut self.track;
                            let _ = if last {
                                write!(
                                    d,
                                    "M{}{}V{}",
                                    Point::new(x, y),
                                    self.east_to_south,
                                    num(ky - arc)
                                )
                            } else {
                                write!(d, "M{}", Point::new(rail_in, ky - arc))
                            };
                            d.push_str(&self.south_to_east);
                            if kx > rail_in + arc {
                                let _ = write!(d, "H{}", num(kx));
                            }
                            let _ = write!(d, "M{}", Point::new(kx + kid.width, ky));
                            if kx > rail_in + arc {
                                let _ = write!(d, "H{}", num(rail_out - arc));
                            }
                            d.push_str(&self.east_to_north);
                            if last {
                                let _ = write!(d, "V{}{}", num(y + arc), self.north_to_east);
                            }
                        }
                        stack.push((kids[i], Point::new(kx, ky)));
                    }
                    // Reversed, so that the branches come off the stack top
                    // to bottom.
                    stack[from..].reverse();
                }
                Part::Bypass { reach } => {
                    let kid = &boxes[kids[0]];
                    let kx = x + 2.0 * arc;
                    self.line(x, y, kx);
                    self.line(kx + kid.width, y, right);
                    let _ = write!(
                        self.track,
                        "M{},{}{}V{}{}H{}{}V{}{}",
                        num(x),
                        num(y),
                        self.east_to_north,
                        num(y - reach + arc),
                        self.north_to_east,
                        num(right - 2.0 * arc),
                        self.east_to_south,
                        num(y - arc),
                        self.south_to_east
                    );
                    stack.push((kids[0], Point::new(kx, y)));
                }
                Part::Loop { reach, count } => {
                    let kid = &boxes[kids[0]];
                    let kx = x + (laid.width - kid.width) / 2.0;
                    let loop_y = y + reach;
                    let middle = x + laid.width / 2.0;
                    self.line(x, y, kx);
                    self.line(kx + kid.width, y, right);
                    let _ = write!(
                        self.track,
                        "M{},{}{}V{}{}H{}{}V{}{}",
                        num(x + arc),
                        num(y),
                        self.loop_down,
                        num(loop_y - arc),
                        self.south_to_east,
                        num(right - arc),
                        self.east_to_north,
                        num(y + arc),
                        self.loop_up
                    );
                    let _ = write!(
                        self.arrows,
                        "M{},{}l{},{}v{}z",
                        num(middle - ARROW / 2.0),
                        num(loop_y),
                        js::Number(ARROW),
                        js::Number(-ARROW / 2.0),
                        js::Number(ARROW)
                    );
                    if !count.is_empty() {
                        let _ = write!(
                            self.nodes,
                            "<text class=\"{p}-edge-label {p}-count\" x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                            num(middle),
                            num(loop_y + COUNT_SIZE + 3.0),
                            js::Number(COUNT_SIZE),
                            Esc(count)
                        );
                    }
                    stack.push((kids[0], Point::new(kx, y)));
                }
                part @ (Part::Terminal(_) | Part::NonTerminal { .. } | Part::Special(_)) => {
                    let text = part.text();
                    let space =
                        if text.starts_with(' ') || text.ends_with(' ') || text.contains("  ") {
                            " xml:space=\"preserve\""
                        } else {
                            ""
                        };
                    let shape = fragment(|f| {
                        write!(
                            f,
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                            num(x),
                            num(y - self.box_top),
                            num(laid.width),
                            num(m.height)
                        )
                    });
                    let label = fragment(|f| {
                        write!(
                            f,
                            "<text x=\"{}\" y=\"{}\"{space}",
                            num(x + laid.width / 2.0),
                            num(y + self.text_shift)
                        )
                    });
                    let stroke = &self.stroke;
                    let nodes = &mut self.nodes;
                    let _ = match part {
                        Part::Terminal(_) => write!(
                            nodes,
                            "<g class=\"{p}-node {p}-terminal\">{shape} rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{stroke}/>{label} font-family=\"{FONT_MONO}\">{}</text></g>",
                            num(self.box_top),
                            Esc(text)
                        ),
                        Part::NonTerminal { name, .. } => write!(
                            nodes,
                            "<g class=\"{p}-node {p}-nonterminal\" data-id=\"{}\">{shape} rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{stroke}/>{label}>{}</text></g>",
                            EscText(name),
                            Esc(text)
                        ),
                        _ => write!(
                            nodes,
                            "<g class=\"{p}-node {p}-special\">{shape} rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\" stroke-dasharray=\"4 3\"{stroke}/>{label} font-style=\"italic\"><tspan fill=\"var(--_m)\">? </tspan>{}<tspan fill=\"var(--_m)\"> ?</tspan></text></g>",
                            Esc(&text[2..text.len() - 2])
                        ),
                    };
                }
            }
        }
    }
}

impl RailroadDb {
    fn draw(&self, config: &RailroadConfig, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
        let base = ctx.options.font_size;
        let font_size = capped(config.font_size, 0.0, 0.0, 200.0);
        let size = if font_size > 0.0 {
            font_size
        } else {
            js::round(base * 0.875)
        };
        let head_size = if font_size > 0.0 { font_size } else { base };
        let padding = capped_option(config.padding, 0.0, 200.0);
        let line_height = js::round(size * 1.5);
        let measurer = ctx.host;
        let m = Metrics {
            measure: measurer,
            size,
            padding: padding.unwrap_or(12.0),
            height: even(line_height + padding.map_or(9.0, |padding| 2.0 * padding)),
            gap: capped(config.horizontal_separation, 16.0, 0.0, 500.0).min(if tight {
                12.0
            } else {
                f64::INFINITY
            }),
            sep: capped(config.vertical_separation, 10.0, 0.0, 500.0),
            arc: capped(config.arc_radius, 10.0, 0.0, 200.0).min(if tight {
                6.0
            } else {
                f64::INFINITY
            }),
        };
        let radius = if config.show_markers == Some(false) {
            0.0
        } else {
            capped(config.marker_radius, 4.0, 0.0, 50.0)
        };
        let stroke_width = capped(config.stroke_width, 0.0, 0.0, 20.0);
        let stroke = if stroke_width > 0.0 {
            format!(" stroke-width=\"{}\"", num(stroke_width))
        } else {
            String::new()
        };
        let pad = ctx.options.padding.unwrap_or(8.0);
        let arc = m.arc;

        let a = num(arc);
        let turn = |sweep: u8, dx: &str, dy: &str| format!("a{a},{a} 0 0 {sweep} {dx}{a},{dy}{a}");

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            m.measure,
            base,
            if tight { ctx.title_room() } else { 4000.0 },
            Style::BOLD,
        );
        let title_height = title.spaced_height(16.0);
        let head_height = js::round(head_size * 1.5);

        let mut drawn = Drawn {
            m: &m,
            track: String::new(),
            arrows: String::new(),
            nodes: String::new(),
            east_to_south: turn(1, "", ""),
            south_to_east: turn(0, "", ""),
            east_to_north: turn(0, "", "-"),
            north_to_east: turn(1, "", "-"),
            loop_down: turn(0, "-", ""),
            loop_up: turn(0, "-", "-"),
            box_top: m.height / 2.0,
            text_shift: size * 0.35,
            stroke: stroke.clone(),
        };
        let p = crate::PREFIX;

        let mut rules = String::new();
        let mut width = title.width;
        let mut top = pad + title_height;
        let mut bottom = pad + title.height;
        for rule in &self.rules {
            let mut boxes = Vec::new();
            let room = if tight {
                80f64.max(
                    ctx.options.max_width.unwrap_or(320.0)
                        - 2.0 * pad
                        - 4.0 * radius
                        - 2.0 * LEAD
                        - 1.0,
                )
            } else {
                f64::INFINITY
            };
            let laid = m.lay_out(rule.definition, &self.nodes, room, &mut boxes);
            let name = shown(&rule.name);
            let laid_box = &boxes[laid];
            let baseline = top + head_height + HEAD_GAP + laid_box.up;
            let start = pad + 2.0 * radius;
            let left = start + LEAD;
            let end = left + laid_box.width + LEAD;
            drawn.track.clear();
            drawn.arrows.clear();
            drawn.nodes.clear();
            drawn.line(start, baseline, left);
            drawn.line(left + laid_box.width, baseline, end);
            drawn.place(&boxes, laid, Point::new(left, baseline));
            let markers = fragment(|f| {
                if radius > 0.0 {
                    write!(
                        f,
                        "<circle class=\"{p}-marker {p}-start\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/><circle class=\"{p}-marker {p}-end\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\"/>",
                        num(pad + radius),
                        num(baseline),
                        num(radius),
                        num(end + radius),
                        num(baseline),
                        num(radius)
                    )
                } else {
                    Ok(())
                }
            });
            let _ = write!(
                rules,
                "<g class=\"{p}-rule\" data-id=\"{}\"><text class=\"{p}-rule-name\" x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"var(--_hw)\" text-anchor=\"start\">{}</text><path class=\"{p}-edge {p}-track\" d=\"{}\" fill=\"none\" stroke=\"var(--_l)\"{stroke}/>",
                EscText(&rule.name),
                num(pad),
                num(top + head_height / 2.0 + head_size * 0.35),
                num(head_size),
                Esc(&name),
                drawn.track
            );
            if !drawn.arrows.is_empty() {
                let _ = write!(
                    rules,
                    "<path class=\"{p}-marker\" d=\"{}\" fill=\"var(--_l)\"/>",
                    drawn.arrows
                );
            }
            let _ = write!(rules, "{markers}{}</g>", drawn.nodes);
            width = width.max(end + 2.0 * radius - pad).max(m.measure.width(
                &name,
                head_size,
                Style::BOLD,
            ));
            bottom = baseline + laid_box.down;
            top = bottom + m.sep + RULE_GAP;
        }

        let total_width = (width + 2.0 * pad).ceil();
        let total_height = (bottom + pad).ceil();
        let title_attrs = crate::svg::builder::TITLE;
        let mut content = String::with_capacity(rules.len() + 512);
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                title_attrs
            )
        );
        if !rules.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-rules\" transform=\"translate(0.5,0.5)\" font-size=\"{}\" text-anchor=\"middle\">{rules}</g>",
                num(size)
            );
        }

        Rendered {
            svg: svg_document(
                "railroad",
                total_width,
                total_height,
                base,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }
}
