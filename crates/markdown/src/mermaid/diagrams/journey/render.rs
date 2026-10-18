//! Pele's `diagrams/journey/render.ts`.

use std::collections::HashMap;

use super::db::{JourneyModel, journey_actors};
use crate::mermaid::{
    Config, Options, Rendered,
    diagrams::common::{
        fit::{fit_label, plain_label},
        fit_width::turn_to_fit,
    },
    js,
    svg::{
        builder::{esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::{RADIUS, series_color},
    },
    text::{Label, Style},
};

const PAD_X: f64 = 10.0;
const PAD_Y: f64 = 8.0;
const MIN_TEXT: f64 = 124.0;
const MAX_TEXT: f64 = 200.0;
const GAP: f64 = 12.0;
const BAND_PAD: f64 = 6.0;
const GUTTER: f64 = 24.0;
const LEVEL: f64 = 26.0;
const FACE: f64 = 11.0;
const DOT: f64 = 5.0;
const DOT_STEP: f64 = 13.0;
const LEGEND_GAP: f64 = 20.0;
const EYES: &str =
    "<circle cx=\"-3.6\" cy=\"-2.8\" r=\"1.3\"/><circle cx=\"3.6\" cy=\"-2.8\" r=\"1.3\"/>";

fn mouth(mood: &str) -> &'static str {
    match mood {
        "happy" => "M-5,2A6,6 0 0 0 5,2",
        "neutral" => "M-4.5,4.5H4.5",
        _ => "M-5,6.5A6,6 0 0 1 5,6.5",
    }
}

fn mood(score: f64) -> &'static str {
    if score > 3.0 {
        "happy"
    } else if score < 3.0 {
        "sad"
    } else {
        "neutral"
    }
}

fn face(x: f64, y: f64, mood: &str) -> String {
    let p = crate::mermaid::PREFIX;
    format!(
        "<g class=\"{p}-face {p}-face-{mood}\" transform=\"translate({},{})\"><circle r=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"/><g fill=\"var(--_l)\">{EYES}</g><path d=\"{}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\"/></g>",
        num(x),
        num(y),
        js::num_str(FACE),
        mouth(mood),
    )
}

fn accessible(model: &JourneyModel) -> Accessible<'_> {
    Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    }
}

pub fn render_journey(model: &JourneyModel, config: &Config, options: &Options) -> Rendered {
    turn_to_fit(options, |down| draw(model, config, options, down))
}

struct LegendItem {
    name: String,
    label: Label,
    x: f64,
    y: f64,
    h: f64,
}

struct Run {
    start: usize,
    count: usize,
    label: Option<Label>,
}

fn draw(model: &JourneyModel, _config: &Config, options: &Options, down: bool) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.75);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);
    if down {
        return vertical_journey(model, options);
    }
    let p = crate::mermaid::PREFIX;
    let tasks = &model.tasks;
    let n = tasks.len();

    // `Task: 5:` names an actor with no name; Mermaid gives it a dot and an
    // empty legend row.
    let names: Vec<String> = journey_actors(tasks)
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect();
    let actor: HashMap<&str, i64> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i as i64))
        .collect();

    // Cards widen, up to a point, for a word that would not fit; past that
    // the word is cut.
    let mut text_w = MIN_TEXT;
    let mut labels: Vec<Label> = tasks
        .iter()
        .map(|task| plain_label(Some(&task.task), measure, small, text_w, 0))
        .collect();
    let mut widest: f64 = 0.0;
    for label in &labels {
        widest = widest.max(label.width);
    }
    if widest > text_w {
        text_w = widest.ceil().min(MAX_TEXT);
        labels = tasks
            .iter()
            .map(|task| fit_label(Some(&task.task), measure, small, text_w, 0))
            .collect();
    }

    let card_w = text_w + 2.0 * PAD_X;
    let pitch = card_w + GAP;
    let plot_w = if n > 0 { n as f64 * pitch - GAP } else { 0.0 };

    let line_h = js::round(small * 1.5);
    let limit = plot_w.max(240.0);
    let mut legend: Vec<LegendItem> = Vec::new();
    let mut legend_w: f64 = 0.0;
    let mut legend_h: f64 = 0.0;
    let mut lx = 0.0;
    for name in &names {
        let label = fit_label(Some(name), measure, small, limit - 2.0 * DOT - 6.0, 0);
        let w = 2.0 * DOT + 6.0 + label.width;
        if lx > 0.0 && lx + w > limit {
            lx = 0.0;
            legend_h += 4.0;
        }
        let y = if lx > 0.0 {
            legend[legend.len() - 1].y
        } else {
            legend_h
        };
        let h = label.height.max(line_h);
        legend.push(LegendItem {
            name: name.clone(),
            label,
            x: lx,
            y,
            h,
        });
        lx += w;
        legend_w = legend_w.max(lx);
        legend_h = legend_h.max(y + h);
        lx += LEGEND_GAP;
    }

    let body_w = if n > 0 { GUTTER + plot_w } else { 0.0 };
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
    let width = body_w
        .max(title.width)
        .max(if n > 0 { GUTTER + legend_w } else { 0.0 });
    let left = pad + (width - body_w) / 2.0 + GUTTER;
    let top = pad + title_h;

    // A band covers a run of tasks that share a section, as in Mermaid; a
    // section with no tasks draws nothing.
    let mut runs: Vec<Run> = Vec::new();
    let mut band_h: f64 = 0.0;
    let mut i = 0;
    while i < n {
        let mut end = i + 1;
        while end < n && tasks[end].section == tasks[i].section {
            end += 1;
        }
        let mut label = None;
        if !tasks[i].section.is_empty() {
            let fitted = fit_label(
                Some(&tasks[i].section),
                measure,
                small,
                (end - i) as f64 * pitch - GAP - 2.0 * PAD_X,
                Style::BOLD,
            );
            band_h = band_h.max(fitted.height.max(line_h) + 2.0 * BAND_PAD);
            label = Some(fitted);
        }
        runs.push(Run {
            start: i,
            count: end - i,
            label,
        });
        i = end;
    }

    let mut text_h = line_h;
    for label in &labels {
        text_h = text_h.max(label.height);
    }
    let dots_h = if !names.is_empty() {
        2.0 * DOT + 6.0
    } else {
        0.0
    };
    let card_h = text_h + dots_h + 2.0 * PAD_Y;
    let cards_y = top + if band_h > 0.0 { band_h + BAND_PAD } else { 0.0 };
    let chart_top = cards_y + card_h + FACE + 16.0;
    let chart_bottom = chart_top + 4.0 * LEVEL;
    let right = left + plot_w;

    let mut body = String::new();
    let mut bottom = top;
    if n > 0 {
        let mut grid = String::new();
        let mut scale = String::new();
        for score in (1..=5).rev() {
            let y = chart_top + (5 - score) as f64 * LEVEL;
            grid += &format!("M{},{}H{}", num(left - 6.0), num(y), num(right));
            scale += &format!(
                "<text x=\"{}\" y=\"{}\">{score}</text>",
                num(left - 12.0),
                num(y + tiny * 0.35)
            );
        }
        body += &format!(
            "<path class=\"{p}-grid\" d=\"{grid}\" fill=\"none\" stroke=\"var(--_a)\"/><g class=\"{p}-scale\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\">{scale}</g>",
            js::num_str(tiny)
        );

        let mut line = String::new();
        let mut stems = String::new();
        let mut faces = String::new();
        let mut cards: Vec<String> = Vec::new();
        for (i, task) in tasks.iter().enumerate() {
            let x = left + i as f64 * pitch;
            let cx = x + card_w / 2.0;
            let score = task.score;
            let level = if score.is_nan() {
                3.0
            } else {
                score.clamp(1.0, 5.0)
            };
            let y = chart_top + (5.0 - level) * LEVEL;
            line += &format!("{}{},{}", if i == 0 { "M" } else { "L" }, num(cx), num(y));
            stems += &format!("M{},{}V{}", num(cx), num(cards_y + card_h), num(y - FACE));
            faces += &face(cx, y, mood(score));

            let mut dots = String::new();
            let people: Vec<&String> = task.people.iter().filter(|name| !name.is_empty()).collect();
            let count = people.len() as f64;
            let step = DOT_STEP.min((card_w - 2.0 * PAD_X - 2.0 * DOT) / (count - 1.0).max(1.0));
            for (k, name) in people.iter().enumerate() {
                dots += &format!(
                    "<circle class=\"{p}-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"var(--_s)\"><title>{}</title></circle>",
                    num(cx + (k as f64 - (count - 1.0) / 2.0) * step),
                    num(cards_y + card_h - PAD_Y - DOT),
                    js::num_str(DOT),
                    series_color(actor.get(name.as_str()).copied().unwrap_or(0)),
                    esc_text(name)
                );
            }
            cards.push(format!(
                "<g class=\"{p}-node {p}-task\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>{}{dots}</g>",
                esc_text(js::trim(&task.task)),
                num(x),
                num(cards_y),
                num(card_w),
                num(card_h),
                label_svg(
                    &labels[i],
                    cx,
                    cards_y + PAD_Y + text_h / 2.0,
                    &format!(" font-size=\"{}\"", js::num_str(small))
                ),
            ));
        }
        body += &format!(
            "<path class=\"{p}-stems\" d=\"{stems}\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"2 3\"/>"
        );
        if n > 1 {
            body += &format!(
                "<path class=\"{p}-edge\" d=\"{line}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linejoin=\"round\"/>"
            );
        }
        body += &faces;

        for run in &runs {
            let members = cards[run.start..run.start + run.count].join("");
            let Some(label) = &run.label else {
                body += &members;
                continue;
            };
            let x = left + run.start as f64 * pitch;
            let w = run.count as f64 * pitch - GAP;
            body += &format!(
                "<g class=\"{p}-cluster {p}-section\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\"/>{}{members}</g>",
                esc_text(js::trim(&tasks[run.start].section)),
                num(x),
                num(top),
                num(w),
                num(band_h),
                label_svg(
                    label,
                    x + w / 2.0,
                    top + band_h / 2.0,
                    &format!(
                        " font-size=\"{}\" font-weight=\"var(--_hw)\"",
                        js::num_str(small)
                    )
                ),
            );
        }
        bottom = chart_bottom + FACE;

        if !legend.is_empty() {
            let x0 = left + (plot_w - legend_w) / 2.0;
            let y0 = bottom + 16.0;
            let mut items = String::new();
            for item in &legend {
                let cy = y0 + item.y + item.h / 2.0;
                items += &format!(
                    "<g class=\"{p}-legend-item\" data-id=\"{}\"><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>{}</g>",
                    esc_text(&item.name),
                    num(x0 + item.x + DOT),
                    num(cy),
                    js::num_str(DOT),
                    series_color(actor.get(item.name.as_str()).copied().unwrap_or(0)),
                    label_svg(
                        &item.label,
                        x0 + item.x + 2.0 * DOT + 6.0 + item.label.width / 2.0,
                        cy,
                        ""
                    ),
                );
            }
            body += &format!(
                "<g class=\"{p}-legend\" font-size=\"{}\">{items}</g>",
                js::num_str(small)
            );
            bottom = y0 + legend_h;
        }
    }

    let total_width = (width + 2.0 * pad).ceil();
    let total_height = (bottom.max(pad) + pad).ceil();
    let content = label_svg(
        &title,
        pad + width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &body;
    Rendered {
        svg: svg_document(
            "journey",
            total_width,
            total_height,
            size,
            options,
            accessible(model),
            &content,
        ),
        width: total_width,
        height: total_height,
        links: Vec::new(),
    }
}

fn vertical_journey(model: &JourneyModel, options: &Options) -> Rendered {
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);
    let p = crate::mermaid::PREFIX;
    let room = 140f64.max(options.max_width.unwrap_or(320.0) - 2.0 * pad);
    let title = fit_label(model.title.as_deref(), measure, size, room, Style::BOLD);
    let names: Vec<String> = journey_actors(&model.tasks)
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect();
    let actors: HashMap<&str, i64> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i as i64))
        .collect();
    let mut y = pad
        + if js::truthy(title.height) {
            title.height + 16.0
        } else {
            0.0
        };
    let mut section: Option<&str> = None;
    let mut body = String::new();
    let mut width = title.width;
    let count = model.tasks.len();
    for (index, task) in model.tasks.iter().enumerate() {
        if section != Some(task.section.as_str()) {
            section = Some(&task.section);
            let head = fit_label(section, measure, size, room - 16.0, Style::BOLD);
            if js::truthy(head.height) {
                body += &label_svg(
                    &head,
                    pad + 8.0 + head.width / 2.0,
                    y + head.height / 2.0,
                    &format!(" class=\"{p}-cluster-label\" font-weight=\"var(--_hw)\""),
                );
                width = width.max(head.width + 16.0);
                y += head.height + 8.0;
            }
        }
        let label = fit_label(Some(&task.task), measure, small, room - 72.0, 0);
        let people: Vec<(&str, Label)> = task
            .people
            .iter()
            .filter(|name| !name.is_empty())
            .map(|name| {
                (
                    name.as_str(),
                    fit_label(Some(name), measure, small, room - 44.0, 0),
                )
            })
            .collect();
        let text_h = label.height.max(42.0);
        let mut h = 2.0 * PAD_Y + text_h;
        for (_, person) in &people {
            h += 16f64.max(person.height) + 4.0;
        }
        let w = room.max(label.width + 72.0);
        body += &format!(
            "<g class=\"{p}-node {p}-journey-step\" data-id=\"{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
            num(pad),
            num(y),
            num(w),
            num(h)
        );
        body += &label_svg(
            &label,
            pad + PAD_X + label.width / 2.0,
            y + PAD_Y + label.height / 2.0,
            &format!(" class=\"{p}-label\""),
        );
        let cx = pad + w - 28.0;
        body += &face(cx, y + PAD_Y + FACE, mood(task.score));
        if !task.score.is_nan() {
            body += &format!(
                "<text class=\"{p}-journey-score\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"middle\">{}</text>",
                num(cx),
                num(y + PAD_Y + 2.0 * FACE + small),
                js::num_str(small),
                num(task.score.clamp(1.0, 5.0))
            );
        }
        let mut py = y + PAD_Y + text_h + 4.0;
        for (name, person) in &people {
            let ph = 16f64.max(person.height);
            body += &format!(
                "<circle class=\"{p}-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                num(pad + 16.0),
                num(py + ph / 2.0),
                js::num_str(DOT),
                series_color(actors[name])
            );
            body += &label_svg(
                person,
                pad + 28.0 + person.width / 2.0,
                py + ph / 2.0,
                &format!(
                    " class=\"{p}-journey-actor\" font-size=\"{}\"",
                    js::num_str(small)
                ),
            );
            py += ph + 4.0;
        }
        body += "</g>";
        y += h;
        if index + 1 < count {
            body += &format!(
                "<path class=\"{p}-edge\" d=\"M{},{}v16\" fill=\"none\" stroke=\"var(--_l)\"/>",
                num(pad + w / 2.0),
                num(y)
            );
        }
        y += 16.0;
        width = width.max(w);
    }
    let total_width = (width + 2.0 * pad).ceil();
    let height = ((pad + title.height).max(y - if count > 0 { 16.0 } else { 0.0 }) + pad).ceil();
    let content = label_svg(
        &title,
        total_width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    ) + &body;
    Rendered {
        svg: svg_document(
            "journey",
            total_width,
            height,
            size,
            options,
            accessible(model),
            &content,
        ),
        width: total_width,
        height,
        links: Vec::new(),
    }
}
