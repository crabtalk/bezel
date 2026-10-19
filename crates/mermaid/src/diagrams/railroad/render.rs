//! Pele's `diagrams/railroad/render.ts`.

use std::{collections::HashMap, fmt::Write as _};

use super::{
    common::fold,
    db::RailroadDb,
    types::{NodeId, Nodes, RailroadNode},
};
use crate::Ctx;
use crate::{
    Config, Rendered, Value,
    diagrams::common::fit_width::{tighten, title_room},
    js,
    svg::{
        builder::{Esc, EscText, LabelSvg, Point, fragment, num},
        root::{Accessible, svg_document},
        theme::{FONT_MONO, RADIUS},
    },
    text::{Host, Style, decode_entities, layout_label},
};

const LEAD: f64 = 12.0;
const HEAD_GAP: f64 = 6.0;
const RULE_GAP: f64 = 18.0;
const COUNT_SIZE: f64 = 12.0;
const ARROW: f64 = 7.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Terminal,
    NonTerminal,
    Special,
    Sequence,
    Choice,
    Bypass,
    Loop,
    Folded,
}

/// A laid-out element. The track enters at the left edge and leaves at the
/// right edge on one baseline, with `up` above it and `down` below it.
struct Laid {
    kind: Kind,
    width: f64,
    up: f64,
    down: f64,
    /// Places in the list of laid-out elements.
    kids: Vec<usize>,
    /// A leaf's text, or a loop's count.
    text: String,
    /// The name a non-terminal refers to.
    id: String,
    /// How far a bypass rises above the baseline, or a loop drops below it.
    reach: f64,
}

impl Laid {
    fn branch(kind: Kind, width: f64, up: f64, down: f64, kids: Vec<usize>) -> Self {
        Self {
            kind,
            width,
            up,
            down,
            kids,
            text: String::new(),
            id: String::new(),
            reach: 0.0,
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

fn option(config: &Config, key: &str, fallback: f64, max: f64) -> f64 {
    match config
        .section("railroad")
        .and_then(|section| section.get(key))
    {
        Some(Value::Number(value)) if *value >= 0.0 => value.min(max),
        _ => fallback,
    }
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

fn reverse<T>(list: &mut [T], from: usize) {
    if from < list.len() {
        list[from..].reverse();
    }
}

fn even(value: f64) -> f64 {
    (value / 2.0).ceil() * 2.0
}

fn leaf(kind: Kind, text: String, id: String, style: u8, m: &Metrics<'_, impl Host>) -> Laid {
    let half = m.height / 2.0;
    let width = even(m.measure.width(&text, m.size, style) + 2.0 * m.padding).max(m.height);
    Laid {
        kind,
        width,
        up: half,
        down: half,
        kids: Vec::new(),
        text,
        id,
        reach: 0.0,
    }
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

/// Where the next branch of a choice sits below the baseline, given the one
/// above it.
fn below(offset: f64, above: &Laid, branch: &Laid, m: &Metrics<'_, impl Host>) -> f64 {
    (offset + above.down + m.sep + branch.up).max(2.0 * m.arc)
}

fn bypass(kid: usize, boxes: &[Laid], m: &Metrics<'_, impl Host>) -> Laid {
    let k = &boxes[kid];
    let reach = (k.up + m.sep).max(2.0 * m.arc);
    Laid {
        kind: Kind::Bypass,
        width: k.width + 4.0 * m.arc,
        up: reach,
        down: k.down,
        kids: vec![kid],
        text: String::new(),
        id: String::new(),
        reach,
    }
}

fn add(boxes: &mut Vec<Laid>, laid: Laid) -> usize {
    boxes.push(laid);
    boxes.len() - 1
}

fn measure(
    root: NodeId,
    nodes: &Nodes,
    m: &Metrics<'_, impl Host>,
    room: f64,
    boxes: &mut Vec<Laid>,
) -> usize {
    let mut budgets: HashMap<NodeId, f64> = HashMap::new();
    let mut pending: Vec<(NodeId, f64)> = vec![(root, room)];
    let mut i = 0;
    while room != f64::INFINITY && i < pending.len() {
        let (node, width) = pending[i];
        budgets.insert(node, width);
        // A sequence that folds puts its rows inside rails, so its parts are
        // sized for those rows.
        let around = match nodes.get(node) {
            RailroadNode::Choice { .. } | RailroadNode::Optional { .. } => 4.0 * m.arc,
            RailroadNode::Repetition { min, .. } => (if *min == 0.0 { 6.0 } else { 2.0 }) * m.arc,
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
                    leaf(Kind::Terminal, text, String::new(), Style::MONO, m)
                }
                RailroadNode::NonTerminal { name } => {
                    leaf(Kind::NonTerminal, shown(name), name.clone(), 0, m)
                }
                RailroadNode::Special { text } => leaf(
                    Kind::Special,
                    format!("? {} ?", shown(text)),
                    String::new(),
                    Style::ITALIC,
                    m,
                ),
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
                        return add(boxes, Laid::branch(Kind::Sequence, width, up, down, kids));
                    }
                    let mut rows: Vec<Laid> = Vec::new();
                    let mut row: Vec<usize> = Vec::new();
                    let mut row_w = 0.0;
                    let flush = |row: &mut Vec<usize>, row_w: &mut f64, rows: &mut Vec<Laid>| {
                        let mut up = 0f64;
                        let mut down = 0f64;
                        for &kid in row.iter() {
                            up = up.max(boxes[kid].up);
                            down = down.max(boxes[kid].down);
                        }
                        rows.push(Laid::branch(
                            Kind::Sequence,
                            *row_w,
                            up,
                            down,
                            std::mem::take(row),
                        ));
                        *row_w = 0.0;
                    };
                    for &kid in &kids {
                        let kw = boxes[kid].width;
                        if !row.is_empty() && row_w + m.gap + kw > 40f64.max(room - 6.0 * m.arc) {
                            flush(&mut row, &mut row_w, &mut rows);
                        }
                        row_w += (if row.is_empty() { 0.0 } else { m.gap }) + kw;
                        row.push(kid);
                    }
                    flush(&mut row, &mut row_w, &mut rows);
                    if rows.len() < 2 {
                        return add(boxes, Laid::branch(Kind::Sequence, width, up, down, kids));
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
                        Kind::Folded,
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
                            offset = below(offset, &boxes[kids[i - 1]], &boxes[kids[i]], m);
                        }
                    }
                    let up = kids.first().map_or(0.0, |&kid| boxes[kid].up);
                    let down = kids.last().map_or(0.0, |&kid| offset + boxes[kid].down);
                    Laid::branch(Kind::Choice, inner + 4.0 * m.arc, up, down, kids)
                }
                RailroadNode::Optional { .. } => bypass(kids[0], boxes, m),
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
                    let laid = Laid {
                        kind: Kind::Loop,
                        width: k.width.max(label) + 2.0 * m.arc,
                        up: k.up,
                        down,
                        kids: vec![kid],
                        text,
                        id: String::new(),
                        reach,
                    };
                    if min == 0.0 {
                        let looped = add(boxes, laid);
                        bypass(looped, boxes, m)
                    } else {
                        laid
                    }
                }
            };
            add(boxes, laid)
        },
    )
}

pub fn render_railroad(model: &RailroadDb, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    tighten(ctx, |tight| draw(model, config, ctx, tight))
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

    fn place(&mut self, boxes: &[Laid], root: usize, left: f64, baseline: f64) {
        let m = self.m;
        let arc = m.arc;
        let p = crate::PREFIX;
        let mut stack: Vec<usize> = vec![root];
        let mut at: Vec<f64> = vec![left, baseline];
        while let Some(index) = stack.pop() {
            let y = at.pop().unwrap_or_default();
            let x = at.pop().unwrap_or_default();
            let laid = &boxes[index];
            let kids = &laid.kids;
            let right = x + laid.width;
            match laid.kind {
                Kind::Sequence => {
                    let mut kx = right;
                    for i in (0..kids.len()).rev() {
                        kx -= boxes[kids[i]].width;
                        stack.push(kids[i]);
                        at.push(kx);
                        at.push(y);
                        if i > 0 {
                            self.line(kx - m.gap, y, kx);
                        }
                        kx -= m.gap;
                    }
                }
                Kind::Folded => {
                    let start = x + 2.0 * arc;
                    let from = stack.len();
                    let mut offset = 0.0;
                    self.line(x, y, start);
                    for i in 0..kids.len() {
                        let kid = &boxes[kids[i]];
                        let ky = y + offset;
                        stack.push(kids[i]);
                        at.push(ky);
                        at.push(start);
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
                    reverse(&mut stack, from);
                    reverse(&mut at, 2 * from);
                }
                Kind::Choice => {
                    let inner = laid.width - 4.0 * arc;
                    let rail_in = x + arc;
                    let rail_out = right - arc;
                    let from = stack.len();
                    let mut offset = 0.0;
                    for i in 0..kids.len() {
                        let kid = &boxes[kids[i]];
                        if i > 0 {
                            offset = below(offset, &boxes[kids[i - 1]], kid, m);
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
                                    Point(x, y),
                                    self.east_to_south,
                                    num(ky - arc)
                                )
                            } else {
                                write!(d, "M{}", Point(rail_in, ky - arc))
                            };
                            d.push_str(&self.south_to_east);
                            if kx > rail_in + arc {
                                let _ = write!(d, "H{}", num(kx));
                            }
                            let _ = write!(d, "M{}", Point(kx + kid.width, ky));
                            if kx > rail_in + arc {
                                let _ = write!(d, "H{}", num(rail_out - arc));
                            }
                            d.push_str(&self.east_to_north);
                            if last {
                                let _ = write!(d, "V{}{}", num(y + arc), self.north_to_east);
                            }
                        }
                        stack.push(kids[i]);
                        at.push(ky);
                        at.push(kx);
                    }
                    // Reversed, so that the branches come off the stack top
                    // to bottom.
                    reverse(&mut stack, from);
                    reverse(&mut at, 2 * from);
                }
                Kind::Bypass => {
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
                        num(y - laid.reach + arc),
                        self.north_to_east,
                        num(right - 2.0 * arc),
                        self.east_to_south,
                        num(y - arc),
                        self.south_to_east
                    );
                    stack.push(kids[0]);
                    at.push(kx);
                    at.push(y);
                }
                Kind::Loop => {
                    let kid = &boxes[kids[0]];
                    let kx = x + (laid.width - kid.width) / 2.0;
                    let loop_y = y + laid.reach;
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
                    if !laid.text.is_empty() {
                        let _ = write!(
                            self.nodes,
                            "<text class=\"{p}-edge-label {p}-count\" x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                            num(middle),
                            num(loop_y + COUNT_SIZE + 3.0),
                            js::Number(COUNT_SIZE),
                            Esc(&laid.text)
                        );
                    }
                    stack.push(kids[0]);
                    at.push(kx);
                    at.push(y);
                }
                Kind::Terminal | Kind::NonTerminal | Kind::Special => {
                    let text = &laid.text;
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
                    let _ = match laid.kind {
                        Kind::Terminal => write!(
                            nodes,
                            "<g class=\"{p}-node {p}-terminal\">{shape} rx=\"{}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"{stroke}/>{label} font-family=\"{FONT_MONO}\">{}</text></g>",
                            num(self.box_top),
                            Esc(text)
                        ),
                        Kind::NonTerminal => write!(
                            nodes,
                            "<g class=\"{p}-node {p}-nonterminal\" data-id=\"{}\">{shape} rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"{stroke}/>{label}>{}</text></g>",
                            EscText(&laid.id),
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

fn draw(model: &RailroadDb, config: &Config, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
    let base = ctx.options.font_size;
    let font_size = option(config, "fontSize", 0.0, 200.0);
    let size = if js::truthy(font_size) {
        font_size
    } else {
        js::round(base * 0.875)
    };
    let head_size = if js::truthy(font_size) {
        font_size
    } else {
        base
    };
    let padding = option(config, "padding", -1.0, 200.0);
    let line_height = js::round(size * 1.5);
    let measurer = ctx.host;
    let m = Metrics {
        measure: measurer,
        size,
        padding: if padding < 0.0 { 12.0 } else { padding },
        height: even(line_height + if padding < 0.0 { 9.0 } else { 2.0 * padding }),
        gap: option(config, "horizontalSeparation", 16.0, 500.0).min(if tight {
            12.0
        } else {
            f64::INFINITY
        }),
        sep: option(config, "verticalSeparation", 10.0, 500.0),
        arc: option(config, "arcRadius", 10.0, 200.0).min(if tight { 6.0 } else { f64::INFINITY }),
    };
    let show_markers = config
        .section("railroad")
        .and_then(|section| section.get("showMarkers"));
    let radius = if show_markers == Some(&Value::Bool(false)) {
        0.0
    } else {
        option(config, "markerRadius", 4.0, 50.0)
    };
    let stroke_width = option(config, "strokeWidth", 0.0, 20.0);
    let stroke = if stroke_width > 0.0 {
        format!(" stroke-width=\"{}\"", num(stroke_width))
    } else {
        String::new()
    };
    let pad = ctx.options.padding.unwrap_or(8.0);
    let arc = m.arc;

    let a = num(arc);
    let turn = |sweep: u8, dx: &str, dy: &str| format!("a{a},{a} 0 0 {sweep} {dx}{a},{dy}{a}");

    let title = layout_label(
        model.title.as_deref(),
        false,
        m.measure,
        base,
        if tight { title_room(ctx) } else { 4000.0 },
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
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
    for rule in &model.rules {
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
        let laid = measure(rule.definition, &model.nodes, &m, room, &mut boxes);
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
        drawn.place(&boxes, laid, left, baseline);
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
        width =
            width
                .max(end + 2.0 * radius - pad)
                .max(m.measure.width(&name, head_size, Style::BOLD));
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
        LabelSvg(
            &title,
            total_width / 2.0,
            pad + title.height / 2.0,
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
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}
