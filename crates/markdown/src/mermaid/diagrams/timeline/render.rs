//! Pele's `diagrams/timeline/render.ts`.

use super::db::TimelineModel;
use crate::mermaid::{
    Config, Options, Rendered,
    config::Value,
    diagrams::common::{
        fit::{fit_label, plain_label},
        fit_width::turn_to_fit,
    },
    js,
    svg::{
        builder::{esc_text, label_svg, num},
        edges::{ARROW, marker},
        root::{Accessible, svg_document},
        theme::{RADIUS, series_color},
    },
    text::{Label, Style},
};
use std::fmt::Write as _;

const PAD_X: f64 = 12.0;
const PAD_Y: f64 = 8.0;
const MIN_TEXT: f64 = 152.0;
const MAX_TEXT: f64 = 216.0;
const EVENT_TEXT_TD: f64 = 264.0;
const GAP: f64 = 20.0;
const AXIS: f64 = 20.0;
const SPINE: f64 = 14.0;
const EVENT_GAP: f64 = 8.0;
const SECTION_GAP: f64 = 8.0;
const TAIL: f64 = 12.0;
const ON_COLOR: &str = " fill=\"var(--_bg)\"";
const LINE: &str = " fill=\"none\" stroke=\"var(--_b)\"";

struct Group {
    /// Index into the model's sections, -1 for the periods before the first.
    section: i64,
    periods: Vec<usize>,
}

fn rect(x: f64, y: f64, w: f64, h: f64, attrs: &str) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\"{attrs}/>",
        num(x),
        num(y),
        num(w),
        num(h)
    )
}

pub fn render_timeline(model: &TimelineModel, config: &Config, options: &Options) -> Rendered {
    if model.direction == "TD" {
        return draw(model, config, options, true);
    }
    turn_to_fit(options, |down| draw(model, config, options, down))
}

fn draw(model: &TimelineModel, config: &Config, options: &Options, vertical: bool) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);
    let p = crate::mermaid::PREFIX;
    let multicolor = !matches!(
        config
            .section("timeline")
            .and_then(|timeline| timeline.get("disableMulticolor")),
        Some(Value::Bool(true))
    );
    let periods = &model.periods;
    let has_sections = !model.sections.is_empty();

    // Mermaid draws nothing for periods that come before the first section;
    // here they lead, in a neutral color.
    let mut groups: Vec<Group> = (0..model.sections.len())
        .map(|section| Group {
            section: section as i64,
            periods: Vec::new(),
        })
        .collect();
    let mut leading = Group {
        section: -1,
        periods: Vec::new(),
    };
    for (i, period) in periods.iter().enumerate() {
        match usize::try_from(period.section_index)
            .ok()
            .and_then(|index| groups.get_mut(index))
        {
            Some(group) => group.periods.push(i),
            None => leading.periods.push(i),
        }
    }
    if !leading.periods.is_empty() {
        groups.insert(0, leading);
    }

    // Columns widen, up to a point, for a word that would not fit; past that
    // the word is cut. Running down, the two columns share the width the
    // host has.
    let room = match options.max_width {
        Some(max) if vertical && max > 0.0 => max - 2.0 * pad,
        _ => f64::INFINITY,
    };
    let taken = AXIS + SPINE * 2.0 + 4.0 * PAD_X;
    let period_text = MIN_TEXT.min(80f64.max(js::round((room - taken) * 0.38)));
    let event_text = EVENT_TEXT_TD.min(110f64.max(room - taken - period_text));
    let mut text_w = period_text;
    let event_text_w = |text_w: f64| if vertical { event_text } else { text_w };
    let mut period_labels: Vec<Label> = periods
        .iter()
        .map(|period| plain_label(Some(&period.text), measure, size, text_w, 0))
        .collect();
    let mut event_labels: Vec<Vec<Label>> = periods
        .iter()
        .map(|period| {
            period
                .events
                .iter()
                .map(|event| plain_label(Some(event), measure, small, event_text_w(text_w), 0))
                .collect()
        })
        .collect();
    let mut widest: f64 = 0.0;
    for label in &period_labels {
        widest = widest.max(label.width);
    }
    if !vertical {
        for labels in &event_labels {
            for label in labels {
                widest = widest.max(label.width);
            }
        }
    }
    let mut overflow = widest > text_w;
    if vertical {
        for labels in &event_labels {
            for label in labels {
                overflow = overflow || label.width > event_text;
            }
        }
    }
    if overflow {
        text_w = widest
            .ceil()
            .max(period_text)
            .min(if room == f64::INFINITY {
                MAX_TEXT
            } else {
                period_text
            });
        period_labels = periods
            .iter()
            .map(|period| fit_label(Some(&period.text), measure, size, text_w, 0))
            .collect();
        event_labels = periods
            .iter()
            .map(|period| {
                period
                    .events
                    .iter()
                    .map(|event| fit_label(Some(event), measure, small, event_text_w(text_w), 0))
                    .collect()
            })
            .collect();
    }

    let line_height = js::round(size * 1.5);
    let col_w = text_w + 2.0 * PAD_X;
    let event_w = event_text_w(text_w) + 2.0 * PAD_X;
    let chip_height = |label: &Label| label.height.max(line_height) + 2.0 * PAD_Y;
    let event_height = |label: &Label| label.height.max(js::round(small * 1.5)) + 2.0 * PAD_Y;
    let color_of = |group: &Group, period: usize| -> String {
        if group.section >= 0 {
            series_color(group.section)
        } else if has_sections {
            "var(--_m)".into()
        } else {
            series_color(if multicolor { period as i64 } else { 0 })
        }
    };

    let has_events = event_labels.iter().any(|labels| !labels.is_empty());
    let slots: usize = groups.iter().map(|group| group.periods.len().max(1)).sum();

    let pitch = col_w + GAP;
    let drawn = !periods.is_empty();
    let body_w = if vertical {
        col_w
            + if has_events {
                AXIS + SPINE * 2.0 + event_w
            } else if drawn {
                AXIS + TAIL
            } else {
                0.0
            }
    } else if slots > 0 {
        slots as f64 * pitch - GAP + if drawn { TAIL + ARROW } else { 0.0 }
    } else {
        0.0
    };
    let title = fit_label(
        model.title.as_deref(),
        measure,
        size,
        body_w.max(320.0),
        Style::BOLD,
    );
    let title_h = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let width = body_w.max(title.width);
    let left = pad + (width - body_w) / 2.0;
    let top = pad + title_h;

    let section_label = |group: &Group, w: f64| -> Label {
        fit_label(
            Some(&model.sections[group.section as usize]),
            measure,
            size,
            w - 2.0 * PAD_X,
            Style::BOLD,
        )
    };
    let header = |group: &Group, label: &Label, x: f64, y: f64, w: f64, h: f64| -> String {
        rect(
            x,
            y,
            w,
            h,
            &format!(" fill=\"{}\"", series_color(group.section)),
        ) + &label_svg(
            label,
            x + w / 2.0,
            y + h / 2.0,
            &format!(" font-weight=\"var(--_hw)\"{ON_COLOR}"),
        )
    };
    let period = |i: usize, color: &str, x: f64, y: f64, h: f64| -> String {
        rect(x, y, col_w, h, &format!(" fill=\"{color}\""))
            + &label_svg(&period_labels[i], x + col_w / 2.0, y + h / 2.0, ON_COLOR)
    };
    let event = |label: &Label, color: &str, x: f64, y: f64, h: f64| -> String {
        format!(
            "<g class=\"{p}-node {p}-event\">{}{}</g>",
            rect(
                x,
                y,
                event_w,
                h,
                &format!(" fill=\"{color}\" fill-opacity=\"0.16\"")
            ),
            label_svg(
                label,
                x + event_w / 2.0,
                y + h / 2.0,
                &format!(" font-size=\"{}\"", js::num_str(small))
            )
        )
    };
    let dot = |x: f64, y: f64, color: &str| -> String {
        format!(
            "<circle class=\"{p}-dot\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\" stroke=\"var(--_bg)\" stroke-width=\"2\"/>",
            num(x),
            num(y)
        )
    };
    let open = |group: &Group| -> String {
        if group.section >= 0 {
            format!(
                "<g class=\"{p}-cluster {p}-section\" data-id=\"{}\">",
                esc_text(js::trim(&model.sections[group.section as usize]))
            )
        } else {
            String::new()
        }
    };
    let open_period = |i: usize| -> String {
        format!(
            "<g class=\"{p}-node {p}-period\" data-id=\"{}\">",
            esc_text(js::trim(&periods[i].text))
        )
    };

    let mut body = String::new();
    let height = if vertical {
        let axis_x = left + col_w + AXIS;
        let spine_x = axis_x + SPINE;
        let events_x = spine_x + SPINE;
        let mut y = top;
        for group in &groups {
            body += &open(group);
            if group.section >= 0 {
                let label = section_label(group, body_w);
                let h = chip_height(&label);
                body += &header(group, &label, left, y, body_w, h);
                y += h + SECTION_GAP + 4.0;
            }
            for &i in &group.periods {
                let color = color_of(group, i);
                let h = chip_height(&period_labels[i]);
                let cy = y + h / 2.0;
                let mut d = format!("M{},{}H{}", num(left + col_w), num(cy), num(axis_x));
                let mut cards = String::new();
                let mut ey = y;
                let mut last = cy;
                for label in &event_labels[i] {
                    let eh = event_height(label);
                    last = ey + eh / 2.0;
                    let _ = write!(d, "M{},{}H{}", num(spine_x), num(last), num(events_x));
                    cards += &event(label, &color, events_x, ey, eh);
                    ey += eh + EVENT_GAP;
                }
                if !cards.is_empty() {
                    let _ = write!(
                        d,
                        "M{},{}H{}V{}",
                        num(axis_x),
                        num(cy),
                        num(spine_x),
                        num(last)
                    );
                }
                body += &open_period(i);
                body += &period(i, &color, left, y, h);
                let _ = write!(body, "<path class=\"{p}-edge\" d=\"{d}\"{LINE}/>");
                body += &dot(axis_x, cy, &color);
                body += &cards;
                body += "</g>";
                y = (y + h).max(ey - EVENT_GAP) + GAP;
            }
            if group.periods.is_empty() {
                y += GAP - SECTION_GAP - 4.0;
            }
            if group.section >= 0 {
                body += "</g>";
            }
        }
        if !groups.is_empty() {
            y -= GAP;
        }
        if drawn {
            let tip = y + TAIL + ARROW;
            body = format!(
                "<path class=\"{p}-axis\" d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(axis_x),
                num(top),
                num(tip - ARROW + 1.0)
            ) + &marker("arrow_point", axis_x, tip, 0.0, 1.0, "var(--_l)")
                + &body;
            y = tip;
        }
        y + pad
    } else {
        let mut header_h: f64 = 0.0;
        let section_labels: Vec<Option<Label>> = groups
            .iter()
            .map(|group| {
                if group.section < 0 {
                    return None;
                }
                let label = section_label(group, group.periods.len().max(1) as f64 * pitch - GAP);
                header_h = header_h.max(chip_height(&label));
                Some(label)
            })
            .collect();
        let mut chip_h: f64 = 0.0;
        for label in &period_labels {
            chip_h = chip_h.max(chip_height(label));
        }
        let periods_y = top
            + if has_sections {
                header_h + SECTION_GAP
            } else {
                0.0
            };
        let axis_y = periods_y + chip_h + if chip_h > 0.0 { AXIS } else { 0.0 };
        let events_y = axis_y + AXIS;
        let mut bottom = if drawn { axis_y + TAIL } else { top + header_h };
        let mut slot = 0;
        for (g, group) in groups.iter().enumerate() {
            let span = group.periods.len().max(1);
            let gx = left + slot as f64 * pitch;
            body += &open(group);
            if let Some(label) = &section_labels[g] {
                body += &header(group, label, gx, top, span as f64 * pitch - GAP, header_h);
            }
            for (k, &i) in group.periods.iter().enumerate() {
                let color = color_of(group, i);
                let x = gx + k as f64 * pitch;
                let cx = x + col_w / 2.0;
                let mut d = format!(
                    "M{},{}V{}",
                    num(cx),
                    num(periods_y + chip_h),
                    num(if !event_labels[i].is_empty() {
                        events_y
                    } else {
                        axis_y
                    })
                );
                let mut cards = String::new();
                let mut ey = events_y;
                for event_label in &event_labels[i] {
                    let eh = event_height(event_label);
                    if ey > events_y {
                        let _ = write!(d, "M{},{}V{}", num(cx), num(ey - EVENT_GAP), num(ey));
                    }
                    cards += &event(event_label, &color, x, ey, eh);
                    ey += eh + EVENT_GAP;
                }
                bottom = bottom.max(ey - EVENT_GAP);
                body += &open_period(i);
                body += &period(i, &color, x, periods_y, chip_h);
                let _ = write!(body, "<path class=\"{p}-edge\" d=\"{d}\"{LINE}/>");
                body += &dot(cx, axis_y, &color);
                body += &cards;
                body += "</g>";
            }
            if group.section >= 0 {
                body += "</g>";
            }
            slot += span;
        }
        if drawn {
            let tip = left + body_w;
            body = format!(
                "<path class=\"{p}-axis\" d=\"M{},{}H{}\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(left),
                num(axis_y),
                num(tip - ARROW + 1.0)
            ) + &marker("arrow_point", tip, axis_y, 1.0, 0.0, "var(--_l)")
                + &body;
        }
        bottom + pad
    };

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = height.max(pad * 2.0).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &body;
    Rendered {
        svg: svg_document(
            "timeline",
            total_width,
            total_height,
            size,
            options,
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
