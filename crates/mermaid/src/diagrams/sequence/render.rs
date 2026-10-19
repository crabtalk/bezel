//! Pele's `diagrams/sequence/render.ts`.

use std::{collections::HashSet, fmt::Write as _};

use indexmap::IndexMap;

use super::{
    actors::{ActorSize, actor_kind, actor_size, draw_actor, is_figure},
    db::SequenceModel,
    types::{SeqActor, SeqMessageText, line, placement},
};
use crate::Ctx;
use crate::{
    Config, Rendered,
    config::{Map, Value},
    diagrams::common::{
        fit_width::{tighten, title_room},
        literal_label::break_label,
    },
    js,
    svg::{
        builder::{esc, esc_text, label_svg, num},
        edges::{ARROW, marker},
        root::{Accessible, svg_document},
        theme::{RADIUS, class_names, resolve_style},
    },
    text::{Host, Label, Style, layout_label},
};

const MSG_PAD: f64 = 12.0;
const SELF_W: f64 = 28.0;
const SELF_H: f64 = 22.0;
const ACT_W: f64 = 10.0;
const ACT_STEP: f64 = 4.0;
const ACT_MIN: f64 = 14.0;
const NOTE_GAP: f64 = 10.0;
const NOTE_OVER: f64 = 16.0;
const FRAME_PAD: f64 = 20.0;
const FRAME_NEST: f64 = 10.0;
const TAB_H: f64 = 20.0;
const FRAME_TEXT: f64 = 240.0;
const BADGE_H: f64 = 16.0;
const CENTRAL_R: f64 = 4.0;
const DASH: &str = " stroke-dasharray=\"4 4\"";

#[derive(Clone, Copy, PartialEq)]
enum Head {
    None,
    Arrow,
    Cross,
    Open,
    HalfSolid,
    HalfStick,
}

#[derive(Clone, Copy)]
struct LineStyle {
    dotted: bool,
    head: Head,
    /// Where the heads are: at the receiver, at the sender only (the reverse
    /// arrows), or at both.
    at_start: bool,
    at_end: bool,
    bottom: bool,
}

fn style(dotted: bool, head: Head, ends: u8, bottom: bool) -> LineStyle {
    LineStyle {
        dotted,
        head,
        at_start: ends != 1,
        at_end: ends != 0,
        bottom,
    }
}

fn line_style(kind: i32) -> Option<LineStyle> {
    Some(match kind {
        line::SOLID => style(false, Head::Arrow, 1, false),
        line::DOTTED => style(true, Head::Arrow, 1, false),
        line::SOLID_CROSS => style(false, Head::Cross, 1, false),
        line::DOTTED_CROSS => style(true, Head::Cross, 1, false),
        line::SOLID_OPEN => style(false, Head::None, 1, false),
        line::DOTTED_OPEN => style(true, Head::None, 1, false),
        line::SOLID_POINT => style(false, Head::Open, 1, false),
        line::DOTTED_POINT => style(true, Head::Open, 1, false),
        line::BIDIRECTIONAL_SOLID => style(false, Head::Arrow, 2, false),
        line::BIDIRECTIONAL_DOTTED => style(true, Head::Arrow, 2, false),
        _ => {
            let (dotted, base) = match kind {
                line::SOLID_TOP..=line::STICK_ARROW_BOTTOM_REVERSE => (false, kind),
                line::SOLID_TOP_DOTTED..=line::STICK_ARROW_BOTTOM_REVERSE_DOTTED => {
                    (true, kind - 10)
                }
                _ => return None,
            };
            match base {
                line::SOLID_TOP => style(dotted, Head::HalfSolid, 1, false),
                line::SOLID_BOTTOM => style(dotted, Head::HalfSolid, 1, true),
                line::STICK_TOP => style(dotted, Head::HalfStick, 1, false),
                line::STICK_BOTTOM => style(dotted, Head::HalfStick, 1, true),
                line::SOLID_ARROW_TOP_REVERSE => style(dotted, Head::HalfSolid, 0, false),
                line::SOLID_ARROW_BOTTOM_REVERSE => style(dotted, Head::HalfSolid, 0, true),
                line::STICK_ARROW_TOP_REVERSE => style(dotted, Head::HalfStick, 0, false),
                _ => style(dotted, Head::HalfStick, 0, true),
            }
        }
    })
}

fn frame_kind(kind: i32) -> Option<&'static str> {
    Some(match kind {
        line::LOOP_START => "loop",
        line::ALT_START => "alt",
        line::OPT_START => "opt",
        line::PAR_START | line::PAR_OVER_START => "par",
        line::CRITICAL_START => "critical",
        line::BREAK_START => "break",
        line::RECT_START => "rect",
        _ => return None,
    })
}

pub struct Numbering {
    /// The number each message line carries, whether or not it is shown. NaN
    /// for everything else.
    pub index: Vec<f64>,
    pub shown: Vec<bool>,
    /// Whether numbers are on after the last statement.
    pub visible: bool,
}

pub fn is_line_message(kind: Option<i32>) -> bool {
    kind.and_then(line_style).is_some()
}

/// Counts message lines the way Mermaid's renderer does while it draws.
pub fn number_messages(messages: &[super::types::SeqMessage], always: bool) -> Numbering {
    let mut index = vec![f64::NAN; messages.len()];
    let mut shown = vec![false; messages.len()];
    let mut current = 1.0;
    let mut step = 1.0;
    let mut visible = false;
    for (i, m) in messages.iter().enumerate() {
        if m.kind == Some(line::AUTONUMBER) {
            if let SeqMessageText::Numbering(spec) = &m.message {
                current = spec.start.filter(|&s| js::truthy(s)).unwrap_or(current);
                step = spec.step.filter(|&s| js::truthy(s)).unwrap_or(step);
                visible = spec.visible;
            }
        } else if is_line_message(m.kind) {
            index[i] = current;
            shown[i] = visible || always;
            current = js::round((current + step) * 100.0) / 100.0;
        }
    }
    Numbering {
        index,
        shown,
        visible,
    }
}

struct Column<'m> {
    id: &'m str,
    actor: &'m SeqActor,
    kind: &'static str,
    label: Label,
    size: ActorSize,
    x: f64,
    create_at: i64,
    destroy_at: i64,
    top: f64,
    life_start: f64,
    life_end: f64,
    activations: Vec<f64>,
}

#[derive(Clone, Copy, PartialEq)]
enum V {
    Line,
    SelfLine,
    Note,
}

struct View {
    kind: V,
    a: usize,
    b: usize,
    label: Label,
    x: f64,
    w: f64,
}

struct Frame {
    kind: &'static str,
    x0: f64,
    x1: f64,
    top: f64,
    title: Label,
    tab_w: f64,
    fill: String,
    dividers: Vec<usize>,
    divider_y: Vec<f64>,
    divider_labels: Vec<Label>,
}

struct BoxRun {
    box_: usize,
    first: usize,
    last: usize,
    title: Label,
    grow: f64,
}

fn option(conf: &Map, key: &str, fallback: f64) -> f64 {
    match conf.get(key) {
        Some(Value::Number(value)) if *value >= 0.0 && *value <= 2000.0 => *value,
        _ => fallback,
    }
}

fn half(x: f64, y: f64, dir: f64, up: f64, stick: bool) -> String {
    let p = crate::PREFIX;
    let bx = num(x - dir * ARROW);
    if stick {
        return format!(
            "<path class=\"{p}-marker\" d=\"M{},{}L{bx},{}\"/>",
            num(x),
            num(y),
            num(y + up * 5.0)
        );
    }
    format!(
        "<path class=\"{p}-marker\" d=\"M{},{}L{bx},{}V{}Z\" fill=\"var(--_l)\" stroke=\"none\"/>",
        num(x),
        num(y),
        num(y + up * 4.5),
        num(y)
    )
}

/// Draws the head whose tip is at (x, y) and points along dir. `up` is the
/// side a half head sits on.
fn head(kind: Head, x: f64, y: f64, dir: f64, up: f64) -> String {
    let p = crate::PREFIX;
    match kind {
        Head::Arrow => marker("arrow_point", x, y, dir, 0.0, "var(--_l)"),
        Head::Cross => marker("arrow_cross", x, y, dir, 0.0, "var(--_l)"),
        Head::Open => format!(
            "<path class=\"{p}-marker\" d=\"M{},{}L{},{}L{},{}\" stroke-linejoin=\"round\"/>",
            num(x - dir * ARROW),
            num(y - 4.5),
            num(x),
            num(y),
            num(x - dir * ARROW),
            num(y + 4.5)
        ),
        Head::HalfSolid => half(x, y, dir, up, false),
        Head::HalfStick => half(x, y, dir, up, true),
        Head::None => String::new(),
    }
}

fn trim(kind: Head) -> f64 {
    match kind {
        Head::Arrow => ARROW - 1.0,
        Head::Cross => 4.0,
        _ => 0.0,
    }
}

pub fn render_sequence(db: &SequenceModel, config: &Config, ctx: &Ctx<'_, impl Host>) -> Rendered {
    tighten(ctx, |tight| draw(db, config, ctx, tight))
}

/// The drawing's horizontal extent and what it has drawn so far.
struct Out {
    min_x: f64,
    max_x: f64,
    /// Shaded blocks and activations close from the inside out. They are
    /// written in reverse, so that the inner ones end up on top.
    rects: Vec<String>,
    activations: Vec<String>,
    frames: String,
    edges: String,
    labels: String,
    notes: String,
    actors: String,
    crosses: String,
}

impl Out {
    fn extend(&mut self, x0: f64, x1: f64) {
        if x0 < self.min_x {
            self.min_x = x0;
        }
        if x1 > self.max_x {
            self.max_x = x1;
        }
    }

    fn end_activation(&mut self, col: &mut Column, end: f64) -> f64 {
        let p = crate::PREFIX;
        let start = col.activations.pop().unwrap_or(end);
        let bottom = end.max(start + ACT_MIN);
        let x = col.x - ACT_W / 2.0 + col.activations.len() as f64 * ACT_STEP;
        self.extend(x, x + ACT_W);
        self.activations.push(format!(
            "<rect class=\"{p}-activation\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\"/>",
            esc_text(col.id),
            num(x),
            num(start),
            js::num_str(ACT_W),
            num(bottom - start)
        ));
        bottom
    }

    /// The lifeline of a destroyed participant ends in a cross a little below
    /// the message that ends it.
    fn destroy(&mut self, col: &mut Column, line_y: f64) -> f64 {
        let p = crate::PREFIX;
        let cx = col.x;
        let cy = line_y + 16.0;
        col.life_end = cy;
        let _ = write!(
            self.crosses,
            "<path class=\"{p}-destroy\" data-id=\"{}\" d=\"M{},{}L{},{}M{},{}L{},{}\"/>",
            esc_text(col.id),
            num(cx - 6.0),
            num(cy - 6.0),
            num(cx + 6.0),
            num(cy + 6.0),
            num(cx + 6.0),
            num(cy - 6.0),
            num(cx - 6.0),
            num(cy + 6.0)
        );
        cy + 6.0
    }
}

fn actor_svg(col: &Column, top: f64, footer: bool) -> String {
    let p = crate::PREFIX;
    let cls = match col.actor.properties.get("class") {
        Some(Value::String(cls)) => class_names(cls),
        _ => String::new(),
    };
    format!(
        "<g class=\"{p}-node {p}-actor {p}-actor-{}{}{cls}\" data-id=\"{}\">{}</g>",
        col.kind,
        if footer {
            format!(" {p}-actor-footer")
        } else {
            String::new()
        },
        esc_text(col.id),
        draw_actor(col.kind, col.x, top, col.size, &col.label)
    )
}

fn left_edge(col: &Column, extra: usize) -> f64 {
    if col.activations.len() + extra > 0 {
        col.x - ACT_W / 2.0
    } else {
        col.x
    }
}

fn right_edge(col: &Column, extra: usize) -> f64 {
    let depth = col.activations.len() + extra;
    if depth > 0 {
        col.x + ACT_W / 2.0 + (depth - 1) as f64 * ACT_STEP
    } else {
        col.x
    }
}

fn draw(db: &SequenceModel, config: &Config, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
    let p = crate::PREFIX;
    let empty = Map::new();
    let conf = config.section("sequence").unwrap_or(&empty);
    let size = ctx.options.font_size;
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.75);
    let measurer = ctx.host;
    let pad = ctx.options.padding.unwrap_or(8.0);
    let flag = |key: &str| conf.get(key).and_then(Value::as_bool);
    let mirror = flag("mirrorActors") != Some(false);
    let right_angles = flag("rightAngles") == Some(true);
    let actor_margin =
        option(conf, "actorMargin", 40.0).min(if tight { 8.0 } else { f64::INFINITY });
    let min_w = option(conf, "width", 64.0).min(if tight { 48.0 } else { f64::INFINITY });
    let min_h = option(conf, "height", js::round(size * 1.5) + 20.0);
    let wrap_w = option(conf, "width", 160.0).min(if tight {
        32f64.max(
            (ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad) / 1f64.max(db.actors.len() as f64)
                - actor_margin
                - 24.0,
        )
    } else {
        f64::INFINITY
    });
    // Mermaid's default of 35 corresponds to Pele's default gap. Other values
    // move it by the difference.
    let msg_gap = 4f64.max(14.0 + option(conf, "messageMargin", 35.0) - 35.0);
    let note_pad = option(conf, "noteMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let box_pad = option(conf, "boxMargin", 10.0).min(if tight { 6.0 } else { f64::INFINITY });
    let align = match conf.get("messageAlign").and_then(Value::as_str) {
        Some("left") => -1,
        Some("right") => 1,
        _ => 0,
    };

    let msg_pad = if tight { 6.0 } else { MSG_PAD };
    let note_over = if tight { 8.0 } else { NOTE_OVER };
    let frame_pad = if tight { 12.0 } else { FRAME_PAD };
    let frame_nest = if tight { 6.0 } else { FRAME_NEST };
    let messages = &db.messages;
    let count = messages.len();
    let numbering = number_messages(messages, flag("showSequenceNumbers") == Some(true));
    let text = |raw: Option<&str>, font_size: f64, wrap: bool, width: f64, base: u8| -> Label {
        let label = layout_label(
            raw,
            false,
            measurer,
            font_size,
            if tight || wrap { width } else { f64::INFINITY },
            base,
        );
        if tight {
            break_label(&label, measurer, 120f64.max(width))
        } else {
            label
        }
    };

    let used: Option<HashSet<&str>> = (flag("hideUnusedParticipants") == Some(true)).then(|| {
        messages
            .iter()
            .flat_map(|m| [m.from.as_deref(), m.to.as_deref()])
            .flatten()
            .collect()
    });

    let mut next_line = vec![0i64; count + 1];
    next_line[count] = -1;
    for i in (0..count).rev() {
        next_line[i] = if is_line_message(messages[i].kind) {
            i as i64
        } else {
            next_line[i + 1]
        };
    }

    let mut cols: Vec<Column> = Vec::new();
    let mut col_of: IndexMap<&str, usize> = IndexMap::new();
    for (id, actor) in &db.actors {
        if used
            .as_ref()
            .is_some_and(|used| !used.contains(id.as_str()))
        {
            continue;
        }
        let kind = actor_kind(&actor.kind);
        let label = text(Some(&actor.description), size, actor.wrap, wrap_w, 0);
        let mut create_at = db
            .created_actors
            .get(id)
            .map_or(-1, |&created| next_line[created.min(count)]);
        if create_at >= 0 && messages[create_at as usize].to.as_deref() != Some(id) {
            create_at = -1;
        }
        let mut destroy_at = db
            .destroyed_actors
            .get(id)
            .map_or(-1, |&destroyed| next_line[destroyed.min(count)]);
        if destroy_at >= 0 {
            let m = &messages[destroy_at as usize];
            if m.to.as_deref() != Some(id) && m.from.as_deref() != Some(id) {
                destroy_at = -1;
            }
        }
        col_of.insert(id, cols.len());
        let size = actor_size(kind, &label, min_w, min_h, tight.then_some(8.0));
        cols.push(Column {
            id,
            actor,
            kind,
            label,
            size,
            x: 0.0,
            create_at,
            destroy_at,
            top: 0.0,
            life_start: 0.0,
            life_end: -1.0,
            activations: Vec::new(),
        });
    }
    let n_cols = cols.len() as i64;
    let last = n_cols - 1;

    // Widths between lifelines. Every message and note says how far apart the
    // lifelines it touches must be.
    let mut needs: IndexMap<i64, f64> = IndexMap::new();
    let mut need = |lo: i64, hi: i64, width: f64| {
        if lo < 0 || hi > last {
            return;
        }
        let key = lo * n_cols + hi;
        if needs.get(&key).copied().unwrap_or(0.0) < width {
            needs.insert(key, width);
        }
    };
    let badge_width = |i: usize| -> f64 {
        if numbering.shown[i] {
            BADGE_H.max(measurer.width(&js::num_str(numbering.index[i]), tiny, 0) + 8.0)
        } else {
            0.0
        }
    };

    let mut views: Vec<Option<View>> = (0..count).map(|_| None).collect();
    for (i, m) in messages.iter().enumerate() {
        let a = m.from.as_deref().and_then(|from| col_of.get(from).copied());
        let b = m.to.as_deref().and_then(|to| col_of.get(to).copied());
        let (Some(a), Some(b), Some(message)) = (a, b, m.message.text()) else {
            continue;
        };
        let (ai, bi) = (a as i64, b as i64);
        if m.kind == Some(line::NOTE) {
            let label = text(Some(message), small, m.wrap, wrap_w, 0);
            let w = label.width + 2.0 * note_pad;
            if m.placement == Some(placement::LEFTOF) {
                need(ai - 1, ai, w + 2.0 * NOTE_GAP + ACT_W);
            } else if m.placement == Some(placement::RIGHTOF) {
                need(ai, ai + 1, w + 2.0 * NOTE_GAP + ACT_W);
            } else if a == b {
                need(ai - 1, ai, w / 2.0 + NOTE_GAP);
                need(ai, ai + 1, w / 2.0 + NOTE_GAP);
            } else {
                need(ai.min(bi), ai.max(bi), w - 2.0 * note_over);
            }
            views[i] = Some(View {
                kind: V::Note,
                a,
                b,
                label,
                x: 0.0,
                w,
            });
        } else if is_line_message(m.kind) {
            let label = text(Some(message), small, m.wrap, wrap_w, 0);
            let badge = badge_width(i);
            if a == b {
                need(
                    ai,
                    ai + 1,
                    SELF_W + 8.0 + label.width + 16.0 + ACT_W + badge,
                );
                views[i] = Some(View {
                    kind: V::SelfLine,
                    a,
                    b,
                    label,
                    x: 0.0,
                    w: 0.0,
                });
            } else {
                let created = if cols[b].create_at == i as i64 {
                    cols[b].size.w / 2.0
                } else {
                    0.0
                };
                let central = if m.central_connection.is_some_and(|c| c != 0) {
                    4.0 * CENTRAL_R
                } else {
                    0.0
                };
                let label_w = if tight {
                    label.width.min(16f64.max(wrap_w - 16.0))
                } else {
                    label.width
                };
                need(
                    ai.min(bi),
                    ai.max(bi),
                    label_w + 2.0 * msg_pad + ACT_W + badge + created + central,
                );
                views[i] = Some(View {
                    kind: V::Line,
                    a,
                    b,
                    label,
                    x: 0.0,
                    w: 0.0,
                });
            }
        }
    }

    let mut runs: Vec<BoxRun> = Vec::new();
    for (i, col) in cols.iter().enumerate() {
        let Some(box_) = col.actor.box_ else {
            continue;
        };
        if let Some(run) = runs.last_mut()
            && run.box_ == box_
            && run.last + 1 == i
        {
            run.last = i;
        } else {
            let b = &db.boxes[box_];
            runs.push(BoxRun {
                box_,
                first: i,
                last: i,
                title: text(b.name.as_deref(), small, b.wrap, wrap_w, 0),
                grow: 0.0,
            });
        }
    }

    let n_gaps = last.max(0) as usize;
    let mut gaps = vec![0.0; n_gaps];
    for i in 0..n_gaps {
        let left = &cols[i];
        let right = &cols[i + 1];
        let boxed = if left.actor.box_ != right.actor.box_ {
            (if left.actor.box_.is_some() {
                box_pad
            } else {
                0.0
            }) + if right.actor.box_.is_some() {
                box_pad
            } else {
                0.0
            }
        } else {
            0.0
        };
        let key = i as i64 * n_cols + i as i64 + 1;
        gaps[i] = (left.size.w / 2.0 + right.size.w / 2.0 + actor_margin + boxed)
            .max(needs.get(&key).copied().unwrap_or(0.0));
    }
    // A message across several lifelines widens each gap it crosses by an
    // equal share of what is missing.
    let mut spans: Vec<(usize, usize, f64)> = Vec::new();
    for (&key, &width) in &needs {
        let lo = key / n_cols;
        let hi = key % n_cols;
        if hi - lo > 1 {
            spans.push((lo as usize, hi as usize, width));
        }
    }
    spans.sort_by(|p, q| (p.1 - p.0).cmp(&(q.1 - q.0)).then(p.0.cmp(&q.0)));
    for &(lo, hi, width) in &spans {
        let mut total = 0.0;
        for gap in &gaps[lo..hi] {
            total += gap;
        }
        if total >= width {
            continue;
        }
        let share = (width - total) / (hi - lo) as f64;
        for gap in &mut gaps[lo..hi] {
            *gap += share;
        }
    }
    let place = |cols: &mut Vec<Column>, gaps: &[f64]| {
        for i in 1..cols.len() {
            cols[i].x = cols[i - 1].x + gaps[i - 1];
        }
    };
    place(&mut cols, &gaps);
    for run in &mut runs {
        let inner = cols[run.last].x + cols[run.last].size.w / 2.0
            - (cols[run.first].x - cols[run.first].size.w / 2.0);
        run.grow = 0f64.max((run.title.width - inner) / 2.0);
        if run.grow == 0.0 {
            continue;
        }
        if run.first > 0 {
            gaps[run.first - 1] += run.grow;
        }
        if (run.last as i64) < last {
            gaps[run.last] += run.grow;
        }
    }
    place(&mut cols, &gaps);

    let mut out = Out {
        min_x: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        rects: Vec::new(),
        activations: Vec::new(),
        frames: String::new(),
        edges: String::new(),
        labels: String::new(),
        notes: String::new(),
        actors: String::new(),
        crosses: String::new(),
    };
    for col in &cols {
        out.extend(col.x - col.size.w / 2.0, col.x + col.size.w / 2.0);
    }
    for run in &runs {
        out.extend(
            cols[run.first].x - cols[run.first].size.w / 2.0 - box_pad - run.grow,
            cols[run.last].x + cols[run.last].size.w / 2.0 + box_pad + run.grow,
        );
    }

    // Horizontal extents of every block, from what it holds.
    let mut arena: Vec<Frame> = Vec::new();
    let mut frames: Vec<Option<usize>> = vec![None; count];
    let mut open: Vec<usize> = Vec::new();
    let span_all = || -> (f64, f64) {
        if last < 0 {
            (0.0, 120.0)
        } else {
            (cols[0].x - frame_pad, cols[last as usize].x + frame_pad)
        }
    };
    for (i, m) in messages.iter().enumerate() {
        let kind = m.kind;
        let frame_kind = kind.and_then(frame_kind);
        if let Some(frame_kind) = frame_kind {
            arena.push(Frame {
                kind: frame_kind,
                x0: f64::INFINITY,
                x1: f64::NEG_INFINITY,
                top: 0.0,
                title: Label::default(),
                tab_w: 0.0,
                fill: match (frame_kind, m.message.text()) {
                    ("rect", Some(text)) => text.to_string(),
                    _ => String::new(),
                },
                dividers: vec![i],
                divider_y: Vec::new(),
                divider_labels: Vec::new(),
            });
            frames[i] = Some(arena.len() - 1);
            open.push(arena.len() - 1);
        } else if matches!(
            kind,
            Some(line::ALT_ELSE | line::PAR_AND | line::CRITICAL_OPTION)
        ) {
            if let Some(&top) = open.last() {
                arena[top].dividers.push(i);
            }
        } else if matches!(
            kind,
            Some(
                line::LOOP_END
                    | line::ALT_END
                    | line::OPT_END
                    | line::PAR_END
                    | line::CRITICAL_END
                    | line::BREAK_END
                    | line::RECT_END
            )
        ) {
            let Some(index) = open.pop() else {
                continue;
            };
            frames[i] = Some(index);
            let frame = &mut arena[index];
            if frame.x0 > frame.x1 {
                (frame.x0, frame.x1) = span_all();
            }
            if frame.kind != "rect" {
                frame.tab_w = measurer.width(frame.kind, tiny, Style::BOLD) + 14.0;
                // A block grows to the right to give its texts up to
                // FRAME_TEXT of width before they wrap.
                for k in 0..frame.dividers.len() {
                    let raw = messages[frame.dividers[k]].message.text().unwrap_or("");
                    let tab = if k == 0 { frame.tab_w + 8.0 } else { 0.0 };
                    let room = frame.x1 - frame.x0 - 16.0 - tab;
                    let label = if raw.is_empty() {
                        Label::default()
                    } else {
                        text(
                            Some(&format!("[{raw}]")),
                            small,
                            true,
                            room.max(if tight {
                                FRAME_TEXT.min(wrap_w)
                            } else {
                                FRAME_TEXT
                            }),
                            0,
                        )
                    };
                    if label.width > room {
                        frame.x1 += label.width - room;
                    }
                    if k == 0 {
                        frame.title = label;
                    } else {
                        frame.divider_labels.push(label);
                    }
                }
            }
            let (x0, x1) = (frame.x0, frame.x1);
            out.extend(x0, x1);
            if let Some(&parent) = open.last() {
                let parent = &mut arena[parent];
                parent.x0 = parent.x0.min(x0 - frame_nest);
                parent.x1 = parent.x1.max(x1 + frame_nest);
            }
        } else if let Some(view) = &mut views[i] {
            let a = &cols[view.a];
            let b = &cols[view.b];
            let (mut x0, mut x1);
            if view.kind == V::Note {
                if m.placement == Some(placement::LEFTOF) {
                    view.x = a.x - NOTE_GAP - view.w;
                } else if m.placement == Some(placement::RIGHTOF) {
                    view.x = a.x + NOTE_GAP;
                } else if view.a == view.b {
                    view.x = a.x - view.w / 2.0;
                } else {
                    let lo = a.x.min(b.x) - note_over;
                    let hi = a.x.max(b.x) + note_over;
                    if tight || m.wrap {
                        view.label = text(
                            m.message.text(),
                            small,
                            true,
                            wrap_w.max(hi - lo - 2.0 * note_pad),
                            0,
                        );
                    }
                    view.w = (hi - lo).max(view.label.width + 2.0 * note_pad);
                    view.x = (lo + hi - view.w) / 2.0;
                }
                x0 = view.x;
                x1 = view.x + view.w;
            } else if view.kind == V::SelfLine {
                x0 = a.x;
                x1 = a.x + ACT_W + SELF_W + 8.0 + view.label.width + badge_width(i);
            } else {
                x0 = a.x.min(b.x);
                x1 = a.x.max(b.x);
                if tight || m.wrap {
                    view.label = text(
                        m.message.text(),
                        small,
                        true,
                        wrap_w.max(x1 - x0 - 2.0 * msg_pad - ACT_W),
                        0,
                    );
                }
                if b.create_at == i as i64 {
                    if b.x > a.x {
                        x1 += b.size.w / 2.0;
                    } else {
                        x0 -= b.size.w / 2.0;
                    }
                }
            }
            out.extend(x0, x1);
            if let Some(&top) = open.last() {
                let frame = &mut arena[top];
                frame.x0 = frame.x0.min(x0 - frame_pad);
                frame.x1 = frame.x1.max(x1 + frame_pad);
            }
        }
    }

    // Vertical placement, top to bottom, drawing as it goes.
    let mut box_title_h: f64 = 0.0;
    for run in &runs {
        box_title_h = box_title_h.max(run.title.height);
    }
    let box_top = if runs.is_empty() {
        0.0
    } else {
        box_pad
            + if box_title_h > 0.0 {
                box_title_h + 4.0
            } else {
                0.0
            }
    };
    let mut row_h: f64 = 0.0;
    for col in &cols {
        if col.create_at < 0 {
            row_h = row_h.max(col.size.h);
        }
    }
    let head_bottom = box_top + row_h;
    for col in &mut cols {
        col.top = head_bottom - col.size.h;
        col.life_start = head_bottom;
    }

    let xs: Vec<f64> = cols.iter().map(|col| col.x).collect();
    // Whether a lifeline runs between two x positions, where text would be
    // struck through.
    let crosses = |x0: f64, x1: f64| -> bool {
        let mut lo = 0;
        let mut hi = xs.len();
        while lo < hi {
            let mid = (lo + hi) >> 1;
            if xs[mid] <= x0 {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo < xs.len() && xs[lo] < x1
    };
    let backed = |label: &Label, cx: f64, cy: f64, attrs: &str| -> String {
        if label.width == 0.0 {
            return String::new();
        }
        let l = cx - label.width / 2.0;
        let back = if crosses(l, l + label.width) {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\" stroke=\"none\"/>",
                num(l - 3.0),
                num(cy - label.height / 2.0 + 2.0),
                num(label.width + 6.0),
                num(label.height - 4.0)
            )
        } else {
            String::new()
        };
        back + &label_svg(label, cx, cy, attrs)
    };
    let badge = |i: usize, cx: f64, cy: f64, w: f64| -> String {
        format!(
            "<g class=\"{p}-sequence-number\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"var(--_l)\" stroke=\"none\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_bg)\" stroke=\"none\">{}</text></g>",
            num(cx - w / 2.0),
            num(cy - BADGE_H / 2.0),
            num(w),
            js::num_str(BADGE_H),
            js::num_str(BADGE_H / 2.0),
            num(cx),
            num(cy + tiny * 0.35),
            js::num_str(tiny),
            esc(&js::num_str(numbering.index[i]))
        )
    };
    let frame_label = format!(" class=\"{p}-frame-label\" fill=\"var(--_m)\" stroke=\"none\"");

    let mut y = head_bottom + 6.0;
    let mut stack: Vec<usize> = Vec::new();
    for (i, m) in messages.iter().enumerate() {
        let kind = m.kind;
        let frame = frames[i];

        if kind == Some(line::ACTIVE_START) {
            if let Some(col) = m.from.as_deref().and_then(|from| col_of.get(from)) {
                cols[*col].activations.push(y);
            }
        } else if kind == Some(line::ACTIVE_END) {
            if let Some(&col) = m.from.as_deref().and_then(|from| col_of.get(from))
                && !cols[col].activations.is_empty()
            {
                y = y.max(out.end_activation(&mut cols[col], y));
            }
        } else if let Some(index) = frame.filter(|&index| arena[index].dividers[0] == i) {
            stack.push(index);
            let frame = &mut arena[index];
            if frame.kind == "rect" {
                frame.top = y + 8.0;
                y = frame.top;
            } else {
                frame.top = y + 12.0;
                y = frame.top + TAB_H.max(frame.title.height + 2.0);
            }
        } else if let Some(index) = frame {
            stack.pop();
            let frame = &arena[index];
            let bottom = y + 12.0;
            let x = num(frame.x0);
            let w = num(frame.x1 - frame.x0);
            if frame.kind == "rect" {
                let fill = if frame.fill.is_empty() {
                    String::new()
                } else {
                    resolve_style(&[format!("fill:{}", frame.fill)]).shape
                };
                out.rects.push(format!(
                    "<rect class=\"{p}-rect\" x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\" rx=\"{RADIUS}\"{}{fill}/>",
                    num(frame.top),
                    num(bottom - frame.top),
                    if fill.is_empty() {
                        " fill-opacity=\"0.5\""
                    } else {
                        ""
                    }
                ));
            } else {
                let mut inner = format!(
                    "<rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"none\"/>",
                    num(frame.top),
                    num(bottom - frame.top)
                );
                for (k, &dy) in frame.divider_y.iter().enumerate() {
                    let label = &frame.divider_labels[k];
                    let _ = write!(
                        inner,
                        "<path d=\"M{x},{}H{}\"{DASH}/>",
                        num(dy),
                        num(frame.x1)
                    );
                    inner.push_str(&backed(
                        label,
                        frame.x0 + 8.0 + label.width / 2.0,
                        dy + 2.0 + label.height / 2.0,
                        &frame_label,
                    ));
                }
                let _ = write!(
                    inner,
                    "<rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\"/><text class=\"{p}-frame-kind\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\" stroke=\"none\">{}</text>",
                    num(frame.top),
                    num(frame.tab_w),
                    js::num_str(TAB_H),
                    num(frame.x0 + frame.tab_w / 2.0),
                    num(frame.top + TAB_H / 2.0 + tiny * 0.35),
                    js::num_str(tiny),
                    frame.kind
                );
                inner.push_str(&backed(
                    &frame.title,
                    frame.x0 + frame.tab_w + 8.0 + frame.title.width / 2.0,
                    frame.top + TAB_H.max(frame.title.height) / 2.0,
                    &frame_label,
                ));
                let _ = write!(
                    out.frames,
                    "<g class=\"{p}-frame {p}-frame-{}\">{inner}</g>",
                    frame.kind
                );
            }
            y = bottom;
        } else if matches!(
            kind,
            Some(line::ALT_ELSE | line::PAR_AND | line::CRITICAL_OPTION)
        ) {
            if let Some(&current) = stack.last() {
                let current = &mut arena[current];
                let height = current
                    .divider_labels
                    .get(current.divider_y.len())
                    .map_or(0.0, |label| label.height);
                current.divider_y.push(y + 12.0);
                y += 12.0 + if height > 0.0 { height } else { 0.0 };
            }
        } else if let Some(view) = views[i].as_ref().filter(|view| view.kind == V::Note) {
            let a = &cols[view.a];
            let h = view.label.height + 2.0 * 4f64.max(note_pad - 4.0);
            let top = y + 10.0;
            let mut x = view.x;
            if m.placement == Some(placement::RIGHTOF) {
                x += right_edge(a, 0) - a.x;
            } else if m.placement == Some(placement::LEFTOF) {
                x += left_edge(a, 0) - a.x;
            }
            out.extend(x, x + view.w);
            let _ = write!(
                out.notes,
                "<g class=\"{p}-note\" data-id=\"i{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"/>{}</g>",
                esc_text(&m.id),
                num(x),
                num(top),
                num(view.w),
                num(h),
                label_svg(
                    &view.label,
                    x + view.w / 2.0,
                    top + h / 2.0,
                    &format!(" class=\"{p}-label\" stroke=\"none\" fill=\"var(--_fg)\"")
                )
            );
            y = top + h;
        } else if let Some(view) = &views[i] {
            let st = kind
                .and_then(line_style)
                .expect("a message line has a style");
            let (va, vb) = (view.a, view.b);
            let label = &view.label;
            let bw = badge_width(i);
            // The activation a `+` starts belongs to this message, so the
            // arrow stops at its edge.
            let mut starting = 0;
            for later in &messages[i + 1..] {
                let t = later.kind;
                if t == Some(line::ACTIVE_START) {
                    if later.from == m.to {
                        starting += 1;
                    }
                } else if !matches!(
                    t,
                    Some(
                        line::ACTIVE_END
                            | line::CENTRAL_CONNECTION
                            | line::CENTRAL_CONNECTION_REVERSE
                    )
                ) {
                    break;
                }
            }
            let up = if st.bottom { 1.0 } else { -1.0 };
            let attrs = if st.dotted { DASH } else { "" };
            let cc = m.central_connection;
            let circle_to = matches!(
                cc,
                Some(line::CENTRAL_CONNECTION | line::CENTRAL_CONNECTION_DUAL)
            );
            let circle_from = matches!(
                cc,
                Some(line::CENTRAL_CONNECTION_REVERSE | line::CENTRAL_CONNECTION_DUAL)
            );
            let circle = |cx: f64, cy: f64| -> String {
                format!(
                    "<circle class=\"{p}-central\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\" stroke=\"none\"/>",
                    num(cx),
                    num(cy),
                    js::num_str(CENTRAL_R)
                )
            };
            let tail_at_end = st.at_start && !st.at_end;
            let head_trim = trim(st.head);
            let mut inner = String::new();
            let line_y;

            if view.kind == V::SelfLine {
                let a = &cols[va];
                let top = y + msg_gap + 0f64.max((label.height - SELF_H) / 2.0);
                let bottom = top + SELF_H;
                let mut x0 = if circle_from {
                    a.x + CENTRAL_R
                } else {
                    right_edge(a, 0)
                };
                let mut x1 = if circle_to {
                    a.x + CENTRAL_R
                } else {
                    right_edge(a, starting)
                };
                let reach = x0.max(x1) + SELF_W + bw / 2.0;
                if circle_from {
                    inner.push_str(&circle(a.x, top));
                }
                if circle_to {
                    inner.push_str(&circle(a.x, bottom));
                }
                if bw > 0.0 {
                    inner.push_str(&badge(
                        i,
                        if tail_at_end { x1 } else { x0 },
                        if tail_at_end { bottom } else { top },
                        bw,
                    ));
                    if tail_at_end {
                        x1 += bw / 2.0;
                    } else {
                        x0 += bw / 2.0;
                    }
                }
                // The loop leaves to the right and comes back heading left,
                // which turns a half head over.
                let heads = (if st.at_start {
                    head(st.head, x0, top, -1.0, up)
                } else {
                    String::new()
                }) + &(if st.at_end {
                    head(st.head, x1, bottom, -1.0, -up)
                } else {
                    String::new()
                });
                let from = num(x0 + if st.at_start { head_trim } else { 0.0 });
                let to = num(x1 + if st.at_end { head_trim } else { 0.0 });
                let d = if right_angles {
                    format!("M{from},{}H{}V{}H{to}", num(top), num(reach), num(bottom))
                } else {
                    format!(
                        "M{from},{}H{}A6,6 0 0 1 {},{}V{}A6,6 0 0 1 {},{}H{to}",
                        num(top),
                        num(reach - 6.0),
                        num(reach),
                        num(top + 6.0),
                        num(bottom - 6.0),
                        num(reach - 6.0),
                        num(bottom)
                    )
                };
                inner = format!("<path d=\"{d}\"{attrs}/>") + &heads + &inner;
                let ax = a.x;
                out.extend(ax, reach + 8.0 + label.width);
                if label.width > 0.0 {
                    let _ = write!(
                        out.labels,
                        "<g class=\"{p}-edge-label\" data-id=\"i{}\">{}</g>",
                        esc_text(&m.id),
                        label_svg(
                            label,
                            reach + 8.0 + label.width / 2.0,
                            (top + bottom) / 2.0,
                            ""
                        )
                    );
                }
                line_y = bottom;
            } else {
                let (ax, bx) = (cols[va].x, cols[vb].x);
                let dir = if bx > ax { 1.0 } else { -1.0 };
                let creating = cols[vb].create_at == i as i64;
                let b_size = cols[vb].size;
                line_y = y
                    + msg_gap
                    + (if label.height > 0.0 {
                        label.height + 2.0
                    } else {
                        6.0
                    })
                    .max(if creating {
                        b_size.h / 2.0 - msg_gap + 6.0
                    } else {
                        0.0
                    });
                let mut x0 = if dir > 0.0 {
                    right_edge(&cols[va], 0)
                } else {
                    left_edge(&cols[va], 0)
                };
                let mut x1 = if dir > 0.0 {
                    left_edge(&cols[vb], starting)
                } else {
                    right_edge(&cols[vb], starting)
                };
                if creating {
                    let b = &mut cols[vb];
                    b.top = line_y - b.size.h / 2.0;
                    b.life_start = b.top + b.size.h;
                    x1 = b.x
                        - dir
                            * if is_figure(b.kind) {
                                18.0
                            } else {
                                b.size.w / 2.0 + 1.0
                            };
                }
                if circle_from {
                    inner.push_str(&circle(ax, line_y));
                    x0 = ax + dir * CENTRAL_R;
                }
                if circle_to {
                    inner.push_str(&circle(bx, line_y));
                    x1 = bx - dir * CENTRAL_R;
                }
                if bw > 0.0 {
                    // The number sits on the lifeline at the tail of the
                    // arrow, past a connection circle if there is one.
                    if tail_at_end {
                        let cx = if circle_to {
                            x1 - dir * (bw / 2.0 + 2.0)
                        } else {
                            x1
                        };
                        inner.push_str(&badge(i, cx, line_y, bw));
                        x1 = cx - (dir * bw) / 2.0;
                    } else {
                        let cx = if circle_from {
                            x0 + dir * (bw / 2.0 + 2.0)
                        } else {
                            x0
                        };
                        inner.push_str(&badge(i, cx, line_y, bw));
                        x0 = cx + (dir * bw) / 2.0;
                    }
                }
                let heads = (if st.at_start {
                    head(st.head, x0, line_y, -dir, up * dir)
                } else {
                    String::new()
                }) + &(if st.at_end {
                    head(st.head, x1, line_y, dir, up * dir)
                } else {
                    String::new()
                });
                let from = x0 + if st.at_start { dir * head_trim } else { 0.0 };
                let to = x1 - if st.at_end { dir * head_trim } else { 0.0 };
                inner = format!(
                    "<path d=\"M{},{}H{}\"{attrs}/>",
                    num(from),
                    num(line_y),
                    num(to)
                ) + &heads
                    + &inner;
                if label.width > 0.0 {
                    let l = x0.min(x1);
                    let r = x0.max(x1);
                    let cx = match align {
                        -1 => l + msg_pad + label.width / 2.0,
                        1 => r - msg_pad - label.width / 2.0,
                        _ => (l + r) / 2.0,
                    };
                    out.extend(cx - label.width / 2.0, cx + label.width / 2.0);
                    let _ = write!(
                        out.labels,
                        "<g class=\"{p}-edge-label\" data-id=\"i{}\">{}</g>",
                        esc_text(&m.id),
                        backed(label, cx, line_y - 4.0 - label.height / 2.0, "")
                    );
                }
                if creating {
                    y = line_y + b_size.h / 2.0 - 6.0;
                }
            }
            let _ = write!(
                out.edges,
                "<g class=\"{p}-edge {p}-message\" data-id=\"i{}\">{inner}</g>",
                esc_text(&m.id)
            );
            y = y.max(line_y);
            if cols[va].destroy_at == i as i64 {
                y = y.max(out.destroy(&mut cols[va], line_y));
            }
            if cols[vb].destroy_at == i as i64 && vb != va {
                y = y.max(out.destroy(&mut cols[vb], line_y));
            }
        }
    }

    y += 20.0;
    for col in &mut cols {
        while !col.activations.is_empty() {
            out.end_activation(col, y);
        }
    }
    let mut bottom = y;
    if mirror {
        let mut foot_h: f64 = 0.0;
        for col in &cols {
            if col.life_end >= 0.0 {
                continue;
            }
            foot_h = foot_h.max(col.size.h);
            out.actors.push_str(&actor_svg(col, y, true));
        }
        bottom = y + foot_h;
    }
    let mut lifelines = String::new();
    for col in &cols {
        out.actors.push_str(&actor_svg(col, col.top, false));
        let end = if col.life_end >= 0.0 { col.life_end } else { y };
        if end > col.life_start {
            let _ = write!(
                lifelines,
                "<path class=\"{p}-lifeline\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
                esc_text(col.id),
                num(col.x),
                num(col.life_start),
                num(end)
            );
        }
    }

    let mut boxes_out = String::new();
    for run in &runs {
        let b = &db.boxes[run.box_];
        let x0 = cols[run.first].x - cols[run.first].size.w / 2.0 - box_pad - run.grow;
        let x1 = cols[run.last].x + cols[run.last].size.w / 2.0 + box_pad + run.grow;
        let fill = if b.fill == "transparent" {
            String::new()
        } else {
            resolve_style(&[format!("fill:{}", b.fill)]).shape
        };
        let _ = write!(
            boxes_out,
            "<g class=\"{p}-cluster {p}-box\"{}><rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}{fill}/>{}</g>",
            match b.name.as_deref().filter(|name| !name.is_empty()) {
                Some(name) => format!(" data-id=\"{}\"", esc_text(name)),
                None => String::new(),
            },
            num(x0),
            num(x1 - x0),
            num(bottom + box_pad),
            if fill.is_empty() {
                " fill-opacity=\"0.5\""
            } else {
                ""
            },
            label_svg(
                &run.title,
                (x0 + x1) / 2.0,
                box_pad / 2.0 + 2.0 + run.title.height / 2.0,
                &format!(" class=\"{p}-cluster-label\" fill=\"var(--_m)\"")
            )
        );
    }
    if !runs.is_empty() {
        bottom += box_pad;
    }

    let mut min_y = 0.0;
    let mut title_out = String::new();
    let title = text(
        db.title.as_deref(),
        size,
        false,
        if tight { title_room(ctx) } else { wrap_w },
        Style::BOLD,
    );
    if out.min_x > out.max_x {
        out.extend(0.0, title.width.max(40.0));
    }
    if title.width > 0.0 {
        let cx = (out.min_x + out.max_x) / 2.0;
        out.extend(cx - title.width / 2.0, cx + title.width / 2.0);
        min_y = -title.height - 12.0;
        title_out = label_svg(
            &title,
            cx,
            min_y + title.height / 2.0,
            &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
        );
    }

    let width = (out.max_x - out.min_x + 2.0 * pad).ceil();
    let height = (bottom - min_y + 2.0 * pad).ceil();
    let mut content = format!(
        "<g transform=\"translate({},{})\">{title_out}",
        num(pad - out.min_x),
        num(pad - min_y)
    );
    let small_str = js::num_str(small);
    if !boxes_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-boxes\" font-size=\"{small_str}\">{boxes_out}</g>"
        );
    }
    if !out.rects.is_empty() {
        out.rects.reverse();
        let _ = write!(
            content,
            "<g class=\"{p}-rects\" fill=\"var(--_a)\">{}</g>",
            out.rects.concat()
        );
    }
    if !lifelines.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-lifelines\" fill=\"none\" stroke=\"var(--_b)\">{lifelines}</g>"
        );
    }
    if !out.activations.is_empty() {
        out.activations.reverse();
        let _ = write!(
            content,
            "<g class=\"{p}-activations\" fill=\"var(--_s)\" stroke=\"var(--_b)\">{}</g>",
            out.activations.concat()
        );
    }
    if !out.frames.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-frames\" font-size=\"{small_str}\" stroke=\"var(--_b)\">{}</g>",
            out.frames
        );
    }
    if !out.edges.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edges\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\">{}</g>",
            out.edges
        );
    }
    if !out.crosses.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-destroys\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-linecap=\"round\">{}</g>",
            out.crosses
        );
    }
    if !out.labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-edge-labels\" font-size=\"{small_str}\">{}</g>",
            out.labels
        );
    }
    if !out.notes.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-notes\" font-size=\"{small_str}\" fill=\"var(--_a)\" stroke=\"var(--_b)\">{}</g>",
            out.notes
        );
    }
    if !out.actors.is_empty() {
        let _ = write!(content, "<g class=\"{p}-nodes\">{}</g>", out.actors);
    }
    content.push_str("</g>");
    let svg = svg_document(
        "sequence",
        width,
        height,
        size,
        ctx,
        Accessible {
            acc_title: db.acc_title.as_deref(),
            acc_descr: db.acc_descr.as_deref(),
        },
        &content,
    );
    Rendered {
        svg,
        width,
        height,
        links: Vec::new(),
    }
}
