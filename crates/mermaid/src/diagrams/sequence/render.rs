//! Pele's `diagrams/sequence/render.ts`.

use super::SequenceConfig;
use crate::config::within;
use crate::diagrams::Render;
use crate::geom::{Point, Size};
use crate::svg::edges::Marker;
use std::{
    collections::HashSet,
    fmt::{self, Write as _},
};

use indexmap::IndexMap;

use super::{
    actors::ActorKind,
    db::SequenceModel,
    types::{Block, CentralConnection, LineType, Placement, SeqActor, SeqMessage, SeqMessageText},
};
use crate::{Config, Ctx, Error};
use crate::{
    Rendered,
    config::Value,
    js,
    svg::{
        builder::{Esc, EscText, fragment, num},
        edges::ARROW,
        root::svg_document,
        theme::{RADIUS, class_names, fill_style},
    },
    text::{Host, Label, Style},
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

impl LineStyle {
    /// Heads at the receiver.
    fn forward(dotted: bool, head: Head, bottom: bool) -> Self {
        Self {
            dotted,
            head,
            at_start: false,
            at_end: true,
            bottom,
        }
    }

    /// Heads at the sender.
    fn reverse(dotted: bool, head: Head, bottom: bool) -> Self {
        Self {
            dotted,
            head,
            at_start: true,
            at_end: false,
            bottom,
        }
    }

    /// Heads at both ends.
    fn both(dotted: bool, head: Head) -> Self {
        Self {
            dotted,
            head,
            at_start: true,
            at_end: true,
            bottom: false,
        }
    }
}

impl LineType {
    /// How a message of this kind is drawn, `None` when it is not a line.
    fn style(self) -> Option<LineStyle> {
        use LineStyle as L;
        use LineType::*;
        Some(match self {
            Solid => L::forward(false, Head::Arrow, false),
            Dotted => L::forward(true, Head::Arrow, false),
            SolidCross => L::forward(false, Head::Cross, false),
            DottedCross => L::forward(true, Head::Cross, false),
            SolidOpen => L::forward(false, Head::None, false),
            DottedOpen => L::forward(true, Head::None, false),
            SolidPoint => L::forward(false, Head::Open, false),
            DottedPoint => L::forward(true, Head::Open, false),
            BidirectionalSolid => L::both(false, Head::Arrow),
            BidirectionalDotted => L::both(true, Head::Arrow),
            SolidTop => L::forward(false, Head::HalfSolid, false),
            SolidBottom => L::forward(false, Head::HalfSolid, true),
            StickTop => L::forward(false, Head::HalfStick, false),
            StickBottom => L::forward(false, Head::HalfStick, true),
            SolidArrowTopReverse => L::reverse(false, Head::HalfSolid, false),
            SolidArrowBottomReverse => L::reverse(false, Head::HalfSolid, true),
            StickArrowTopReverse => L::reverse(false, Head::HalfStick, false),
            StickArrowBottomReverse => L::reverse(false, Head::HalfStick, true),
            SolidTopDotted => L::forward(true, Head::HalfSolid, false),
            SolidBottomDotted => L::forward(true, Head::HalfSolid, true),
            StickTopDotted => L::forward(true, Head::HalfStick, false),
            StickBottomDotted => L::forward(true, Head::HalfStick, true),
            SolidArrowTopReverseDotted => L::reverse(true, Head::HalfSolid, false),
            SolidArrowBottomReverseDotted => L::reverse(true, Head::HalfSolid, true),
            StickArrowTopReverseDotted => L::reverse(true, Head::HalfStick, false),
            StickArrowBottomReverseDotted => L::reverse(true, Head::HalfStick, true),
            _ => return None,
        })
    }

    /// Whether a message of this kind is drawn as a line between lifelines.
    fn is_line(self) -> bool {
        self.style().is_some()
    }
}

struct Numbering {
    /// The number each message line carries, whether or not it is shown. NaN
    /// for everything else.
    index: Vec<f64>,
    shown: Vec<bool>,
}

impl Numbering {
    /// Counts message lines the way Mermaid's renderer does while it draws.
    fn of(messages: &[SeqMessage], always: bool) -> Self {
        let mut index = vec![f64::NAN; messages.len()];
        let mut shown = vec![false; messages.len()];
        let mut current = 1.0;
        let mut step = 1.0;
        let mut visible = false;
        for (i, m) in messages.iter().enumerate() {
            if m.kind == LineType::Autonumber {
                if let SeqMessageText::Numbering(spec) = &m.message {
                    current = spec.start.filter(|&s| js::truthy(s)).unwrap_or(current);
                    step = spec.step.filter(|&s| js::truthy(s)).unwrap_or(step);
                    visible = spec.visible;
                }
            } else if m.kind.is_line() {
                index[i] = current;
                shown[i] = visible || always;
                current = js::round((current + step) * 100.0) / 100.0;
            }
        }
        Numbering { index, shown }
    }
}

struct Column<'m> {
    id: &'m str,
    actor: &'m SeqActor,
    kind: ActorKind,
    label: Label,
    size: Size,
    x: f64,
    /// The message line that creates the participant.
    create_at: Option<usize>,
    /// The message line that destroys the participant.
    destroy_at: Option<usize>,
    top: f64,
    life_start: f64,
    /// Where the lifeline ends, when the participant is destroyed.
    life_end: Option<f64>,
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
    kind: Block,
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

fn option(value: Option<f64>, fallback: f64) -> f64 {
    within(value, fallback, 0.0, 2000.0)
}

/// A half head whose tip is at `tip`, pointing along `dir`, on the side
/// `up`.
fn half(f: &mut fmt::Formatter<'_>, tip: Point, dir: f64, up: f64, stick: bool) -> fmt::Result {
    let p = crate::PREFIX;
    let Point { x, y } = tip;
    let bx = num(x - dir * ARROW);
    if stick {
        return write!(
            f,
            "<path class=\"{p}-marker\" d=\"M{tip}L{bx},{}\"/>",
            num(y + up * 5.0)
        );
    }
    write!(
        f,
        "<path class=\"{p}-marker\" d=\"M{tip}L{bx},{}V{}Z\" fill=\"var(--_l)\" stroke=\"none\"/>",
        num(y + up * 4.5),
        num(y)
    )
}

impl Head {
    /// Draws the head whose tip is at `tip` and points along `dir`. `up` is
    /// the side a half head sits on.
    fn draw(self, tip: Point, dir: f64, up: f64) -> impl fmt::Display {
        fragment(move |f| {
            let p = crate::PREFIX;
            let Point { x, y } = tip;
            match self {
                Head::Arrow => write!(
                    f,
                    "{}",
                    Marker::Arrow.draw(tip, Point::new(dir, 0.0), "var(--_l)")
                ),
                Head::Cross => write!(
                    f,
                    "{}",
                    Marker::draw_of(
                        Marker::from_name("arrow_cross"),
                        tip,
                        Point::new(dir, 0.0),
                        "var(--_l)"
                    )
                ),
                Head::Open => write!(
                    f,
                    "<path class=\"{p}-marker\" d=\"M{}L{tip}L{}\" stroke-linejoin=\"round\"/>",
                    Point::new(x - dir * ARROW, y - 4.5),
                    Point::new(x - dir * ARROW, y + 4.5)
                ),
                Head::HalfSolid => half(f, tip, dir, up, false),
                Head::HalfStick => half(f, tip, dir, up, true),
                Head::None => Ok(()),
            }
        })
    }

    /// How far a line stops short of the tip.
    fn trim(self) -> f64 {
        match self {
            Head::Arrow => ARROW - 1.0,
            Head::Cross => 4.0,
            _ => 0.0,
        }
    }
}

/// Where a message's text sits along its line.
#[derive(Clone, Copy)]
enum Align {
    Left,
    Centre,
    Right,
}

impl Render for SequenceModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.tighten(|tight| self.draw(&config.sequence, ctx, tight)))
    }
}

/// The drawing's horizontal extent and what it has drawn so far.
struct Out<'m> {
    min_x: f64,
    max_x: f64,
    /// Shaded blocks and activations close from the inside out. They are
    /// written in reverse, so that the inner ones end up on top.
    rects: Vec<String>,
    activations: Vec<Activation<'m>>,
    frames: String,
    edges: String,
    labels: String,
    notes: String,
    actors: String,
    crosses: String,
}

/// An activation bar on participant `id`.
struct Activation<'m> {
    id: &'m str,
    /// The bar's top left.
    at: Point,
    height: f64,
}

impl<'m> Out<'m> {
    fn extend(&mut self, x0: f64, x1: f64) {
        if x0 < self.min_x {
            self.min_x = x0;
        }
        if x1 > self.max_x {
            self.max_x = x1;
        }
    }

    fn end_activation(&mut self, col: &mut Column<'m>, end: f64) -> f64 {
        let start = col.activations.pop().unwrap_or(end);
        let bottom = end.max(start + ACT_MIN);
        let x = col.x - ACT_W / 2.0 + col.activations.len() as f64 * ACT_STEP;
        self.extend(x, x + ACT_W);
        self.activations.push(Activation {
            id: col.id,
            at: Point::new(x, start),
            height: bottom - start,
        });
        bottom
    }

    /// The lifeline of a destroyed participant ends in a cross a little below
    /// the message that ends it.
    fn destroy(&mut self, col: &mut Column<'m>, line_y: f64) -> f64 {
        let p = crate::PREFIX;
        let cx = col.x;
        let cy = line_y + 16.0;
        col.life_end = Some(cy);
        let _ = write!(
            self.crosses,
            "<path class=\"{p}-destroy\" data-id=\"{}\" d=\"M{}L{}M{}L{}\"/>",
            EscText(col.id),
            Point::new(cx - 6.0, cy - 6.0),
            Point::new(cx + 6.0, cy + 6.0),
            Point::new(cx + 6.0, cy - 6.0),
            Point::new(cx - 6.0, cy + 6.0)
        );
        cy + 6.0
    }
}

impl Column<'_> {
    /// The participant drawn with its top at `top`, at the foot of the
    /// diagram when `footer`.
    fn svg(&self, top: f64, footer: bool) -> impl fmt::Display + '_ {
        fragment(move |f| {
            let p = crate::PREFIX;
            write!(
                f,
                "<g class=\"{p}-node {p}-actor {p}-actor-{}",
                self.kind.name()
            )?;
            if footer {
                write!(f, " {p}-actor-footer")?;
            }
            if let Some(Value::String(cls)) = self.actor.properties.get("class") {
                f.write_str(&class_names(cls))?;
            }
            write!(
                f,
                "\" data-id=\"{}\">{}</g>",
                EscText(self.id),
                self.kind
                    .draw(Point::new(self.x, top), self.size, &self.label)
            )
        })
    }

    /// The left edge of the lifeline with `extra` more activations.
    fn left_edge(&self, extra: usize) -> f64 {
        if self.activations.len() + extra > 0 {
            self.x - ACT_W / 2.0
        } else {
            self.x
        }
    }

    /// The right edge of the lifeline with `extra` more activations.
    fn right_edge(&self, extra: usize) -> f64 {
        let depth = self.activations.len() + extra;
        if depth > 0 {
            self.x + ACT_W / 2.0 + (depth - 1) as f64 * ACT_STEP
        } else {
            self.x
        }
    }
}

impl SequenceModel {
    fn draw(&self, conf: &SequenceConfig, ctx: &Ctx<'_, impl Host>, tight: bool) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let tiny = js::round(size * 0.75);
        let measurer = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let mirror = conf.mirror_actors != Some(false);
        let right_angles = conf.right_angles == Some(true);
        let actor_margin =
            option(conf.actor_margin, 40.0).min(if tight { 8.0 } else { f64::INFINITY });
        let min_w = option(conf.width, 64.0).min(if tight { 48.0 } else { f64::INFINITY });
        let min_h = option(conf.height, js::round(size * 1.5) + 20.0);
        let wrap_w = option(conf.width, 160.0).min(if tight {
            32f64.max(
                (ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad)
                    / 1f64.max(self.actors.len() as f64)
                    - actor_margin
                    - 24.0,
            )
        } else {
            f64::INFINITY
        });
        // Mermaid's default of 35 corresponds to Pele's default gap. Other values
        // move it by the difference.
        let msg_gap = 4f64.max(14.0 + option(conf.message_margin, 35.0) - 35.0);
        let note_pad = option(conf.note_margin, 10.0).min(if tight { 6.0 } else { f64::INFINITY });
        let box_pad = option(conf.box_margin, 10.0).min(if tight { 6.0 } else { f64::INFINITY });
        let align = match conf.message_align.as_deref() {
            Some("left") => Align::Left,
            Some("right") => Align::Right,
            _ => Align::Centre,
        };

        let msg_pad = if tight { 6.0 } else { MSG_PAD };
        let note_over = if tight { 8.0 } else { NOTE_OVER };
        let frame_pad = if tight { 12.0 } else { FRAME_PAD };
        let frame_nest = if tight { 6.0 } else { FRAME_NEST };
        let messages = &self.messages;
        let count = messages.len();
        let numbering = Numbering::of(messages, conf.show_sequence_numbers == Some(true));
        let text = |raw: Option<&str>, font_size: f64, wrap: bool, width: f64, base: u8| -> Label {
            let label = Label::layout(
                raw,
                false,
                measurer,
                font_size,
                if tight || wrap { width } else { f64::INFINITY },
                base,
            );
            if tight {
                label.broken(measurer, 120f64.max(width))
            } else {
                label
            }
        };

        let used: Option<HashSet<&str>> =
            (conf.hide_unused_participants == Some(true)).then(|| {
                messages
                    .iter()
                    .flat_map(|m| [m.from.as_deref(), m.to.as_deref()])
                    .flatten()
                    .collect()
            });

        // The first message line at or after each message.
        let mut next_line: Vec<Option<usize>> = vec![None; count + 1];
        for i in (0..count).rev() {
            next_line[i] = if messages[i].kind.is_line() {
                Some(i)
            } else {
                next_line[i + 1]
            };
        }

        let mut cols: Vec<Column> = Vec::new();
        let mut col_of: IndexMap<&str, usize> = IndexMap::new();
        for (id, actor) in &self.actors {
            if used
                .as_ref()
                .is_some_and(|used| !used.contains(id.as_str()))
            {
                continue;
            }
            let kind = ActorKind::from_name(&actor.kind);
            let label = text(Some(&actor.description), size, actor.wrap, wrap_w, 0);
            let create_at = self
                .created_actors
                .get(id)
                .and_then(|&created| next_line[created.min(count)])
                .filter(|&at| messages[at].to.as_deref() == Some(id));
            let destroy_at = self
                .destroyed_actors
                .get(id)
                .and_then(|&destroyed| next_line[destroyed.min(count)])
                .filter(|&at| {
                    let m = &messages[at];
                    m.to.as_deref() == Some(id) || m.from.as_deref() == Some(id)
                });
            col_of.insert(id, cols.len());
            let size = kind.size(&label, Size::new(min_w, min_h), tight.then_some(8.0));
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
                life_end: None,
                activations: Vec::new(),
            });
        }
        let n_cols = cols.len();

        // Widths between lifelines. Every message and note says how far apart the
        // lifelines it touches must be, keyed by the columns at either end.
        let mut needs: IndexMap<(usize, usize), f64> = IndexMap::new();
        let mut need = |lo: Option<usize>, hi: usize, width: f64| {
            let Some(lo) = lo.filter(|_| hi < n_cols) else {
                return;
            };
            if needs.get(&(lo, hi)).copied().unwrap_or(0.0) < width {
                needs.insert((lo, hi), width);
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
            if m.kind == LineType::Note {
                let label = text(Some(message), small, m.wrap, wrap_w, 0);
                let w = label.width + 2.0 * note_pad;
                if m.placement == Some(Placement::LeftOf) {
                    need(a.checked_sub(1), a, w + 2.0 * NOTE_GAP + ACT_W);
                } else if m.placement == Some(Placement::RightOf) {
                    need(Some(a), a + 1, w + 2.0 * NOTE_GAP + ACT_W);
                } else if a == b {
                    need(a.checked_sub(1), a, w / 2.0 + NOTE_GAP);
                    need(Some(a), a + 1, w / 2.0 + NOTE_GAP);
                } else {
                    need(Some(a.min(b)), a.max(b), w - 2.0 * note_over);
                }
                views[i] = Some(View {
                    kind: V::Note,
                    a,
                    b,
                    label,
                    x: 0.0,
                    w,
                });
            } else if m.kind.is_line() {
                let label = text(Some(message), small, m.wrap, wrap_w, 0);
                let badge = badge_width(i);
                if a == b {
                    need(
                        Some(a),
                        a + 1,
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
                    let created = if cols[b].create_at == Some(i) {
                        cols[b].size.w / 2.0
                    } else {
                        0.0
                    };
                    let central = if m
                        .central_connection
                        .is_some_and(|c| c != CentralConnection::Neither)
                    {
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
                        Some(a.min(b)),
                        a.max(b),
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
                let b = &self.boxes[box_];
                runs.push(BoxRun {
                    box_,
                    first: i,
                    last: i,
                    title: text(b.name.as_deref(), small, b.wrap, wrap_w, 0),
                    grow: 0.0,
                });
            }
        }

        let n_gaps = n_cols.saturating_sub(1);
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
            gaps[i] = (left.size.w / 2.0 + right.size.w / 2.0 + actor_margin + boxed)
                .max(needs.get(&(i, i + 1)).copied().unwrap_or(0.0));
        }
        // A message across several lifelines widens each gap it crosses by an
        // equal share of what is missing.
        let mut spans: Vec<(usize, usize, f64)> = Vec::new();
        for (&(lo, hi), &width) in &needs {
            if hi - lo > 1 {
                spans.push((lo, hi, width));
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
            if run.last + 1 < n_cols {
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
            match (cols.first(), cols.last()) {
                (Some(first), Some(last)) => (first.x - frame_pad, last.x + frame_pad),
                _ => (0.0, 120.0),
            }
        };
        for (i, m) in messages.iter().enumerate() {
            let kind = m.kind;
            if let Some(block) = kind.opens() {
                arena.push(Frame {
                    kind: block,
                    x0: f64::INFINITY,
                    x1: f64::NEG_INFINITY,
                    top: 0.0,
                    title: Label::default(),
                    tab_w: 0.0,
                    fill: match (block, m.message.text()) {
                        (Block::Rect, Some(text)) => text.to_string(),
                        _ => String::new(),
                    },
                    dividers: vec![i],
                    divider_y: Vec::new(),
                    divider_labels: Vec::new(),
                });
                frames[i] = Some(arena.len() - 1);
                open.push(arena.len() - 1);
            } else if kind.divides() {
                if let Some(&top) = open.last() {
                    arena[top].dividers.push(i);
                }
            } else if kind.closes() {
                let Some(index) = open.pop() else {
                    continue;
                };
                frames[i] = Some(index);
                let frame = &mut arena[index];
                if frame.x0 > frame.x1 {
                    (frame.x0, frame.x1) = span_all();
                }
                if frame.kind != Block::Rect {
                    frame.tab_w = measurer.width(frame.kind.name(), tiny, Style::BOLD) + 14.0;
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
                    if m.placement == Some(Placement::LeftOf) {
                        view.x = a.x - NOTE_GAP - view.w;
                    } else if m.placement == Some(Placement::RightOf) {
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
                    if b.create_at == Some(i) {
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
            if col.create_at.is_none() {
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
        let backed = |out: &mut String, label: &Label, centre: Point, attrs: &str| {
            if label.width == 0.0 {
                return;
            }
            let Point { x: cx, y: cy } = centre;
            let l = cx - label.width / 2.0;
            if crosses(l, l + label.width) {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\" stroke=\"none\"/>",
                    num(l - 3.0),
                    num(cy - label.height / 2.0 + 2.0),
                    num(label.width + 6.0),
                    num(label.height - 4.0)
                );
            }
            let _ = write!(out, "{}", label.svg(centre, attrs));
        };
        let badge = |out: &mut String, i: usize, centre: Point, w: f64| {
            let Point { x: cx, y: cy } = centre;
            let _ = write!(
                out,
                "<g class=\"{p}-sequence-number\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"var(--_l)\" stroke=\"none\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" fill=\"var(--_bg)\" stroke=\"none\">{}</text></g>",
                num(cx - w / 2.0),
                num(cy - BADGE_H / 2.0),
                num(w),
                js::Number(BADGE_H),
                js::Number(BADGE_H / 2.0),
                num(cx),
                num(cy + tiny * 0.35),
                js::Number(tiny),
                Esc(&js::num_str(numbering.index[i]))
            );
        };
        let frame_label = format!(" class=\"{p}-frame-label\" fill=\"var(--_m)\" stroke=\"none\"");

        let mut y = head_bottom + 6.0;
        let mut stack: Vec<usize> = Vec::new();
        let mut extras = String::new();
        for (i, m) in messages.iter().enumerate() {
            let kind = m.kind;
            let frame = frames[i];

            if kind == LineType::ActiveStart {
                if let Some(col) = m.from.as_deref().and_then(|from| col_of.get(from)) {
                    cols[*col].activations.push(y);
                }
            } else if kind == LineType::ActiveEnd {
                if let Some(&col) = m.from.as_deref().and_then(|from| col_of.get(from))
                    && !cols[col].activations.is_empty()
                {
                    y = y.max(out.end_activation(&mut cols[col], y));
                }
            } else if let Some(index) = frame.filter(|&index| arena[index].dividers[0] == i) {
                stack.push(index);
                let frame = &mut arena[index];
                if frame.kind == Block::Rect {
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
                if frame.kind == Block::Rect {
                    let fill = if frame.fill.is_empty() {
                        String::new()
                    } else {
                        fill_style(&frame.fill)
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
                    let _ = write!(
                        out.frames,
                        "<g class=\"{p}-frame {p}-frame-{}\"><rect x=\"{x}\" y=\"{}\" width=\"{w}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"none\"/>",
                        frame.kind.name(),
                        num(frame.top),
                        num(bottom - frame.top)
                    );
                    for (k, &dy) in frame.divider_y.iter().enumerate() {
                        let label = &frame.divider_labels[k];
                        let _ = write!(
                            out.frames,
                            "<path d=\"M{x},{}H{}\"{DASH}/>",
                            num(dy),
                            num(frame.x1)
                        );
                        backed(
                            &mut out.frames,
                            label,
                            Point::new(
                                frame.x0 + 8.0 + label.width / 2.0,
                                dy + 2.0 + label.height / 2.0,
                            ),
                            &frame_label,
                        );
                    }
                    let _ = write!(
                        out.frames,
                        "<rect x=\"{x}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\"/><text class=\"{p}-frame-kind\" x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\" stroke=\"none\">{}</text>",
                        num(frame.top),
                        num(frame.tab_w),
                        js::Number(TAB_H),
                        num(frame.x0 + frame.tab_w / 2.0),
                        num(frame.top + TAB_H / 2.0 + tiny * 0.35),
                        js::Number(tiny),
                        frame.kind.name()
                    );
                    backed(
                        &mut out.frames,
                        &frame.title,
                        Point::new(
                            frame.x0 + frame.tab_w + 8.0 + frame.title.width / 2.0,
                            frame.top + TAB_H.max(frame.title.height) / 2.0,
                        ),
                        &frame_label,
                    );
                    out.frames.push_str("</g>");
                }
                y = bottom;
            } else if kind.divides() {
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
                if m.placement == Some(Placement::RightOf) {
                    x += a.right_edge(0) - a.x;
                } else if m.placement == Some(Placement::LeftOf) {
                    x += a.left_edge(0) - a.x;
                }
                out.extend(x, x + view.w);
                let _ = write!(
                    out.notes,
                    "<g class=\"{p}-note\" data-id=\"i{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"/>{}</g>",
                    EscText(&m.id),
                    num(x),
                    num(top),
                    num(view.w),
                    num(h),
                    view.label.svg(
                        Point::new(x + view.w / 2.0, top + h / 2.0),
                        crate::class!("label", "stroke=\"none\" fill=\"var(--_fg)\"")
                    )
                );
                y = top + h;
            } else if let Some(view) = &views[i] {
                let st = kind.style().expect("a message line has a style");
                let (va, vb) = (view.a, view.b);
                let label = &view.label;
                let bw = badge_width(i);
                // The activation a `+` starts belongs to this message, so the
                // arrow stops at its edge.
                let mut starting = 0;
                for later in &messages[i + 1..] {
                    match later.kind {
                        LineType::ActiveStart => {
                            if later.from == m.to {
                                starting += 1;
                            }
                        }
                        LineType::ActiveEnd
                        | LineType::CentralConnection
                        | LineType::CentralConnectionReverse => {}
                        _ => break,
                    }
                }
                let up = if st.bottom { 1.0 } else { -1.0 };
                let attrs = if st.dotted { DASH } else { "" };
                let cc = m.central_connection.unwrap_or_default();
                let circle_to = cc.at_to();
                let circle_from = cc.at_from();
                let circle = |out: &mut String, centre: Point| {
                    let _ = write!(
                        out,
                        "<circle class=\"{p}-central\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"var(--_l)\" stroke=\"none\"/>",
                        num(centre.x),
                        num(centre.y),
                        js::Number(CENTRAL_R)
                    );
                };
                let tail_at_end = st.at_start && !st.at_end;
                let head_trim = st.head.trim();
                extras.clear();
                let line_y;

                if view.kind == V::SelfLine {
                    let a = &cols[va];
                    let top = y + msg_gap + 0f64.max((label.height - SELF_H) / 2.0);
                    let bottom = top + SELF_H;
                    let mut x0 = if circle_from {
                        a.x + CENTRAL_R
                    } else {
                        a.right_edge(0)
                    };
                    let mut x1 = if circle_to {
                        a.x + CENTRAL_R
                    } else {
                        a.right_edge(starting)
                    };
                    let reach = x0.max(x1) + SELF_W + bw / 2.0;
                    if circle_from {
                        circle(&mut extras, Point::new(a.x, top));
                    }
                    if circle_to {
                        circle(&mut extras, Point::new(a.x, bottom));
                    }
                    if bw > 0.0 {
                        badge(
                            &mut extras,
                            i,
                            if tail_at_end {
                                Point::new(x1, bottom)
                            } else {
                                Point::new(x0, top)
                            },
                            bw,
                        );
                        if tail_at_end {
                            x1 += bw / 2.0;
                        } else {
                            x0 += bw / 2.0;
                        }
                    }
                    // The loop leaves to the right and comes back heading left,
                    // which turns a half head over.
                    let from = num(x0 + if st.at_start { head_trim } else { 0.0 });
                    let to = num(x1 + if st.at_end { head_trim } else { 0.0 });
                    let _ = write!(
                        out.edges,
                        "<g class=\"{p}-edge {p}-message\" data-id=\"i{}\"><path d=\"",
                        EscText(&m.id)
                    );
                    let _ = if right_angles {
                        write!(
                            out.edges,
                            "M{from},{}H{}V{}H{to}",
                            num(top),
                            num(reach),
                            num(bottom)
                        )
                    } else {
                        write!(
                            out.edges,
                            "M{from},{}H{}A6,6 0 0 1 {}V{}A6,6 0 0 1 {}H{to}",
                            num(top),
                            num(reach - 6.0),
                            Point::new(reach, top + 6.0),
                            num(bottom - 6.0),
                            Point::new(reach - 6.0, bottom)
                        )
                    };
                    let _ = write!(out.edges, "\"{attrs}/>");
                    if st.at_start {
                        let _ =
                            write!(out.edges, "{}", st.head.draw(Point::new(x0, top), -1.0, up));
                    }
                    if st.at_end {
                        let _ = write!(
                            out.edges,
                            "{}",
                            st.head.draw(Point::new(x1, bottom), -1.0, -up)
                        );
                    }
                    let ax = a.x;
                    out.extend(ax, reach + 8.0 + label.width);
                    if label.width > 0.0 {
                        let _ = write!(
                            out.labels,
                            "<g class=\"{p}-edge-label\" data-id=\"i{}\">{}</g>",
                            EscText(&m.id),
                            label.svg(
                                Point::new(reach + 8.0 + label.width / 2.0, (top + bottom) / 2.0),
                                ""
                            )
                        );
                    }
                    line_y = bottom;
                } else {
                    let (ax, bx) = (cols[va].x, cols[vb].x);
                    let dir = if bx > ax { 1.0 } else { -1.0 };
                    let creating = cols[vb].create_at == Some(i);
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
                        cols[va].right_edge(0)
                    } else {
                        cols[va].left_edge(0)
                    };
                    let mut x1 = if dir > 0.0 {
                        cols[vb].left_edge(starting)
                    } else {
                        cols[vb].right_edge(starting)
                    };
                    if creating {
                        let b = &mut cols[vb];
                        b.top = line_y - b.size.h / 2.0;
                        b.life_start = b.top + b.size.h;
                        x1 = b.x
                            - dir
                                * if b.kind.is_figure() {
                                    18.0
                                } else {
                                    b.size.w / 2.0 + 1.0
                                };
                    }
                    if circle_from {
                        circle(&mut extras, Point::new(ax, line_y));
                        x0 = ax + dir * CENTRAL_R;
                    }
                    if circle_to {
                        circle(&mut extras, Point::new(bx, line_y));
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
                            badge(&mut extras, i, Point::new(cx, line_y), bw);
                            x1 = cx - (dir * bw) / 2.0;
                        } else {
                            let cx = if circle_from {
                                x0 + dir * (bw / 2.0 + 2.0)
                            } else {
                                x0
                            };
                            badge(&mut extras, i, Point::new(cx, line_y), bw);
                            x0 = cx + (dir * bw) / 2.0;
                        }
                    }
                    let from = x0 + if st.at_start { dir * head_trim } else { 0.0 };
                    let to = x1 - if st.at_end { dir * head_trim } else { 0.0 };
                    let _ = write!(
                        out.edges,
                        "<g class=\"{p}-edge {p}-message\" data-id=\"i{}\"><path d=\"M{}H{}\"{attrs}/>",
                        EscText(&m.id),
                        Point::new(from, line_y),
                        num(to)
                    );
                    if st.at_start {
                        let _ = write!(
                            out.edges,
                            "{}",
                            st.head.draw(Point::new(x0, line_y), -dir, up * dir)
                        );
                    }
                    if st.at_end {
                        let _ = write!(
                            out.edges,
                            "{}",
                            st.head.draw(Point::new(x1, line_y), dir, up * dir)
                        );
                    }
                    if label.width > 0.0 {
                        let l = x0.min(x1);
                        let r = x0.max(x1);
                        let cx = match align {
                            Align::Left => l + msg_pad + label.width / 2.0,
                            Align::Right => r - msg_pad - label.width / 2.0,
                            Align::Centre => (l + r) / 2.0,
                        };
                        out.extend(cx - label.width / 2.0, cx + label.width / 2.0);
                        let _ = write!(
                            out.labels,
                            "<g class=\"{p}-edge-label\" data-id=\"i{}\">",
                            EscText(&m.id)
                        );
                        backed(
                            &mut out.labels,
                            label,
                            Point::new(cx, line_y - 4.0 - label.height / 2.0),
                            "",
                        );
                        out.labels.push_str("</g>");
                    }
                    if creating {
                        y = line_y + b_size.h / 2.0 - 6.0;
                    }
                }
                let _ = write!(out.edges, "{extras}</g>");
                y = y.max(line_y);
                if cols[va].destroy_at == Some(i) {
                    y = y.max(out.destroy(&mut cols[va], line_y));
                }
                if cols[vb].destroy_at == Some(i) && vb != va {
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
                if col.life_end.is_some() {
                    continue;
                }
                foot_h = foot_h.max(col.size.h);
                let _ = write!(out.actors, "{}", col.svg(y, true));
            }
            bottom = y + foot_h;
        }
        let mut lifelines = String::new();
        for col in &cols {
            let _ = write!(out.actors, "{}", col.svg(col.top, false));
            let end = col.life_end.unwrap_or(y);
            if end > col.life_start {
                let _ = write!(
                    lifelines,
                    "<path class=\"{p}-lifeline\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
                    EscText(col.id),
                    num(col.x),
                    num(col.life_start),
                    num(end)
                );
            }
        }

        let mut boxes_out = String::new();
        for run in &runs {
            let b = &self.boxes[run.box_];
            let x0 = cols[run.first].x - cols[run.first].size.w / 2.0 - box_pad - run.grow;
            let x1 = cols[run.last].x + cols[run.last].size.w / 2.0 + box_pad + run.grow;
            let fill = if b.fill == "transparent" {
                String::new()
            } else {
                fill_style(&b.fill)
            };
            let _ = write!(
                boxes_out,
                "<g class=\"{p}-cluster {p}-box\"{}><rect x=\"{}\" y=\"0\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\" stroke=\"var(--_b)\"{}{fill}/>{}</g>",
                match b.name.as_deref().filter(|name| !name.is_empty()) {
                    Some(name) => format!(" data-id=\"{}\"", EscText(name)),
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
                run.title.svg(
                    Point::new(
                        (x0 + x1) / 2.0,
                        box_pad / 2.0 + 2.0 + run.title.height / 2.0
                    ),
                    crate::class!("cluster-label", "fill=\"var(--_m)\"")
                )
            );
        }
        if !runs.is_empty() {
            bottom += box_pad;
        }

        let mut min_y = 0.0;
        let mut title_out = String::new();
        let title = text(
            self.titles.title.as_deref(),
            size,
            false,
            if tight { ctx.title_room() } else { wrap_w },
            Style::BOLD,
        );
        if out.min_x > out.max_x {
            out.extend(0.0, title.width.max(40.0));
        }
        if title.width > 0.0 {
            let cx = (out.min_x + out.max_x) / 2.0;
            out.extend(cx - title.width / 2.0, cx + title.width / 2.0);
            min_y = -title.height - 12.0;
            let title_attrs = crate::svg::builder::TITLE;
            let _ = write!(
                title_out,
                "{}",
                title.svg(Point::new(cx, min_y + title.height / 2.0), title_attrs)
            );
        }

        let width = (out.max_x - out.min_x + 2.0 * pad).ceil();
        let height = (bottom - min_y + 2.0 * pad).ceil();
        let mut content = format!(
            "<g transform=\"translate({},{})\">{title_out}",
            num(pad - out.min_x),
            num(pad - min_y)
        );
        let small_str = js::Number(small);
        if !boxes_out.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-boxes\" font-size=\"{small_str}\">{boxes_out}</g>"
            );
        }
        if !out.rects.is_empty() {
            let _ = write!(content, "<g class=\"{p}-rects\" fill=\"var(--_a)\">");
            for rect in out.rects.iter().rev() {
                content.push_str(rect);
            }
            content.push_str("</g>");
        }
        if !lifelines.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-lifelines\" fill=\"none\" stroke=\"var(--_b)\">{lifelines}</g>"
            );
        }
        if !out.activations.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-activations\" fill=\"var(--_s)\" stroke=\"var(--_b)\">"
            );
            for bar in out.activations.iter().rev() {
                let _ = write!(
                    content,
                    "<rect class=\"{p}-activation\" data-id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\"/>",
                    EscText(bar.id),
                    num(bar.at.x),
                    num(bar.at.y),
                    js::Number(ACT_W),
                    num(bar.height)
                );
            }
            content.push_str("</g>");
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
            self.titles.accessible(),
            &content,
        );
        Rendered {
            svg,
            width,
            height,
            links: Vec::new(),
        }
    }
}
