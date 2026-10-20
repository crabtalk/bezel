//! Pele's `diagrams/timeline/render.ts`.

use super::db::TimelineModel;
use crate::diagrams::Render;
use crate::geom::{Point, Size};
use crate::svg::edges::Marker;
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered, js,
    svg::{
        builder::{EscText, num, rect},
        edges::ARROW,
        root::svg_document,
        theme::{RADIUS, SeriesColor, series_color},
    },
    text::{Label, Style},
};
use serde::Deserialize;
use serde_with::{DefaultOnError, serde_as};
use std::fmt::{self, Write as _};

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
    /// Index into the model's sections, `None` for the periods before the
    /// first.
    section: Option<usize>,
    periods: Vec<usize>,
}

/// A period's colour: its series, or muted.
#[derive(Clone, Copy)]
enum Hue {
    Series(SeriesColor),
    Muted,
}

impl fmt::Display for Hue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hue::Series(color) => write!(f, "{color}"),
            Hue::Muted => f.write_str("var(--_m)"),
        }
    }
}

/// Mermaid's `timeline` config.
#[serde_as]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TimelineConfig {
    #[serde(flatten)]
    pub base: crate::config::BaseConfig,
    #[serde_as(as = "DefaultOnError")]
    pub disable_multicolor: Option<bool>,
}

impl Render for TimelineModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.timeline;
        Ok(if self.direction.is_vertical() {
            self.draw(config, ctx, true)
        } else {
            ctx.turn_to_fit(|down| self.draw(config, ctx, down))
        })
    }
}

impl TimelineModel {
    fn draw(&self, config: &TimelineConfig, ctx: &Ctx<'_, impl Host>, vertical: bool) -> Rendered {
        let size = ctx.options.font_size;
        let small = js::round(size * 0.875);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);
        let p = crate::PREFIX;
        let multicolor = config.disable_multicolor != Some(true);
        let periods = &self.periods;
        let has_sections = !self.sections.is_empty();

        // Mermaid draws nothing for periods that come before the first section;
        // here they lead, in a neutral color.
        let mut groups: Vec<Group> = (0..self.sections.len())
            .map(|section| Group {
                section: Some(section),
                periods: Vec::new(),
            })
            .collect();
        let mut leading = Group {
            section: None,
            periods: Vec::new(),
        };
        for (i, period) in periods.iter().enumerate() {
            match period.section_index.and_then(|index| groups.get_mut(index)) {
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
        let room = match ctx.options.max_width {
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
            .map(|period| Label::plain(Some(&period.text), measure, size, text_w, 0))
            .collect();
        let mut event_labels: Vec<Vec<Label>> = periods
            .iter()
            .map(|period| {
                period
                    .events
                    .iter()
                    .map(|event| Label::plain(Some(event), measure, small, event_text_w(text_w), 0))
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
                .map(|period| Label::fit(Some(&period.text), measure, size, text_w, 0))
                .collect();
            event_labels = periods
                .iter()
                .map(|period| {
                    period
                        .events
                        .iter()
                        .map(|event| {
                            Label::fit(Some(event), measure, small, event_text_w(text_w), 0)
                        })
                        .collect()
                })
                .collect();
        }

        let line_height = js::round(size * 1.5);
        let col_w = text_w + 2.0 * PAD_X;
        let event_w = event_text_w(text_w) + 2.0 * PAD_X;
        let chip_height = |label: &Label| label.height.max(line_height) + 2.0 * PAD_Y;
        let event_height = |label: &Label| label.height.max(js::round(small * 1.5)) + 2.0 * PAD_Y;
        let color_of = |group: &Group, period: usize| match group.section {
            Some(section) => Hue::Series(series_color(section as i64)),
            None if has_sections => Hue::Muted,
            None => Hue::Series(series_color(if multicolor { period as i64 } else { 0 })),
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
        let title = Label::fit(
            self.titles.title.as_deref(),
            measure,
            size,
            body_w.max(320.0),
            Style::BOLD,
        );
        let title_h = title.spaced_height(16.0);
        let width = body_w.max(title.width);
        let left = pad + (width - body_w) / 2.0;
        let top = pad + title_h;

        let section_label = |section: usize, w: f64| -> Label {
            Label::fit(
                Some(&self.sections[section]),
                measure,
                size,
                w - 2.0 * PAD_X,
                Style::BOLD,
            )
        };
        let header_attrs = format!(" font-weight=\"var(--_hw)\"{ON_COLOR}");
        let header = |out: &mut String, section: usize, label: &Label, at: Point, chip: Size| {
            let _ = write!(
                out,
                "{}{}",
                rect(
                    at,
                    chip,
                    format_args!(" rx=\"{RADIUS}\" fill=\"{}\"", series_color(section as i64))
                ),
                label.svg(
                    Point::new(at.x + chip.w / 2.0, at.y + chip.h / 2.0),
                    &header_attrs
                )
            );
        };
        let period = |out: &mut String, i: usize, color: Hue, at: Point, h: f64| {
            let _ = write!(
                out,
                "{}{}",
                rect(
                    at,
                    Size::new(col_w, h),
                    format_args!(" rx=\"{RADIUS}\" fill=\"{color}\"")
                ),
                period_labels[i].svg(Point::new(at.x + col_w / 2.0, at.y + h / 2.0), ON_COLOR)
            );
        };
        let event_attrs = format!(" font-size=\"{}\"", js::Number(small));
        let event = |out: &mut String, label: &Label, color: Hue, at: Point, h: f64| {
            let _ = write!(
                out,
                "<g class=\"{p}-node {p}-event\">{}{}</g>",
                rect(
                    at,
                    Size::new(event_w, h),
                    format_args!(" rx=\"{RADIUS}\" fill=\"{color}\" fill-opacity=\"0.16\"")
                ),
                label.svg(
                    Point::new(at.x + event_w / 2.0, at.y + h / 2.0),
                    &event_attrs
                )
            );
        };
        let dot = |out: &mut String, at: Point, color: Hue| {
            let _ = write!(
                out,
                "<circle class=\"{p}-dot\" cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\" stroke=\"var(--_bg)\" stroke-width=\"2\"/>",
                num(at.x),
                num(at.y)
            );
        };
        let open = |out: &mut String, group: &Group| {
            if let Some(section) = group.section {
                let _ = write!(
                    out,
                    "<g class=\"{p}-cluster {p}-section\" data-id=\"{}\">",
                    EscText(js::trim(&self.sections[section]))
                );
            }
        };
        let open_period = |out: &mut String, i: usize| {
            let _ = write!(
                out,
                "<g class=\"{p}-node {p}-period\" data-id=\"{}\">",
                EscText(js::trim(&periods[i].text))
            );
        };

        let mut body = String::new();
        let mut axis = String::new();
        let height = if vertical {
            let axis_x = left + col_w + AXIS;
            let spine_x = axis_x + SPINE;
            let events_x = spine_x + SPINE;
            let mut y = top;
            for group in &groups {
                open(&mut body, group);
                if let Some(section) = group.section {
                    let label = section_label(section, body_w);
                    let h = chip_height(&label);
                    header(
                        &mut body,
                        section,
                        &label,
                        Point::new(left, y),
                        Size::new(body_w, h),
                    );
                    y += h + SECTION_GAP + 4.0;
                }
                for &i in &group.periods {
                    let color = color_of(group, i);
                    let h = chip_height(&period_labels[i]);
                    let cy = y + h / 2.0;
                    open_period(&mut body, i);
                    period(&mut body, i, color, Point::new(left, y), h);
                    let _ = write!(
                        body,
                        "<path class=\"{p}-edge\" d=\"M{}H{}",
                        Point::new(left + col_w, cy),
                        num(axis_x)
                    );
                    let mut ey = y;
                    let mut last = cy;
                    for label in &event_labels[i] {
                        let eh = event_height(label);
                        last = ey + eh / 2.0;
                        let _ = write!(body, "M{}H{}", Point::new(spine_x, last), num(events_x));
                        ey += eh + EVENT_GAP;
                    }
                    if !event_labels[i].is_empty() {
                        let _ = write!(
                            body,
                            "M{}H{}V{}",
                            Point::new(axis_x, cy),
                            num(spine_x),
                            num(last)
                        );
                    }
                    let _ = write!(body, "\"{LINE}/>");
                    dot(&mut body, Point::new(axis_x, cy), color);
                    let mut ey = y;
                    for label in &event_labels[i] {
                        let eh = event_height(label);
                        event(&mut body, label, color, Point::new(events_x, ey), eh);
                        ey += eh + EVENT_GAP;
                    }
                    body.push_str("</g>");
                    y = (y + h).max(ey - EVENT_GAP) + GAP;
                }
                if group.periods.is_empty() {
                    y += GAP - SECTION_GAP - 4.0;
                }
                if group.section.is_some() {
                    body.push_str("</g>");
                }
            }
            if !groups.is_empty() {
                y -= GAP;
            }
            if drawn {
                let tip = y + TAIL + ARROW;
                let _ = write!(
                    axis,
                    "<path class=\"{p}-axis\" d=\"M{}V{}\" fill=\"none\" stroke=\"var(--_l)\"/>{}",
                    Point::new(axis_x, top),
                    num(tip - ARROW + 1.0),
                    Marker::Arrow.draw(Point::new(axis_x, tip), Point::new(0.0, 1.0), "var(--_l)")
                );
                y = tip;
            }
            y + pad
        } else {
            let mut header_h: f64 = 0.0;
            let section_labels: Vec<Option<Label>> = groups
                .iter()
                .map(|group| {
                    let label = section_label(
                        group.section?,
                        group.periods.len().max(1) as f64 * pitch - GAP,
                    );
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
                open(&mut body, group);
                if let (Some(section), Some(label)) = (group.section, &section_labels[g]) {
                    header(
                        &mut body,
                        section,
                        label,
                        Point::new(gx, top),
                        Size::new(span as f64 * pitch - GAP, header_h),
                    );
                }
                for (k, &i) in group.periods.iter().enumerate() {
                    let color = color_of(group, i);
                    let x = gx + k as f64 * pitch;
                    let cx = x + col_w / 2.0;
                    open_period(&mut body, i);
                    period(&mut body, i, color, Point::new(x, periods_y), chip_h);
                    let _ = write!(
                        body,
                        "<path class=\"{p}-edge\" d=\"M{}V{}",
                        Point::new(cx, periods_y + chip_h),
                        num(if !event_labels[i].is_empty() {
                            events_y
                        } else {
                            axis_y
                        })
                    );
                    let mut ey = events_y;
                    for event_label in &event_labels[i] {
                        let eh = event_height(event_label);
                        if ey > events_y {
                            let _ = write!(body, "M{}V{}", Point::new(cx, ey - EVENT_GAP), num(ey));
                        }
                        ey += eh + EVENT_GAP;
                    }
                    bottom = bottom.max(ey - EVENT_GAP);
                    let _ = write!(body, "\"{LINE}/>");
                    dot(&mut body, Point::new(cx, axis_y), color);
                    let mut ey = events_y;
                    for event_label in &event_labels[i] {
                        let eh = event_height(event_label);
                        event(&mut body, event_label, color, Point::new(x, ey), eh);
                        ey += eh + EVENT_GAP;
                    }
                    body.push_str("</g>");
                }
                if group.section.is_some() {
                    body.push_str("</g>");
                }
                slot += span;
            }
            if drawn {
                let tip = left + body_w;
                let _ = write!(
                    axis,
                    "<path class=\"{p}-axis\" d=\"M{}H{}\" fill=\"none\" stroke=\"var(--_l)\"/>{}",
                    Point::new(left, axis_y),
                    num(tip - ARROW + 1.0),
                    Marker::Arrow.draw(Point::new(tip, axis_y), Point::new(1.0, 0.0), "var(--_l)")
                );
            }
            bottom + pad
        };

        let total_width = (width + 2.0 * pad).ceil();
        let total_height = height.max(pad * 2.0).ceil();
        let title_attrs = crate::svg::builder::TITLE;
        let mut content = String::with_capacity(axis.len() + body.len() + 512);
        let _ = write!(
            content,
            "{}{axis}{body}",
            title.svg(
                Point::new(total_width / 2.0, pad + title.height / 2.0),
                title_attrs
            )
        );
        Rendered {
            svg: svg_document(
                "timeline",
                total_width,
                total_height,
                size,
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
