//! Pele's `diagrams/journey/render.ts`.

use crate::geom::Point;
use crate::svg::builder::EscText;
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::db::JourneyModel;
use crate::diagrams::Render;
use crate::{
    Config, Error, Rendered, js,
    svg::{
        builder::{TITLE, fragment, num},
        root::svg_document,
        theme::{RADIUS, series_color},
    },
    text::{Label, Style},
};
use crate::{Ctx, Host};

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

/// How a task felt, by its score. A score that is not a number is neutral.
#[derive(Clone, Copy)]
enum Mood {
    Happy,
    Neutral,
    Sad,
}

impl Mood {
    fn of(score: f64) -> Self {
        if score > 3.0 {
            Mood::Happy
        } else if score < 3.0 {
            Mood::Sad
        } else {
            Mood::Neutral
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mood::Happy => "happy",
            Mood::Neutral => "neutral",
            Mood::Sad => "sad",
        }
    }

    fn mouth(self) -> &'static str {
        match self {
            Mood::Happy => "M-5,2A6,6 0 0 0 5,2",
            Mood::Neutral => "M-4.5,4.5H4.5",
            Mood::Sad => "M-5,6.5A6,6 0 0 1 5,6.5",
        }
    }

    /// The face centred on `at`.
    fn face(self, at: Point) -> impl fmt::Display {
        fragment(move |f| {
            write!(
                f,
                "<g class=\"{p}-face {p}-face-{}\" transform=\"translate({},{})\"><circle r=\"{}\" fill=\"var(--_bg)\" stroke=\"var(--_l)\"/><g fill=\"var(--_l)\">{EYES}</g><path d=\"{}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linecap=\"round\"/></g>",
                self.name(),
                num(at.x),
                num(at.y),
                js::Number(FACE),
                self.mouth(),
                p = crate::PREFIX,
            )
        })
    }
}

impl Render for JourneyModel {
    fn render(&self, _config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        Ok(ctx.turn_to_fit(|down| {
            if down {
                self.draw_down(ctx)
            } else {
                self.draw(ctx)
            }
        }))
    }
}

struct LegendItem {
    name: String,
    label: Label,
    /// The item's top left, from the legend's.
    at: Point,
    h: f64,
}

struct Run {
    start: usize,
    count: usize,
    label: Option<Label>,
}

impl JourneyModel {
    /// Everyone with a name, in legend order. `Task: 5:` names an actor with
    /// no name; Mermaid gives it a dot and an empty legend row.
    fn named_actors(&self) -> Vec<String> {
        self.actors()
            .into_iter()
            .filter(|name| !name.is_empty())
            .collect()
    }

    fn draw(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let tiny = js::round(size * 0.75);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;
        let tasks = &self.tasks;
        let n = tasks.len();

        let names = self.named_actors();
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
            .map(|task| Label::plain(Some(&task.task), measure, small, text_w, 0))
            .collect();
        let mut widest: f64 = 0.0;
        for label in &labels {
            widest = widest.max(label.width);
        }
        if widest > text_w {
            text_w = widest.ceil().min(MAX_TEXT);
            labels = tasks
                .iter()
                .map(|task| Label::fit(Some(&task.task), measure, small, text_w, 0))
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
            let label = Label::fit(Some(name), measure, small, limit - 2.0 * DOT - 6.0, 0);
            let w = 2.0 * DOT + 6.0 + label.width;
            if lx > 0.0 && lx + w > limit {
                lx = 0.0;
                legend_h += 4.0;
            }
            let y = match legend.last() {
                Some(last) if lx > 0.0 => last.at.y,
                _ => legend_h,
            };
            let h = label.height.max(line_h);
            legend.push(LegendItem {
                name: name.clone(),
                label,
                at: Point::new(lx, y),
                h,
            });
            lx += w;
            legend_w = legend_w.max(lx);
            legend_h = legend_h.max(y + h);
            lx += LEGEND_GAP;
        }

        let body_w = if n > 0 { GUTTER + plot_w } else { 0.0 };
        let title = Label::fit(
            self.titles.title.as_deref(),
            measure,
            size,
            body_w.max(320.0),
            Style::BOLD,
        );
        let title_h = title.spaced_height(16.0);
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
                let fitted = Label::fit(
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

        let mut body = String::with_capacity(16 * 1024);
        let _ = write!(
            body,
            "{}",
            title.svg(
                Point::new(pad + width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        let mut bottom = top;
        if n > 0 {
            let mut grid = String::new();
            let mut scale = String::new();
            for score in (1..=5).rev() {
                let y = chart_top + (5 - score) as f64 * LEVEL;
                let _ = write!(grid, "M{},{}H{}", num(left - 6.0), num(y), num(right));
                let _ = write!(
                    scale,
                    "<text x=\"{}\" y=\"{}\">{score}</text>",
                    num(left - 12.0),
                    num(y + tiny * 0.35)
                );
            }
            let _ = write!(
                body,
                "<path class=\"{p}-grid\" d=\"{grid}\" fill=\"none\" stroke=\"var(--_a)\"/><g class=\"{p}-scale\" font-size=\"{}\" text-anchor=\"end\" fill=\"var(--_m)\">{scale}</g>",
                js::Number(tiny)
            );

            let mut line = String::new();
            let mut stems = String::new();
            let mut faces = String::new();
            let mut cards = String::new();
            // Where each task's card in `cards` ends.
            let mut ends: Vec<usize> = Vec::with_capacity(tasks.len());
            let card_attrs = format!(" font-size=\"{}\"", js::Number(small));
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
                let _ = write!(
                    line,
                    "{}{},{}",
                    if i == 0 { "M" } else { "L" },
                    num(cx),
                    num(y)
                );
                let _ = write!(
                    stems,
                    "M{},{}V{}",
                    num(cx),
                    num(cards_y + card_h),
                    num(y - FACE)
                );
                let _ = write!(faces, "{}", Mood::of(score).face(Point::new(cx, y)));

                let _ = write!(
                    cards,
                    "<g class=\"{p}-node {p}-task\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>{}",
                    EscText(js::trim(&task.task)),
                    num(x),
                    num(cards_y),
                    num(card_w),
                    num(card_h),
                    labels[i].svg(Point::new(cx, cards_y + PAD_Y + text_h / 2.0), &card_attrs),
                );
                let people: Vec<&String> =
                    task.people.iter().filter(|name| !name.is_empty()).collect();
                let count = people.len() as f64;
                let step =
                    DOT_STEP.min((card_w - 2.0 * PAD_X - 2.0 * DOT) / (count - 1.0).max(1.0));
                for (k, name) in people.iter().enumerate() {
                    let _ = write!(
                        cards,
                        "<circle class=\"{p}-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"var(--_s)\"><title>{}</title></circle>",
                        num(cx + (k as f64 - (count - 1.0) / 2.0) * step),
                        num(cards_y + card_h - PAD_Y - DOT),
                        js::Number(DOT),
                        series_color(actor.get(name.as_str()).copied().unwrap_or(0)),
                        EscText(name)
                    );
                }
                cards.push_str("</g>");
                ends.push(cards.len());
            }
            let _ = write!(
                body,
                "<path class=\"{p}-stems\" d=\"{stems}\" fill=\"none\" stroke=\"var(--_b)\" stroke-dasharray=\"2 3\"/>"
            );
            if n > 1 {
                let _ = write!(
                    body,
                    "<path class=\"{p}-edge\" d=\"{line}\" fill=\"none\" stroke=\"var(--_l)\" stroke-linejoin=\"round\"/>"
                );
            }
            body.push_str(&faces);

            let band_attrs = format!(
                " font-size=\"{}\" font-weight=\"var(--_hw)\"",
                js::Number(small)
            );
            for run in &runs {
                let from = if run.start == 0 {
                    0
                } else {
                    ends[run.start - 1]
                };
                let members = &cards[from..ends[run.start + run.count - 1]];
                let Some(label) = &run.label else {
                    body.push_str(members);
                    continue;
                };
                let x = left + run.start as f64 * pitch;
                let w = run.count as f64 * pitch - GAP;
                let _ = write!(
                    body,
                    "<g class=\"{p}-cluster {p}-section\" data-id=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_a)\"/>{}{members}</g>",
                    EscText(js::trim(&tasks[run.start].section)),
                    num(x),
                    num(top),
                    num(w),
                    num(band_h),
                    label.svg(Point::new(x + w / 2.0, top + band_h / 2.0), &band_attrs),
                );
            }
            bottom = chart_bottom + FACE;

            if !legend.is_empty() {
                let x0 = left + (plot_w - legend_w) / 2.0;
                let y0 = bottom + 16.0;
                let mut items = String::new();
                for item in &legend {
                    let cy = y0 + item.at.y + item.h / 2.0;
                    let _ = write!(
                        items,
                        "<g class=\"{p}-legend-item\" data-id=\"{}\"><circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>{}</g>",
                        EscText(&item.name),
                        num(x0 + item.at.x + DOT),
                        num(cy),
                        js::Number(DOT),
                        series_color(actor.get(item.name.as_str()).copied().unwrap_or(0)),
                        item.label.svg(
                            Point::new(
                                x0 + item.at.x + 2.0 * DOT + 6.0 + item.label.width / 2.0,
                                cy
                            ),
                            ""
                        ),
                    );
                }
                let _ = write!(
                    body,
                    "<g class=\"{p}-legend\" font-size=\"{}\">{items}</g>",
                    js::Number(small)
                );
                bottom = y0 + legend_h;
            }
        }

        let total_width = (width + 2.0 * pad).ceil();
        let total_height = (bottom.max(pad) + pad).ceil();
        Rendered {
            svg: svg_document(
                "journey",
                total_width,
                total_height,
                size,
                ctx,
                self.titles.accessible(),
                &body,
            ),
            width: total_width,
            height: total_height,
            links: Vec::new(),
        }
    }

    /// The journey as a column of steps.
    fn draw_down(&self, ctx: &Ctx<'_, impl Host>) -> Rendered {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;
        let room = 140f64.max(ctx.options.max_width.unwrap_or(320.0) - 2.0 * pad);
        let title = Label::fit(
            self.titles.title.as_deref(),
            measure,
            size,
            room,
            Style::BOLD,
        );
        let names = self.named_actors();
        let actors: HashMap<&str, i64> = names
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i as i64))
            .collect();
        let mut y = pad + title.spaced_height(16.0);
        let mut section: Option<&str> = None;
        let mut body = String::new();
        let mut width = title.width;
        let count = self.tasks.len();
        let actor_attrs = format!(
            " class=\"{p}-journey-actor\" font-size=\"{}\"",
            js::Number(small)
        );
        for (index, task) in self.tasks.iter().enumerate() {
            if section != Some(task.section.as_str()) {
                section = Some(&task.section);
                let head = Label::fit(section, measure, size, room - 16.0, Style::BOLD);
                if head.height > 0.0 {
                    let _ = write!(
                        body,
                        "{}",
                        head.svg(
                            Point::new(pad + 8.0 + head.width / 2.0, y + head.height / 2.0),
                            crate::class!("cluster-label", "font-weight=\"var(--_hw)\"")
                        )
                    );
                    width = width.max(head.width + 16.0);
                    y += head.height + 8.0;
                }
            }
            let label = Label::fit(Some(&task.task), measure, small, room - 72.0, 0);
            let people: Vec<(&str, Label)> = task
                .people
                .iter()
                .filter(|name| !name.is_empty())
                .map(|name| {
                    (
                        name.as_str(),
                        Label::fit(Some(name), measure, small, room - 44.0, 0),
                    )
                })
                .collect();
            let text_h = label.height.max(42.0);
            let mut h = 2.0 * PAD_Y + text_h;
            for (_, person) in &people {
                h += 16f64.max(person.height) + 4.0;
            }
            let w = room.max(label.width + 72.0);
            let _ = write!(
                body,
                "<g class=\"{p}-node {p}-journey-step\" data-id=\"{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"var(--_s)\" stroke=\"var(--_b)\"/>",
                num(pad),
                num(y),
                num(w),
                num(h)
            );
            let _ = write!(
                body,
                "{}",
                label.svg(
                    Point::new(
                        pad + PAD_X + label.width / 2.0,
                        y + PAD_Y + label.height / 2.0
                    ),
                    crate::class!("label")
                )
            );
            let cx = pad + w - 28.0;
            let _ = write!(
                body,
                "{}",
                Mood::of(task.score).face(Point::new(cx, y + PAD_Y + FACE))
            );
            if !task.score.is_nan() {
                let _ = write!(
                    body,
                    "<text class=\"{p}-journey-score\" x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"middle\">{}</text>",
                    num(cx),
                    num(y + PAD_Y + 2.0 * FACE + small),
                    js::Number(small),
                    num(task.score.clamp(1.0, 5.0))
                );
            }
            let mut py = y + PAD_Y + text_h + 4.0;
            for (name, person) in &people {
                let ph = 16f64.max(person.height);
                let _ = write!(
                    body,
                    "<circle class=\"{p}-actor\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\"/>",
                    num(pad + 16.0),
                    num(py + ph / 2.0),
                    js::Number(DOT),
                    series_color(actors[name])
                );
                let _ = write!(
                    body,
                    "{}",
                    person.svg(
                        Point::new(pad + 28.0 + person.width / 2.0, py + ph / 2.0),
                        &actor_attrs
                    )
                );
                py += ph + 4.0;
            }
            body.push_str("</g>");
            y += h;
            if index + 1 < count {
                let _ = write!(
                    body,
                    "<path class=\"{p}-edge\" d=\"M{},{}v16\" fill=\"none\" stroke=\"var(--_l)\"/>",
                    num(pad + w / 2.0),
                    num(y)
                );
            }
            y += 16.0;
            width = width.max(w);
        }
        let total_width = (width + 2.0 * pad).ceil();
        let height =
            ((pad + title.height).max(y - if count > 0 { 16.0 } else { 0.0 }) + pad).ceil();
        let mut content = String::with_capacity(body.len() + 512);
        let _ = write!(
            content,
            "{}{body}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                TITLE
            )
        );
        Rendered {
            svg: svg_document(
                "journey",
                total_width,
                height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width: total_width,
            height,
            links: Vec::new(),
        }
    }
}
