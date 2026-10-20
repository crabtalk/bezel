//! Pele's `diagrams/eventmodeling/render.ts`.

use crate::geom::{Point, Size};
use crate::svg::builder::{Esc, EscText, TITLE, fragment};
use crate::svg::edges::Marker;
use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write as _},
};

use super::{
    model::{EmBox, EmRelation, EmSpecification, EventModelingModel},
    parser::EntityType,
};
use crate::{
    Config, Ctx, Error, Rendered,
    diagrams::Render,
    js,
    svg::{
        builder::num,
        root::svg_document,
        theme::{FONT_MONO, RADIUS, series_color},
    },
    text::{Host, Label, Span, Style, decode_entities},
};

const BOX_PAD_X: f64 = 12.0;
const BOX_PAD_Y: f64 = 10.0;
const BOX_MIN_WIDTH: f64 = 96.0;
const BOX_GAP: f64 = 16.0;
const STAGGER: f64 = 80.0;
const LANE_PAD: f64 = 12.0;
const LANE_GAP: f64 = 8.0;
const DATA_COLUMNS: usize = 48;
const NOTE_COLUMNS: usize = 80;
const CARD_GAP: f64 = 8.0;
const RAIL_GAP: f64 = 8.0;
const CARD_MIN: f64 = 120.0;
const ARROW_ROOM: f64 = 20.0;

impl EntityType {
    /// Its series slot. A UI has none and is drawn plain.
    fn series(self) -> Option<i64> {
        match self {
            Self::Command => Some(0),
            Self::Event => Some(1),
            Self::ReadModel => Some(2),
            Self::Processor => Some(6),
            Self::Ui => None,
        }
    }
}

struct Placed {
    name: Label,
    data: Vec<String>,
    at: Point,
    size: Size,
}

impl Placed {
    fn right(&self) -> f64 {
        self.at.x + self.size.w
    }

    fn bottom(&self) -> f64 {
        self.at.y + self.size.h
    }
}

struct Band<'m> {
    lane_label: &'m str,
    label: Label,
    right: f64,
    y: f64,
    h: f64,
}

/// Splits text into lines of at most `columns` characters, keeping its own
/// line breaks.
fn hard_wrap(text: &str, columns: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in decode_entities(text).split('\n') {
        if js::len(line) <= columns {
            out.push(line.to_string());
            continue;
        }
        let units: Vec<u16> = line.encode_utf16().collect();
        let mut i = 0;
        while i < units.len() {
            out.push(String::from_utf16_lossy(
                &units[i..(i + columns).min(units.len())],
            ));
            i += columns;
        }
    }
    out
}

/// A line of text [`lines`] writes.
trait Row {
    fn is_blank(&self) -> bool;
    fn write_esc(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

impl Row for String {
    fn is_blank(&self) -> bool {
        self.is_empty()
    }

    fn write_esc(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Esc(self))
    }
}

impl Row for Vec<Span> {
    fn is_blank(&self) -> bool {
        self.iter().all(|span| span.text.is_empty())
    }

    fn write_esc(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.iter()
            .try_for_each(|span| write!(f, "{}", Esc(&span.text)))
    }
}

/// `rows` as lines starting at `at.x`, the first with its baseline at `at.y`.
fn lines<'a>(rows: &'a [impl Row], at: Point, step: f64, attrs: &'a str) -> impl fmt::Display + 'a {
    let Point { x, y } = at;
    fragment(move |f| {
        if rows.iter().all(Row::is_blank) {
            return Ok(());
        }
        write!(f, "<text{attrs} xml:space=\"preserve\">")?;
        for (i, row) in rows.iter().enumerate() {
            if !row.is_blank() {
                write!(
                    f,
                    "<tspan x=\"{}\" y=\"{}\">",
                    num(x),
                    num(y + i as f64 * step)
                )?;
                row.write_esc(f)?;
                f.write_str("</tspan>")?;
            }
        }
        f.write_str("</text>")
    })
}

impl EmBox {
    /// The class a reset box adds.
    fn reset_class(&self) -> &'static str {
        if self.reset {
            concat!(" ", crate::prefix!(), "-em-reset")
        } else {
            ""
        }
    }
}

/// A box's frame: the attributes its rectangles share.
fn frame(at: Point, size: Size) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"",
            num(at.x),
            num(at.y),
            num(size.w),
            num(size.h)
        )
    })
}

fn widest(measure: &impl Host, rows: &[String], font_size: f64, style: u8) -> f64 {
    let mut width = 0f64;
    for row in rows {
        width = width.max(measure.width(row, font_size, style));
    }
    width
}

impl Render for EventModelingModel {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.turn_to_fit(|down| {
            if down {
                self.draw_cards(ctx)
            } else {
                self.draw_lanes(ctx)
            }
        }))
    }
}

impl EventModelingModel {
    /// Lanes with time running left to right.
    fn draw_lanes(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let name_size = js::round(size * 0.875);
        let small = js::round(size * 0.8125);
        let mono = js::round(size * 0.75);
        let mono_step = js::round(mono * 1.4);
        let small_step = js::round(small * 1.5);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        // Bands by lane key, in the model's lane order.
        let mut bands: Vec<Band> = Vec::new();
        let mut band_of: HashMap<usize, usize> = HashMap::new();
        let mut label_column = 0f64;
        for lane in &self.lanes {
            let label = Label::layout(
                Some(&lane.label),
                false,
                measure,
                small,
                4000.0,
                Style::BOLD,
            );
            label_column = label_column.max(label.width);
            band_of.insert(lane.key, bands.len());
            bands.push(Band {
                lane_label: &lane.label,
                label,
                right: f64::NEG_INFINITY,
                y: 0.0,
                h: 0.0,
            });
        }
        label_column += 2.0 * LANE_PAD;

        // Time runs left to right. A frame in a new lane starts under the end of
        // the one before it, so the arrow between them can drop straight down; a
        // frame in the same lane follows on.
        let mut placed: Vec<Placed> = Vec::new();
        for (index, box_) in self.boxes.iter().enumerate() {
            let band_index = band_of[&box_.lane.key];
            let name = Label::layout(
                Some(&box_.name),
                false,
                measure,
                name_size,
                4000.0,
                Style::BOLD,
            );
            let data = match box_.data.as_deref() {
                Some(text) if !text.is_empty() => hard_wrap(text, DATA_COLUMNS),
                _ => Vec::new(),
            };
            let w = BOX_MIN_WIDTH
                .max(name.width.max(widest(measure, &data, mono, Style::MONO)) + 2.0 * BOX_PAD_X);
            let h = 2.0 * BOX_PAD_Y
                + name.height
                + if data.is_empty() {
                    0.0
                } else {
                    6.0 + data.len() as f64 * mono_step
                };
            let band = &bands[band_index];
            let mut x = 0.0;
            if index > 0 {
                let previous = &placed[index - 1];
                let stagger =
                    previous.at.x + (previous.size.w / 2.0).max(previous.size.w - STAGGER);
                x = if self.boxes[index - 1].lane.key == box_.lane.key {
                    band.right + BOX_GAP
                } else {
                    stagger.max(band.right + BOX_GAP)
                };
            }
            let band = &mut bands[band_index];
            band.right = x + w;
            band.h = band.h.max(h);
            placed.push(Placed {
                name,
                data,
                at: Point::new(x, 0.0),
                size: Size::new(w, h),
            });
        }

        let mut content_right = 0f64;
        for item in &placed {
            content_right = content_right.max(item.at.x + item.size.w);
        }
        let lane_width = label_column + content_right + LANE_PAD;

        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(12.0);
        let left = pad;
        let top = pad + title_height;

        let mut y = top;
        let mut lanes = String::new();
        let lane_attrs = format!(
            " class=\"{p}-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
            num(small)
        );
        for band in &mut bands {
            band.y = y;
            band.h += 2.0 * LANE_PAD;
            let _ = write!(
                lanes,
                "<g class=\"{p}-cluster {p}-em-lane\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\"/>{}</g>",
                EscText(band.lane_label),
                num(left),
                num(y),
                num(lane_width),
                num(band.h),
                band.label.svg(
                    Point::new(left + LANE_PAD + band.label.width / 2.0, y + band.h / 2.0),
                    &lane_attrs
                )
            );
            y += band.h + LANE_GAP;
        }
        let mut bottom = if bands.is_empty() { top } else { y - LANE_GAP };

        let mut nodes = String::new();
        let name_attrs = format!(
            " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
            num(name_size)
        );
        let data_attrs = format!(
            " class=\"{p}-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
            num(mono)
        );
        for (index, item) in placed.iter_mut().enumerate() {
            let box_ = &self.boxes[index];
            let band = &bands[band_of[&box_.lane.key]];
            item.at.x += left + label_column;
            item.at.y = band.y + (band.h - item.size.h) / 2.0;
            let frame = frame(item.at, item.size);
            let _ = write!(
                nodes,
                "<g class=\"{p}-node {p}-em-{}{}\" data-id=\"{}\">",
                box_.kind.name(),
                box_.reset_class(),
                EscText(&box_.id)
            );
            let _ = match box_.kind.series() {
                None => write!(
                    nodes,
                    "<rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>"
                ),
                Some(series) => write!(
                    nodes,
                    "<rect {frame} fill=\"var(--_bg)\"/><rect {frame} fill=\"{0}\" fill-opacity=\"0.22\" stroke=\"{0}\"/>",
                    series_color(series)
                ),
            };
            let name_y = item.at.y + BOX_PAD_Y + item.name.height / 2.0;
            let _ = write!(
                nodes,
                "{}{}</g>",
                item.name.svg(
                    Point::new(item.at.x + item.size.w / 2.0, name_y),
                    &name_attrs
                ),
                lines(
                    &item.data,
                    Point::new(
                        item.at.x + BOX_PAD_X,
                        item.at.y
                            + BOX_PAD_Y
                            + item.name.height
                            + 6.0
                            + mono_step / 2.0
                            + mono * 0.35,
                    ),
                    mono_step,
                    &data_attrs,
                )
            );
        }

        let mut edges = String::new();
        let mut d = String::new();
        let trim = Marker::Arrow.trim();
        for relation in &self.relations {
            let (si, ti) = (relation.source_index, relation.target_index);
            if si == ti {
                continue;
            }
            let (s, t) = (&placed[si], &placed[ti]);
            d.clear();
            let head = if self.boxes[si].lane.key == self.boxes[ti].lane.key {
                // Along the lane, from the side of one box to the side of the
                // other.
                let forward = t.at.x >= s.at.x;
                let start = Point::new(
                    if forward { s.right() } else { s.at.x },
                    s.at.y + s.size.h / 2.0,
                );
                let end = Point::new(
                    if forward { t.at.x } else { t.right() },
                    t.at.y + t.size.h / 2.0,
                );
                let dir = if end.x >= start.x { 1.0 } else { -1.0 };
                let _ = write!(d, "M{start}L{},{}", num(end.x - dir * trim), num(end.y));
                Marker::Arrow.draw(end, Point::new(dir, 0.0), "var(--_l)")
            } else {
                // Between lanes, straight down or up where the boxes overlap,
                // else in an S.
                let down = t.at.y > s.at.y;
                let from = s.at.x.max(t.at.x);
                let to = s.right().min(t.right());
                let inset = |w: f64| 24f64.min(w / 4.0);
                let sx = if to - from >= 16.0 {
                    (from + to) / 2.0
                } else if t.at.x > s.at.x {
                    s.right() - inset(s.size.w)
                } else {
                    s.at.x + inset(s.size.w)
                };
                let tx = if to - from >= 16.0 {
                    sx
                } else if t.at.x > s.at.x {
                    t.at.x + inset(t.size.w)
                } else {
                    t.right() - inset(t.size.w)
                };
                let sy = if down { s.bottom() } else { s.at.y };
                let ty = if down { t.at.y } else { t.bottom() };
                let dir = if down { 1.0 } else { -1.0 };
                let end = ty - dir * trim;
                let mid = (sy + end) / 2.0;
                let _ = if sx == tx {
                    write!(d, "M{},{}V{}", num(sx), num(sy), num(end))
                } else {
                    write!(
                        d,
                        "M{},{}C{},{} {},{} {},{}",
                        num(sx),
                        num(sy),
                        num(sx),
                        num(mid),
                        num(tx),
                        num(mid),
                        num(tx),
                        num(end)
                    )
                };
                Marker::Arrow.draw(Point::new(tx, ty), Point::new(0.0, dir), "var(--_l)")
            };
            let _ = write!(edges, "{}", relation.edge(&d, head));
        }

        // Notes and given/when/then specifications, listed under the lanes with
        // the frame they belong to.
        let names = self.frame_names();
        let mut cards = String::new();
        let mut cards_right = 0f64;
        let head_attrs = format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small));
        let row_attrs = format!(" font-size=\"{}\"", num(small));
        let mut card = |kind: &str, heading: &str, frame: &str, rows: Vec<String>| {
            let head = Label::layout(
                Some(&names.heading(heading, frame)),
                false,
                measure,
                small,
                4000.0,
                Style::BOLD,
            );
            let w = head.width.max(widest(measure, &rows, small, 0)) + 2.0 * BOX_PAD_X;
            let h = 2.0 * BOX_PAD_Y + head.height + rows.len() as f64 * small_step;
            let cy = bottom
                + if cards.is_empty() && bands.is_empty() {
                    0.0
                } else {
                    CARD_GAP
                };
            let _ = write!(
                cards,
                "<g class=\"{p}-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>{}{}</g>",
                EscText(frame),
                num(left),
                num(cy),
                num(w),
                num(h),
                head.svg(
                    Point::new(
                        left + BOX_PAD_X + head.width / 2.0,
                        cy + BOX_PAD_Y + head.height / 2.0
                    ),
                    &head_attrs
                ),
                lines(
                    &rows,
                    Point::new(
                        left + BOX_PAD_X,
                        cy + BOX_PAD_Y + head.height + small_step / 2.0 + small * 0.35,
                    ),
                    small_step,
                    &row_attrs
                )
            );
            bottom = cy + h;
            cards_right = cards_right.max(w);
        };
        for note in &self.notes {
            card(
                "note",
                "Note",
                &note.frame,
                hard_wrap(&note.text, NOTE_COLUMNS),
            );
        }
        for spec in &self.specifications {
            let rows = spec.rows();
            card(
                "specification",
                "Given / When / Then",
                &spec.frame,
                rows.iter()
                    .flat_map(|row| hard_wrap(row, NOTE_COLUMNS))
                    .collect(),
            );
        }

        let width = (if bands.is_empty() { 0.0 } else { lane_width })
            .max(cards_right)
            .max(title.width);
        let total_width = (width + 2.0 * pad).ceil();
        let total_height = (bottom + pad).ceil();
        let mut content =
            String::with_capacity(lanes.len() + edges.len() + nodes.len() + cards.len() + 512);
        let _ = write!(
            content,
            "{}{lanes}{edges}{nodes}{cards}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );

        self.document(ctx, size, total_width, total_height, &content)
    }

    fn frame_names(&self) -> FrameNames<'_> {
        let mut names = HashMap::new();
        for box_ in &self.boxes {
            names.insert(box_.id.as_str(), box_.name.as_str());
        }
        FrameNames(names)
    }

    fn document(
        &self,
        ctx: &Ctx<'_, impl Host>,
        size: f64,
        width: f64,
        height: f64,
        content: &str,
    ) -> Rendered {
        Rendered {
            svg: svg_document(
                "eventmodeling",
                width,
                height,
                size,
                ctx,
                self.titles.accessible(),
                content,
            ),
            width,
            height,
            links: Vec::new(),
        }
    }

    /// One card per box, top to bottom, each under its lane's name.
    fn draw_cards(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let mono = js::round(size * 0.75);
        let pad = ctx.options.padding.unwrap_or(8.0);
        let measure = ctx.host;
        let room = 160f64.max(ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad);
        // A relation that skips a card runs down a rail beside the cards.
        // Relations into one card share a rail; others whose runs overlap take
        // the next rail out, so that no two follow the same line.
        let n = self.boxes.len();
        let mut runs: Vec<Option<(usize, usize)>> = vec![None; n];
        let mut run_order: Vec<usize> = Vec::new();
        // A relation between neighbouring cards passes the lower card's lane
        // heading.
        let mut passed: HashSet<usize> = HashSet::new();
        for relation in &self.relations {
            let (s, t) = (relation.source_index, relation.target_index);
            if s == t {
                continue;
            }
            if s.abs_diff(t) < 2 {
                passed.insert(s.max(t));
                continue;
            }
            match &mut runs[t] {
                Some(run) => {
                    run.0 = run.0.min(s);
                    run.1 = run.1.max(s);
                }
                None => {
                    runs[t] = Some((s.min(t), s.max(t)));
                    run_order.push(t);
                }
            }
        }
        let mut rail_of: HashMap<usize, usize> = HashMap::new();
        let mut rail_ends: Vec<usize> = Vec::new();
        let mut sorted: Vec<(usize, (usize, usize))> = run_order
            .iter()
            .map(|&t| (t, runs[t].expect("a run")))
            .collect();
        sorted.sort_by_key(|&(_, (from, _))| from);
        for (target, (from, to)) in sorted {
            let rail = rail_ends
                .iter()
                .position(|&end| end < from)
                .unwrap_or(rail_ends.len());
            if rail == rail_ends.len() {
                rail_ends.push(to);
            } else {
                rail_ends[rail] = to;
            }
            rail_of.insert(target, rail);
        }
        let gutter = 24.0 + RAIL_GAP * (rail_ends.len().saturating_sub(1)) as f64;
        // Rails are never shared to save room: past what the width allows, the
        // drawing is wider.
        let card_width = CARD_MIN.max(room - gutter);
        let text_room = card_width - 2.0 * BOX_PAD_X;
        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            room,
            Style::BOLD,
        );
        let mut y = pad + title.spaced_height(12.0);
        let mut bottom = pad + title.height;
        let mut width = title.width.max(card_width + gutter);
        let mut nodes = String::new();
        let mut edges = String::new();
        let mut cards = String::new();
        let mut placed: Vec<Card> = Vec::with_capacity(n);
        let lane_attrs = format!(
            " class=\"{p}-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
            num(small)
        );
        let name_attrs = format!(
            " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
            num(small)
        );
        let data_attrs = format!(
            " class=\"{p}-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
            num(mono)
        );
        let text = |raw: &str, font_size: f64, style: u8| -> Label {
            Label::literal(&decode_entities(raw), measure, font_size, text_room, style)
        };
        for (index, box_) in self.boxes.iter().enumerate() {
            let lane_room = if passed.contains(&index) {
                text_room - ARROW_ROOM
            } else {
                text_room
            };
            let lane = Label::layout(
                Some(&box_.lane.label),
                false,
                measure,
                small,
                lane_room,
                Style::BOLD,
            )
            .broken(measure, lane_room);
            let name = Label::layout(
                Some(&box_.name),
                false,
                measure,
                small,
                text_room - 36.0,
                Style::BOLD,
            );
            let data = box_
                .data
                .as_deref()
                .filter(|data| !data.is_empty())
                .map(|data| text(data, mono, Style::MONO));
            let w = card_width
                .max(lane.width + 2.0 * BOX_PAD_X)
                .max(name.width + 2.0 * BOX_PAD_X + 36.0)
                .max(data.as_ref().map_or(0.0, |data| data.width) + 2.0 * BOX_PAD_X);
            let head_h = lane.height + 12.0;
            let h =
                2.0 * BOX_PAD_Y + name.height + data.as_ref().map_or(0.0, |data| data.height + 6.0);
            let cy = y + head_h;
            let frame = frame(Point::new(pad, cy), Size::new(w, h));
            let _ = write!(
                nodes,
                "<g class=\"{p}-cluster {p}-em-lane\" data-id=\"{}\">",
                EscText(&box_.lane.label)
            );
            let _ = write!(
                nodes,
                "{}",
                lane.start_lines(Point::new(pad + BOX_PAD_X, y), &lane_attrs)
            );
            let _ = write!(
                nodes,
                "<g class=\"{p}-node {p}-em-{}{}\" data-id=\"{}\"><rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>",
                box_.kind.name(),
                box_.reset_class(),
                EscText(&box_.id)
            );
            if let Some(series) = box_.kind.series() {
                let _ = write!(
                    nodes,
                    "<rect {frame} fill=\"{0}\" fill-opacity=\"0.22\" stroke=\"{0}\"/>",
                    series_color(series)
                );
            }
            let _ = write!(
                nodes,
                "{}",
                name.svg(
                    Point::new(
                        pad + BOX_PAD_X + name.width / 2.0,
                        cy + BOX_PAD_Y + name.height / 2.0
                    ),
                    &name_attrs
                )
            );
            let _ = write!(
                nodes,
                "<text class=\"{p}-em-frame\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
                num(pad + w - BOX_PAD_X),
                num(cy + BOX_PAD_Y + small),
                num(mono),
                Esc(&box_.id)
            );
            if let Some(data) = &data {
                let _ = write!(
                    nodes,
                    "{}",
                    lines(
                        &data.lines,
                        Point::new(
                            pad + BOX_PAD_X,
                            cy + BOX_PAD_Y
                                + name.height
                                + 6.0
                                + data.line_height / 2.0
                                + mono * 0.35,
                        ),
                        data.line_height,
                        &data_attrs,
                    )
                );
            }
            nodes.push_str("</g></g>");
            placed.push(Card {
                top: cy,
                size: Size::new(w, h),
                index,
                lane: lane.width,
            });
            width = width.max(w + gutter);
            bottom = cy + h;
            y = bottom + 28.0;
        }
        let mut d = String::new();
        for relation in &self.relations {
            let (si, ti) = (relation.source_index, relation.target_index);
            if si == ti {
                continue;
            }
            let (s, t) = (&placed[si], &placed[ti]);
            d.clear();
            let down = t.top > s.top;
            let dir = if down { 1.0 } else { -1.0 };
            let sy = if down { s.top + s.size.h } else { s.top };
            let ty = if down { t.top } else { t.top + t.size.h };
            let head = if s.index.abs_diff(t.index) == 1 {
                // The lower card's lane heading sits in the gap, and wraps to
                // leave the arrow room beside it.
                let cx = pad
                    + (s.size.w.min(t.size.w) / 2.0)
                        .max(BOX_PAD_X + (if down { t } else { s }).lane + 16.0);
                let _ = write!(
                    d,
                    "M{},{}V{}",
                    num(cx),
                    num(sy),
                    num(ty - dir * Marker::Arrow.trim())
                );
                Marker::Arrow.draw(Point::new(cx, ty), Point::new(0.0, dir), "var(--_l)")
            } else {
                let rail = pad + width - gutter
                    + 12.0
                    + RAIL_GAP * rail_of.get(&ti).copied().unwrap_or(0) as f64;
                // A card can send one relation and receive another: they leave
                // above and arrive below its middle.
                let from = s.top + s.size.h / 2.0 - 5.0;
                let to = t.top + t.size.h / 2.0 + 5.0;
                let _ = write!(
                    d,
                    "M{},{}H{}V{}H{}",
                    num(pad + s.size.w),
                    num(from),
                    num(rail),
                    num(to),
                    num(pad + t.size.w + Marker::Arrow.trim())
                );
                Marker::Arrow.draw(
                    Point::new(pad + t.size.w, to),
                    Point::new(-1.0, 0.0),
                    "var(--_l)",
                )
            };
            let _ = write!(edges, "{}", relation.edge(&d, head));
        }
        let names = self.frame_names();
        let head_attrs = format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small));
        let row_attrs = format!(" font-size=\"{}\"", num(small));
        let mut card = |kind: &str, heading: &str, frame: &str, raw: &str| {
            let head = Label::layout(
                Some(&names.heading(heading, frame)),
                false,
                measure,
                small,
                text_room,
                Style::BOLD,
            );
            let body = text(raw, small, 0);
            let w = card_width
                .max(head.width + 2.0 * BOX_PAD_X)
                .max(body.width + 2.0 * BOX_PAD_X);
            let h = 2.0 * BOX_PAD_Y + head.height + body.height;
            let _ = write!(
                cards,
                "<g class=\"{p}-em-{kind}\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>{}{}</g>",
                EscText(frame),
                num(pad),
                num(y),
                num(w),
                num(h),
                head.svg(
                    Point::new(
                        pad + BOX_PAD_X + head.width / 2.0,
                        y + BOX_PAD_Y + head.height / 2.0
                    ),
                    &head_attrs
                ),
                lines(
                    &body.lines,
                    Point::new(
                        pad + BOX_PAD_X,
                        y + BOX_PAD_Y + head.height + body.line_height / 2.0 + small * 0.35,
                    ),
                    body.line_height,
                    &row_attrs
                )
            );
            bottom = y + h;
            y = bottom + CARD_GAP;
            width = width.max(w);
        };
        for note in &self.notes {
            card("note", "Note", &note.frame, &note.text);
        }
        for spec in &self.specifications {
            card(
                "specification",
                "Given / When / Then",
                &spec.frame,
                &spec.rows().join("\n"),
            );
        }
        let total_width = (width + 2.0 * pad).ceil();
        let height = (bottom + pad).ceil();
        let mut content = String::with_capacity(edges.len() + nodes.len() + cards.len() + 512);
        let _ = write!(
            content,
            "{}{edges}{nodes}{cards}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        self.document(ctx, size, total_width, height, &content)
    }
}

/// Each frame id with the name of its box, the last box of an id winning.
struct FrameNames<'m>(HashMap<&'m str, &'m str>);

impl FrameNames<'_> {
    /// A card's heading, naming the frame it belongs to.
    fn heading(&self, heading: &str, frame: &str) -> String {
        match self.0.get(frame) {
            Some(name) => format!("{heading} · {frame} {name}"),
            None => format!("{heading} · {frame}"),
        }
    }
}

struct Card {
    top: f64,
    size: Size,
    index: usize,
    /// The width of its lane's name.
    lane: f64,
}

impl EmRelation {
    fn edge<'a>(&'a self, d: &'a str, head: impl fmt::Display + 'a) -> impl fmt::Display + 'a {
        fragment(move |f| {
            write!(
                f,
                "<g class=\"{p}-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}</g>",
                EscText(&self.source.id),
                EscText(&self.target.id),
                p = crate::PREFIX,
            )
        })
    }
}

impl EmSpecification {
    fn rows(&self) -> Vec<String> {
        let mut rows = vec![format!("Given  {}", self.given.join(", "))];
        if !self.when.is_empty() {
            rows.push(format!("When  {}", self.when.join(", ")));
        }
        rows.push(format!("Then  {}", self.then.join(", ")));
        rows
    }
}
