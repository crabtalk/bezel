//! Pele's `diagrams/eventmodeling/render.ts`.

use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
};

use super::model::{EmBox, EventModelingModel};
use crate::{
    Config, Options, Rendered,
    diagrams::common::{
        fit_width::turn_to_fit,
        literal_label::{break_label, left_label_svg, literal_label},
    },
    js,
    svg::{
        builder::{esc, esc_text, label_svg, num},
        edges::{marker, marker_trim},
        root::{Accessible, svg_document},
        theme::{FONT_MONO, RADIUS, series_color},
    },
    text::{Label, Measure, Style, decode_entities, layout_label},
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

/// Series slot for each kind of entity. A UI has none and is drawn plain.
fn series(kind: &str) -> Option<i64> {
    match kind {
        "command" => Some(0),
        "event" => Some(1),
        "readmodel" => Some(2),
        "processor" => Some(6),
        _ => None,
    }
}

struct Placed {
    name: Label,
    data: Vec<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
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

fn lines(rows: &[String], x: f64, y: f64, step: f64, attrs: &str) -> String {
    let mut body = String::new();
    for (i, row) in rows.iter().enumerate() {
        if !row.is_empty() {
            let _ = write!(
                body,
                "<tspan x=\"{}\" y=\"{}\">{}</tspan>",
                num(x),
                num(y + i as f64 * step),
                esc(row)
            );
        }
    }
    if body.is_empty() {
        String::new()
    } else {
        format!("<text{attrs} xml:space=\"preserve\">{body}</text>")
    }
}

fn widest(measure: &dyn Measure, rows: &[String], font_size: f64, style: u8) -> f64 {
    let mut width = 0f64;
    for row in rows {
        width = width.max(measure.width(row, font_size, style));
    }
    width
}

pub fn render_event_model(
    model: &EventModelingModel,
    config: &Config,
    options: &Options,
) -> Rendered {
    turn_to_fit(options, |down| draw(model, config, options, down))
}

fn draw(model: &EventModelingModel, _config: &Config, options: &Options, down: bool) -> Rendered {
    if down {
        return vertical_event_model(model, options);
    }
    let p = crate::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let name_size = js::round(size * 0.875);
    let small = js::round(size * 0.8125);
    let mono = js::round(size * 0.75);
    let mono_step = js::round(mono * 1.4);
    let small_step = js::round(small * 1.5);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);

    // Bands by lane key, in the model's lane order.
    let mut bands: Vec<Band> = Vec::new();
    let mut band_of: HashMap<usize, usize> = HashMap::new();
    let mut label_column = 0f64;
    for lane in &model.lanes {
        let label = layout_label(
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
    for (index, box_) in model.boxes.iter().enumerate() {
        let band_index = band_of[&box_.lane.key];
        let name = layout_label(
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
            let stagger = previous.x + (previous.w / 2.0).max(previous.w - STAGGER);
            x = if model.boxes[index - 1].lane.key == box_.lane.key {
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
            x,
            y: 0.0,
            w,
            h,
        });
    }

    let mut content_right = 0f64;
    for item in &placed {
        content_right = content_right.max(item.x + item.w);
    }
    let lane_width = label_column + content_right + LANE_PAD;

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };
    let left = pad;
    let top = pad + title_height;

    let mut y = top;
    let mut lanes = String::new();
    for band in &mut bands {
        band.y = y;
        band.h += 2.0 * LANE_PAD;
        let _ = write!(
            lanes,
            "<g class=\"{p}-cluster {p}-em-lane\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\"/>{}</g>",
            esc_text(band.lane_label),
            num(left),
            num(y),
            num(lane_width),
            num(band.h),
            label_svg(
                &band.label,
                left + LANE_PAD + band.label.width / 2.0,
                y + band.h / 2.0,
                &format!(
                    " class=\"{p}-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
                    num(small)
                )
            )
        );
        y += band.h + LANE_GAP;
    }
    let mut bottom = if bands.is_empty() { top } else { y - LANE_GAP };

    let mut nodes = String::new();
    for (index, item) in placed.iter_mut().enumerate() {
        let box_ = &model.boxes[index];
        let band = &bands[band_of[&box_.lane.key]];
        item.x += left + label_column;
        item.y = band.y + (band.h - item.h) / 2.0;
        let frame = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"",
            num(item.x),
            num(item.y),
            num(item.w),
            num(item.h)
        );
        let shape = match series(box_.kind) {
            None => format!("<rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>"),
            Some(series) => format!(
                "<rect {frame} fill=\"var(--_bg)\"/><rect {frame} fill=\"{0}\" fill-opacity=\"0.22\" stroke=\"{0}\"/>",
                series_color(series)
            ),
        };
        let name_y = item.y + BOX_PAD_Y + item.name.height / 2.0;
        let text = label_svg(
            &item.name,
            item.x + item.w / 2.0,
            name_y,
            &format!(
                " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                num(name_size)
            ),
        ) + &lines(
            &item.data,
            item.x + BOX_PAD_X,
            item.y + BOX_PAD_Y + item.name.height + 6.0 + mono_step / 2.0 + mono * 0.35,
            mono_step,
            &format!(
                " class=\"{p}-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
                num(mono)
            ),
        );
        let _ = write!(
            nodes,
            "<g class=\"{p}-node {p}-em-{}{}\" data-id=\"{}\">{shape}{text}</g>",
            box_.kind,
            if box_.reset {
                format!(" {p}-em-reset")
            } else {
                String::new()
            },
            esc_text(&box_.id)
        );
    }

    let mut edges = String::new();
    let trim = marker_trim("arrow_point");
    for relation in &model.relations {
        let (si, ti) = (relation.source_index, relation.target_index);
        if si == ti {
            continue;
        }
        let (s, t) = (&placed[si], &placed[ti]);
        let (d, head) = if model.boxes[si].lane.key == model.boxes[ti].lane.key {
            // Along the lane, from the side of one box to the side of the
            // other.
            let forward = t.x >= s.x;
            let sx = if forward { s.x + s.w } else { s.x };
            let tx = if forward { t.x } else { t.x + t.w };
            let sy = s.y + s.h / 2.0;
            let ty = t.y + t.h / 2.0;
            let dir = if tx >= sx { 1.0 } else { -1.0 };
            (
                format!(
                    "M{},{}L{},{}",
                    num(sx),
                    num(sy),
                    num(tx - dir * trim),
                    num(ty)
                ),
                marker("arrow_point", tx, ty, dir, 0.0, "var(--_l)"),
            )
        } else {
            // Between lanes, straight down or up where the boxes overlap,
            // else in an S.
            let down = t.y > s.y;
            let from = s.x.max(t.x);
            let to = (s.x + s.w).min(t.x + t.w);
            let inset = |w: f64| 24f64.min(w / 4.0);
            let sx = if to - from >= 16.0 {
                (from + to) / 2.0
            } else if t.x > s.x {
                s.x + s.w - inset(s.w)
            } else {
                s.x + inset(s.w)
            };
            let tx = if to - from >= 16.0 {
                sx
            } else if t.x > s.x {
                t.x + inset(t.w)
            } else {
                t.x + t.w - inset(t.w)
            };
            let sy = if down { s.y + s.h } else { s.y };
            let ty = if down { t.y } else { t.y + t.h };
            let dir = if down { 1.0 } else { -1.0 };
            let end = ty - dir * trim;
            let mid = (sy + end) / 2.0;
            (
                if sx == tx {
                    format!("M{},{}V{}", num(sx), num(sy), num(end))
                } else {
                    format!(
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
                },
                marker("arrow_point", tx, ty, 0.0, dir, "var(--_l)"),
            )
        };
        edges.push_str(&edge(&relation.source, &relation.target, &d, &head));
    }

    // Notes and given/when/then specifications, listed under the lanes with
    // the frame they belong to.
    let names = frame_names(&model.boxes);
    let mut cards = String::new();
    let mut cards_right = 0f64;
    let mut card = |kind: &str, heading: &str, frame: &str, rows: Vec<String>| {
        let head = layout_label(
            Some(&card_heading(&names, heading, frame)),
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
            esc_text(frame),
            num(left),
            num(cy),
            num(w),
            num(h),
            label_svg(
                &head,
                left + BOX_PAD_X + head.width / 2.0,
                cy + BOX_PAD_Y + head.height / 2.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small))
            ),
            lines(
                &rows,
                left + BOX_PAD_X,
                cy + BOX_PAD_Y + head.height + small_step / 2.0 + small * 0.35,
                small_step,
                &format!(" font-size=\"{}\"", num(small))
            )
        );
        bottom = cy + h;
        cards_right = cards_right.max(w);
    };
    for note in &model.notes {
        card(
            "note",
            "Note",
            &note.frame,
            hard_wrap(&note.text, NOTE_COLUMNS),
        );
    }
    for spec in &model.specifications {
        let rows = specification_rows(spec);
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
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &lanes
        + &edges
        + &nodes
        + &cards;

    document(model, options, size, total_width, total_height, &content)
}

fn edge(source: &EmBox, target: &EmBox, d: &str, head: &str) -> String {
    let p = crate::PREFIX;
    format!(
        "<g class=\"{p}-edge\" data-id=\"{}-{}\"><path d=\"{d}\" fill=\"none\" stroke=\"var(--_l)\"/>{head}</g>",
        esc_text(&source.id),
        esc_text(&target.id)
    )
}

/// Each frame id with the name of its box, the last box of an id winning.
fn frame_names(boxes: &[EmBox]) -> HashMap<&str, &str> {
    let mut names = HashMap::new();
    for box_ in boxes {
        names.insert(box_.id.as_str(), box_.name.as_str());
    }
    names
}

fn card_heading(names: &HashMap<&str, &str>, heading: &str, frame: &str) -> String {
    match names.get(frame) {
        Some(name) => format!("{heading} · {frame} {name}"),
        None => format!("{heading} · {frame}"),
    }
}

fn specification_rows(spec: &super::model::EmSpecification) -> Vec<String> {
    let mut rows = vec![format!("Given  {}", spec.given.join(", "))];
    if !spec.when.is_empty() {
        rows.push(format!("When  {}", spec.when.join(", ")));
    }
    rows.push(format!("Then  {}", spec.then.join(", ")));
    rows
}

fn document(
    model: &EventModelingModel,
    options: &Options,
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
            options,
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            content,
        ),
        width,
        height,
        links: Vec::new(),
    }
}

struct Card {
    y: f64,
    h: f64,
    w: f64,
    index: usize,
    lane: f64,
}

fn vertical_event_model(model: &EventModelingModel, options: &Options) -> Rendered {
    let p = crate::PREFIX;
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let mono = js::round(size * 0.75);
    let pad = options.padding.unwrap_or(8.0);
    let measure = options.measurer();
    let measure = &*measure;
    let room = 160f64.max(options.max_width.unwrap_or(320.0) - 2.0 * pad);
    // A relation that skips a card runs down a rail beside the cards.
    // Relations into one card share a rail; others whose runs overlap take
    // the next rail out, so that no two follow the same line.
    let n = model.boxes.len();
    let mut runs: Vec<Option<(usize, usize)>> = vec![None; n];
    let mut run_order: Vec<usize> = Vec::new();
    // A relation between neighbouring cards passes the lower card's lane
    // heading.
    let mut passed: HashSet<usize> = HashSet::new();
    for relation in &model.relations {
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
    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        room,
        Style::BOLD,
    );
    let mut y = pad
        + if js::truthy(title.height) {
            title.height + 12.0
        } else {
            0.0
        };
    let mut bottom = pad + title.height;
    let mut width = title.width.max(card_width + gutter);
    let mut nodes = String::new();
    let mut edges = String::new();
    let mut cards = String::new();
    let mut placed: Vec<Card> = Vec::with_capacity(n);
    let text = |raw: &str, font_size: f64, style: u8| -> Label {
        literal_label(&decode_entities(raw), measure, font_size, text_room, style)
    };
    let joined = |label: &Label| -> Vec<String> {
        label
            .lines
            .iter()
            .map(|line| line.iter().map(|span| span.text.as_str()).collect())
            .collect()
    };
    for (index, box_) in model.boxes.iter().enumerate() {
        let lane_room = if passed.contains(&index) {
            text_room - ARROW_ROOM
        } else {
            text_room
        };
        let lane = break_label(
            &layout_label(
                Some(&box_.lane.label),
                false,
                measure,
                small,
                lane_room,
                Style::BOLD,
            ),
            measure,
            lane_room,
        );
        let name = layout_label(
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
        let h = 2.0 * BOX_PAD_Y + name.height + data.as_ref().map_or(0.0, |data| data.height + 6.0);
        let cy = y + head_h;
        let frame = format!(
            "x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"",
            num(pad),
            num(cy),
            num(w),
            num(h)
        );
        let _ = write!(
            nodes,
            "<g class=\"{p}-cluster {p}-em-lane\" data-id=\"{}\">",
            esc_text(&box_.lane.label)
        );
        nodes.push_str(&left_label_svg(
            &lane,
            pad + BOX_PAD_X,
            y,
            &format!(
                " class=\"{p}-cluster-label\" font-size=\"{}\" font-weight=\"var(--_hw)\" fill=\"var(--_m)\"",
                num(small)
            ),
        ));
        let _ = write!(
            nodes,
            "<g class=\"{p}-node {p}-em-{}{}\" data-id=\"{}\"><rect {frame} fill=\"var(--_bg)\" stroke=\"var(--_b)\"/>",
            box_.kind,
            if box_.reset {
                format!(" {p}-em-reset")
            } else {
                String::new()
            },
            esc_text(&box_.id)
        );
        if let Some(series) = series(box_.kind) {
            let _ = write!(
                nodes,
                "<rect {frame} fill=\"{0}\" fill-opacity=\"0.22\" stroke=\"{0}\"/>",
                series_color(series)
            );
        }
        nodes.push_str(&label_svg(
            &name,
            pad + BOX_PAD_X + name.width / 2.0,
            cy + BOX_PAD_Y + name.height / 2.0,
            &format!(
                " class=\"{p}-label\" font-size=\"{}\" font-weight=\"var(--_hw)\"",
                num(small)
            ),
        ));
        let _ = write!(
            nodes,
            "<text class=\"{p}-em-frame\" x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"{}\" fill=\"var(--_m)\">{}</text>",
            num(pad + w - BOX_PAD_X),
            num(cy + BOX_PAD_Y + small),
            num(mono),
            esc(&box_.id)
        );
        if let Some(data) = &data {
            nodes.push_str(&lines(
                &joined(data),
                pad + BOX_PAD_X,
                cy + BOX_PAD_Y + name.height + 6.0 + data.line_height / 2.0 + mono * 0.35,
                data.line_height,
                &format!(
                    " class=\"{p}-em-data\" font-family=\"{FONT_MONO}\" font-size=\"{}\"",
                    num(mono)
                ),
            ));
        }
        nodes.push_str("</g></g>");
        placed.push(Card {
            y: cy,
            h,
            w,
            index,
            lane: lane.width,
        });
        width = width.max(w + gutter);
        bottom = cy + h;
        y = bottom + 28.0;
    }
    for relation in &model.relations {
        let (si, ti) = (relation.source_index, relation.target_index);
        if si == ti {
            continue;
        }
        let (s, t) = (&placed[si], &placed[ti]);
        let down = t.y > s.y;
        let dir = if down { 1.0 } else { -1.0 };
        let sy = if down { s.y + s.h } else { s.y };
        let ty = if down { t.y } else { t.y + t.h };
        let (d, head) = if s.index.abs_diff(t.index) == 1 {
            // The lower card's lane heading sits in the gap, and wraps to
            // leave the arrow room beside it.
            let cx =
                pad + (s.w.min(t.w) / 2.0).max(BOX_PAD_X + (if down { t } else { s }).lane + 16.0);
            (
                format!(
                    "M{},{}V{}",
                    num(cx),
                    num(sy),
                    num(ty - dir * marker_trim("arrow_point"))
                ),
                marker("arrow_point", cx, ty, 0.0, dir, "var(--_l)"),
            )
        } else {
            let rail = pad + width - gutter
                + 12.0
                + RAIL_GAP * rail_of.get(&ti).copied().unwrap_or(0) as f64;
            // A card can send one relation and receive another: they leave
            // above and arrive below its middle.
            let from = s.y + s.h / 2.0 - 5.0;
            let to = t.y + t.h / 2.0 + 5.0;
            (
                format!(
                    "M{},{}H{}V{}H{}",
                    num(pad + s.w),
                    num(from),
                    num(rail),
                    num(to),
                    num(pad + t.w + marker_trim("arrow_point"))
                ),
                marker("arrow_point", pad + t.w, to, -1.0, 0.0, "var(--_l)"),
            )
        };
        edges.push_str(&edge(&relation.source, &relation.target, &d, &head));
    }
    let names = frame_names(&model.boxes);
    let mut card = |kind: &str, heading: &str, frame: &str, raw: &str| {
        let head = layout_label(
            Some(&card_heading(&names, heading, frame)),
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
            esc_text(frame),
            num(pad),
            num(y),
            num(w),
            num(h),
            label_svg(
                &head,
                pad + BOX_PAD_X + head.width / 2.0,
                y + BOX_PAD_Y + head.height / 2.0,
                &format!(" font-size=\"{}\" font-weight=\"var(--_hw)\"", num(small))
            ),
            lines(
                &joined(&body),
                pad + BOX_PAD_X,
                y + BOX_PAD_Y + head.height + body.line_height / 2.0 + small * 0.35,
                body.line_height,
                &format!(" font-size=\"{}\"", num(small))
            )
        );
        bottom = y + h;
        y = bottom + CARD_GAP;
        width = width.max(w);
    };
    for note in &model.notes {
        card("note", "Note", &note.frame, &note.text);
    }
    for spec in &model.specifications {
        card(
            "specification",
            "Given / When / Then",
            &spec.frame,
            &specification_rows(spec).join("\n"),
        );
    }
    let total_width = (width + 2.0 * pad).ceil();
    let height = (bottom + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &edges
        + &nodes
        + &cards;
    document(model, options, size, total_width, height, &content)
}
