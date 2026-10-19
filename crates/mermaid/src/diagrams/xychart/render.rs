//! Pele's `diagrams/xychart/render.ts`.

use super::{
    db::{XAxis, XyChartModel},
    ticks::{tick_index, tick_step, tick_value, ticks_between},
};
use crate::{
    Config, Error, Options, Rendered,
    config::{Map, Value},
    detect::Kind,
    diagrams::common::fit_width::cap_width,
    js,
    svg::{
        builder::{esc, esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::series_color,
    },
    text::{Label, Style, layout_label},
};
use std::fmt::Write as _;

const TICK: f64 = 4.0;
const SWATCH: f64 = 12.0;
const ROW: f64 = 26.0;
const LEGEND_GAP: f64 = 24.0;
const LIMIT: f64 = 1e300;

struct Mark {
    at: f64,
    label: Label,
}

#[derive(Clone, Copy, PartialEq)]
enum Anchor {
    Start,
    Middle,
    End,
}

impl Anchor {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
}

fn on(config: &Map, key: &str, fallback: bool) -> bool {
    match config.get(key) {
        Some(Value::Bool(value)) => *value,
        _ => fallback,
    }
}

fn dimension(value: Option<&Value>, fallback: f64) -> f64 {
    match value {
        Some(Value::Number(value)) if *value > 0.0 => js::round(value.clamp(100.0, 4000.0)),
        _ => fallback,
    }
}

fn finite(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(-LIMIT, LIMIT))
}

fn widest(marks: &[Mark]) -> f64 {
    let mut width: f64 = 0.0;
    for mark in marks {
        width = width.max(mark.label.width);
    }
    width
}

/// Writes a label with its left edge, center or right edge at x. A plain
/// one-line label keeps its place when the text is wider or narrower than
/// measured.
fn aligned(label: &Label, x: f64, y: f64, anchor: Anchor) -> String {
    if label.lines.len() == 1 {
        let line = &label.lines[0];
        if line.len() == 1 && line[0].icon.is_none() && line[0].style == 0 {
            return format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\">{}</text>",
                num(x),
                num(y + label.size * 0.35),
                anchor.name(),
                esc(&line[0].text)
            );
        }
    }
    let shift = match anchor {
        Anchor::Start => label.width / 2.0,
        Anchor::End => -label.width / 2.0,
        Anchor::Middle => 0.0,
    };
    label_svg(label, x + shift, y, "")
}

pub fn render_xy_chart(
    model: &XyChartModel,
    config: &Config,
    options: &Options,
) -> Result<Rendered, Error> {
    if model.plots.is_empty() {
        return Err(
            Error::semantic("No Plot to render, please provide a plot with some data")
                .of(Kind::XyChart),
        );
    }
    let p = crate::PREFIX;
    let empty = Map::new();
    let chart = config.section("xyChart").unwrap_or(&empty);
    let size = options.font_size.unwrap_or(16.0);
    let small = js::round(size * 0.875);
    let tiny = js::round(size * 0.75);
    let measure = options.measurer();
    let pad = options.padding.unwrap_or(8.0);
    let natural = dimension(chart.get("width"), 700.0);
    let width = cap_width(options, natural, 240.0, 0.0);
    // A narrower chart is not as much shorter, so that the plot keeps room
    // for its marks.
    let height = js::round(dimension(chart.get("height"), 500.0) * 0.6f64.max(width / natural));
    let orientation = match model.orientation {
        Some(orientation) => Some(orientation),
        None => chart.get("chartOrientation").and_then(Value::as_str),
    };
    let horizontal = orientation == Some("horizontal");
    // The value of a bar is always written past its end, which is what
    // showDataLabelOutsideBar asks for in Mermaid. Inside, whether it fits
    // changes from bar to bar and with the width of the chart.
    let show_values = on(chart, "showDataLabel", false);
    let x = &model.x_axis;
    let plots = &model.plots;
    let text = |raw: Option<&str>, font_size: f64, max: f64| -> Label {
        layout_label(raw, false, &*measure, font_size, max, 0)
    };
    let line_height = text(Some("0"), small, 4000.0).height;
    let tiny_height = text(Some("0"), tiny, 4000.0).height;

    let title = layout_label(
        if on(chart, "showTitle", true) {
            model.title.as_deref()
        } else {
            None
        },
        false,
        &*measure,
        size,
        width - 2.0 * pad,
        Style::BOLD,
    );
    let title_space = if title.height > 0.0 {
        title.height + 12.0
    } else {
        0.0
    };

    let mut legend: Vec<(usize, Label)> = Vec::new();
    let mut legend_width: f64 = 0.0;
    if on(chart, "showLegend", true) {
        for (index, plot) in plots.iter().enumerate() {
            if plot.title.is_empty() {
                continue;
            }
            let label = text(Some(&plot.title), small, width * 0.3);
            legend_width = legend_width.max(label.width + SWATCH + 8.0);
            legend.push((index, label));
        }
    }
    let legend_space = if !legend.is_empty() {
        legend_width + LEGEND_GAP
    } else {
        0.0
    };

    // The value range: as written, or taken from the data, where bars stand
    // on zero and the range grows to whole steps.
    let bars = plots.iter().filter(|plot| plot.kind == "bar").count();
    let mut lo = model.y_axis.min;
    let mut hi = model.y_axis.max;
    let exact = model.has_set_y_axis && lo.is_finite() && hi.is_finite() && lo != hi;
    if exact {
        lo = finite(Some(lo)).unwrap_or(lo);
        hi = finite(Some(hi)).unwrap_or(hi);
    } else {
        lo = f64::INFINITY;
        hi = f64::NEG_INFINITY;
        for plot in plots {
            for &(_, raw) in &plot.data {
                let Some(value) = finite(raw) else {
                    continue;
                };
                if value < lo {
                    lo = value;
                }
                if value > hi {
                    hi = value;
                }
            }
        }
        if lo > hi {
            lo = 0.0;
            hi = 0.0;
        }
        if bars > 0 {
            lo = js::min(lo, 0.0);
            hi = js::max(hi, 0.0);
        }
        if lo == hi {
            if lo == 0.0 {
                hi = 1.0;
            } else {
                lo -= lo.abs() / 2.0;
                hi += hi.abs() / 2.0;
            }
        }
    }

    let value_marks = |lo: &mut f64, hi: &mut f64, length: f64, spacing: f64| -> Vec<Mark> {
        let step = tick_step(
            (*hi - *lo).abs(),
            js::max(2.0, js::min(10.0, (length / spacing).floor())),
        );
        if !exact {
            *lo = tick_value(step, (tick_index(step, *lo) + 1e-9).floor());
            *hi = tick_value(step, (tick_index(step, *hi) - 1e-9).ceil());
        }
        let (lo, hi) = (*lo, *hi);
        ticks_between(js::min(lo, hi), js::max(lo, hi), step)
            .into_iter()
            .map(|value| Mark {
                at: (value - lo) / (hi - lo),
                label: text(Some(&js::num_str(value)), small, 4000.0),
            })
            .collect()
    };

    // Places along the category axis are fractions of its length. Categories
    // and bars sit in the middle of equal slots; the points of a line on a
    // numeric axis run from end to end.
    let mut count = 1;
    match x {
        XAxis::Band { categories, .. } => count = categories.len().max(1),
        XAxis::Linear { .. } => {
            for plot in plots {
                count = count.max(plot.data.len());
            }
        }
    }
    let band = matches!(x, XAxis::Band { .. });
    let slot = 1.0 / count as f64;
    let inset = if band || bars > 0 { slot / 2.0 } else { 0.0 };
    let (x0, x1) = match x {
        XAxis::Linear { min, max, .. } => (finite(Some(*min)), finite(Some(*max))),
        XAxis::Band { .. } => (None, None),
    };
    let ranged = matches!((x0, x1), (Some(a), Some(b)) if a != b);
    let place = |i: usize, n: usize| -> f64 {
        if band {
            (i as f64 + 0.5) * slot
        } else {
            inset
                + (if n > 1 {
                    i as f64 / (n - 1) as f64
                } else if ranged {
                    0.0
                } else {
                    0.5
                }) * (1.0 - 2.0 * inset)
        }
    };

    let category_conf = chart.section("xAxis").unwrap_or(&empty);
    let value_conf = chart.section("yAxis").unwrap_or(&empty);
    let left_conf = if horizontal {
        category_conf
    } else {
        value_conf
    };
    let bottom_conf = if horizontal {
        value_conf
    } else {
        category_conf
    };
    let left_title = text(
        if on(left_conf, "showTitle", true) {
            Some(if horizontal {
                x.title()
            } else {
                model.y_axis.title.as_str()
            })
        } else {
            None
        },
        small,
        height * 0.8,
    );
    let bottom_title = text(
        if on(bottom_conf, "showTitle", true) {
            Some(if horizontal {
                model.y_axis.title.as_str()
            } else {
                x.title()
            })
        } else {
            None
        },
        small,
        width * 0.8,
    );
    let left_tick = if on(left_conf, "showTick", true) {
        TICK
    } else {
        0.0
    };
    let bottom_tick = if on(bottom_conf, "showTick", true) {
        TICK
    } else {
        0.0
    };
    let left_labels = on(left_conf, "showLabel", true);
    let bottom_labels = on(bottom_conf, "showLabel", true);

    let wrap_at = if horizontal {
        width * 0.3
    } else {
        48f64.max((width - 2.0 * pad - 64.0 - legend_space) * slot - 8.0)
    };
    let categories: Vec<Label> = match x {
        XAxis::Band { categories, .. } if on(category_conf, "showLabel", true) => categories
            .iter()
            .map(|category| text(Some(category), small, wrap_at))
            .collect(),
        _ => Vec::new(),
    };
    let mut category_width: f64 = 0.0;
    let mut category_height: f64 = 0.0;
    for label in &categories {
        category_width = category_width.max(label.width);
        category_height = category_height.max(label.height);
    }

    // The labelled places of the category axis. Labels that would run into
    // each other are thinned out.
    let category_marks = |length: f64| -> Vec<Mark> {
        let mut marks = Vec::new();
        match x {
            XAxis::Band {
                categories: names, ..
            } => {
                let need = if categories.is_empty() {
                    8.0
                } else if horizontal {
                    category_height
                } else {
                    category_width + 12.0
                };
                let every = js::max(1.0, (need / (length * slot)).ceil());
                let mut i = 0.0;
                while i < names.len() as f64 {
                    marks.push(Mark {
                        at: (i + 0.5) * slot,
                        label: categories
                            .get(i as usize)
                            .cloned()
                            .unwrap_or_else(|| text(None, small, 0.0)),
                    });
                    i += every;
                }
            }
            XAxis::Linear { .. } => {
                if let (Some(x0), Some(x1)) = (x0, x1)
                    && ranged
                {
                    let step = tick_step(
                        (x1 - x0).abs(),
                        js::max(
                            2.0,
                            js::min(
                                10.0,
                                (length / if horizontal { 32.0 } else { 80.0 }).floor(),
                            ),
                        ),
                    );
                    for value in ticks_between(js::min(x0, x1), js::max(x0, x1), step) {
                        marks.push(Mark {
                            at: inset + ((value - x0) / (x1 - x0)) * (1.0 - 2.0 * inset),
                            label: text(Some(&js::num_str(value)), small, 4000.0),
                        });
                    }
                } else if let Some(x0) = x0 {
                    marks.push(Mark {
                        at: 0.5,
                        label: text(Some(&js::num_str(x0)), small, 4000.0),
                    });
                }
            }
        }
        marks
    };

    // Room past the end of the value axis for what is written beside a point
    // or a bar.
    let mut point_label_width: f64 = 0.0;
    let mut value_width: f64 = 0.0;
    for plot in plots {
        for label in plot.point_labels.iter().flatten() {
            point_label_width = point_label_width.max(measure.width(label, tiny, 0));
        }
        if show_values && horizontal && plot.kind == "bar" {
            for &(_, value) in &plot.data {
                let written = finite(value).map_or(String::new(), js::num_str);
                value_width = value_width.max(measure.width(&written, tiny, 0));
            }
        }
    }

    let bottom_label_height = if !bottom_labels {
        0.0
    } else if horizontal || !band {
        line_height
    } else {
        category_height
    };
    let bottom_space = bottom_tick
        + if bottom_label_height > 0.0 {
            bottom_label_height + 4.0
        } else {
            0.0
        }
        + if bottom_title.height > 0.0 {
            bottom_title.height + 4.0
        } else {
            0.0
        };
    let headroom = if horizontal {
        0.0
    } else if point_label_width > 0.0 || show_values {
        tiny_height + 4.0
    } else if left_labels {
        line_height / 2.0
    } else {
        0.0
    };
    let top = pad + title_space + headroom;
    let plot_height = 1f64.max(height - pad - bottom_space - top);
    let bottom = top + plot_height;

    let left_marks = if horizontal {
        category_marks(plot_height)
    } else {
        value_marks(&mut lo, &mut hi, plot_height, 56.0)
    };
    let left_label_width = if left_labels {
        widest(&left_marks)
    } else {
        0.0
    };
    let left =
        pad + if left_title.height > 0.0 {
            left_title.height + 6.0
        } else {
            0.0
        } + if left_label_width > 0.0 {
            left_label_width + 6.0
        } else {
            0.0
        } + left_tick;
    let beside = js::max(
        if point_label_width > 0.0 {
            point_label_width + 12.0
        } else {
            0.0
        },
        if value_width > 0.0 {
            value_width + 8.0
        } else {
            0.0
        },
    );
    let reach = if horizontal {
        js::min(width * 0.25, beside)
    } else {
        0.0
    };
    let mut plot_width = 1f64.max(width - pad - left - legend_space - reach);
    let bottom_marks = if horizontal {
        value_marks(&mut lo, &mut hi, plot_width, 80.0)
    } else {
        category_marks(plot_width)
    };
    if bottom_labels && legend_space + reach == 0.0 {
        let mut over: f64 = 0.0;
        for mark in &bottom_marks {
            over = js::max(
                over,
                mark.at * plot_width + mark.label.width / 2.0 - plot_width,
            );
        }
        plot_width = js::max(1.0, plot_width - over);
    }
    let right = left + plot_width;

    let px = |c: f64, v: f64| -> f64 {
        if horizontal {
            left + v * plot_width
        } else {
            left + c * plot_width
        }
    };
    let py = |c: f64, v: f64| -> f64 {
        if horizontal {
            top + c * plot_height
        } else {
            bottom - v * plot_height
        }
    };
    let scale = |value: f64| -> f64 { (value - lo) / (hi - lo) };
    let clamp = |v: f64| -> f64 { js::min(js::max(v, 0.0), 1.0) };

    // A line across the plot at a place on the value axis.
    let rule = |v: f64| -> String {
        if horizontal {
            format!("M{},{}V{}", num(px(0.0, v)), num(top), num(bottom))
        } else {
            format!("M{},{}H{}", num(left), num(py(0.0, v)), num(right))
        }
    };

    let mut grid = String::new();
    for mark in if horizontal {
        &bottom_marks
    } else {
        &left_marks
    } {
        grid.push_str(&rule(mark.at));
    }

    let mut axis = String::new();
    let mut labels = String::new();
    if on(left_conf, "showAxisLine", true) {
        let _ = write!(axis, "M{},{}V{}", num(left), num(top), num(bottom));
    }
    if on(bottom_conf, "showAxisLine", true) {
        let _ = write!(axis, "M{},{}H{}", num(left), num(bottom), num(right));
    }
    for mark in &left_marks {
        let y = if horizontal {
            top + mark.at * plot_height
        } else {
            bottom - mark.at * plot_height
        };
        if left_tick != 0.0 {
            let _ = write!(
                axis,
                "M{},{}h{}",
                num(left),
                num(y),
                js::num_str(-left_tick)
            );
        }
        if left_labels {
            labels.push_str(&aligned(
                &mark.label,
                left - left_tick - 6.0,
                y,
                Anchor::End,
            ));
        }
    }
    for mark in &bottom_marks {
        let at = left + mark.at * plot_width;
        if bottom_tick != 0.0 {
            let _ = write!(
                axis,
                "M{},{}v{}",
                num(at),
                num(bottom),
                js::num_str(bottom_tick)
            );
        }
        if bottom_labels {
            labels.push_str(&aligned(
                &mark.label,
                at,
                bottom + bottom_tick + 4.0 + mark.label.height / 2.0,
                Anchor::Middle,
            ));
        }
    }

    let span = (if horizontal { plot_height } else { plot_width }) * slot;
    let group = span * 0.7;
    let thickness = group / bars.max(1) as f64;
    let gap = if bars > 1 {
        js::min(2.0, thickness * 0.2)
    } else {
        0.0
    };
    let base = clamp(scale(0.0));
    // Bars that go both ways get a line to stand on.
    if bars > 0 && base > 0.0 && base < 1.0 {
        axis.push_str(&rule(base));
    }
    // Lines are drawn over the bars wherever they were declared, as a bar
    // would hide part of one under it. A series keeps the color of its place
    // in the source.
    let mut series = String::new();
    let mut lines = String::new();
    let mut outside = String::new();
    let mut point_labels = String::new();
    let mut bar = 0;
    let bars_f = bars as f64;
    for (index, plot) in plots.iter().enumerate() {
        let color = series_color(index as i64);
        let id = if plot.title.is_empty() {
            String::new()
        } else {
            format!(" data-id=\"{}\"", esc_text(&plot.title))
        };
        // A series declared before the categories can hold more values than
        // there are categories.
        let n = if band {
            plot.data.len().min(count)
        } else {
            plot.data.len()
        };
        if plot.kind == "bar" {
            let offset = (bar as f64 - (bars_f - 1.0) / 2.0) * thickness - (thickness - gap) / 2.0;
            bar += 1;
            let mut rects = String::new();
            for i in 0..n {
                let Some(value) = finite(plot.data[i].1) else {
                    continue;
                };
                let c = place(i, n);
                let v = clamp(scale(value));
                let from = js::min(base, v);
                let to = js::max(base, v);
                let bx = if horizontal {
                    px(c, from)
                } else {
                    px(c, 0.0) + offset
                };
                let by = if horizontal {
                    py(c, 0.0) + offset
                } else {
                    py(c, to)
                };
                let bw = if horizontal {
                    (to - from) * plot_width
                } else {
                    thickness - gap
                };
                let bh = if horizontal {
                    thickness - gap
                } else {
                    (to - from) * plot_height
                };
                let _ = write!(
                    rects,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    num(bx),
                    num(by),
                    num(bw),
                    num(bh)
                );
                if !show_values {
                    continue;
                }
                let label = js::num_str(value);
                let w = measure.width(&label, tiny, 0);
                // Towards the end of the bar: 1 when it runs up or right from
                // the baseline, -1 the other way.
                let out = if v >= base { 1.0 } else { -1.0 };
                let mut tx = bx + bw / 2.0;
                let mut ty = by + bh / 2.0;
                let mut anchor = "middle";
                if horizontal {
                    if bh < tiny {
                        continue;
                    }
                    tx = (if out > 0.0 { bx + bw } else { bx }) + out * 6.0;
                    anchor = if out > 0.0 { "start" } else { "end" };
                } else {
                    if w + 2.0 > span / bars_f {
                        continue;
                    }
                    ty = (if out > 0.0 { by } else { by + bh }) - out * (tiny_height / 2.0 + 2.0);
                }
                let _ = write!(
                    outside,
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"{anchor}\">{label}</text>",
                    num(tx),
                    num(ty + tiny * 0.35)
                );
            }
            let _ = write!(
                series,
                "<g class=\"{p}-series {p}-bars\"{id} fill=\"{color}\">{rects}</g>"
            );
            continue;
        }

        // A line is cut where it leaves the value range, so it stays inside
        // the plot.
        let mut d = String::new();
        let mut dots = String::new();
        let mut pen = false;
        let mut pc = 0.0;
        let mut pv = f64::NAN;
        for i in 0..n {
            let Some(value) = finite(plot.data[i].1) else {
                pv = f64::NAN;
                continue;
            };
            let c = place(i, n);
            let v = scale(value);
            let visible = (0.0..=1.0).contains(&v);
            if pv.is_nan() {
                if visible {
                    let _ = write!(d, "M{},{}", num(px(c, v)), num(py(c, v)));
                }
                pen = visible;
            } else {
                let dv = v - pv;
                let mut t0 = 0.0;
                let mut t1 = 1.0;
                if dv != 0.0 {
                    let a = -pv / dv;
                    let b = (1.0 - pv) / dv;
                    t0 = js::max(t0, js::min(a, b));
                    t1 = js::min(t1, js::max(a, b));
                } else if !visible {
                    t0 = 2.0;
                }
                if t0 <= t1 {
                    let c0 = pc + (c - pc) * t0;
                    let v0 = pv + dv * t0;
                    let c1 = pc + (c - pc) * t1;
                    let v1 = pv + dv * t1;
                    if !pen || t0 > 0.0 {
                        let _ = write!(d, "M{},{}", num(px(c0, v0)), num(py(c0, v0)));
                    }
                    let _ = write!(d, "L{},{}", num(px(c1, v1)), num(py(c1, v1)));
                }
                pen = visible;
            }
            pc = c;
            pv = v;
            if !visible {
                continue;
            }
            let cx = px(c, v);
            let cy = py(c, v);
            if span >= 12.0 {
                let _ = write!(
                    dots,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"3\"/>",
                    num(cx),
                    num(cy)
                );
            }
            let label = text(
                plot.point_labels
                    .as_ref()
                    .and_then(|labels| labels.get(i))
                    .map(String::as_str),
                tiny,
                4000.0,
            );
            if label.height > 0.0 {
                let half = label.width / 2.0;
                point_labels.push_str(&if horizontal {
                    aligned(&label, cx + 10.0, cy, Anchor::Start)
                } else {
                    aligned(
                        &label,
                        js::max(js::min(cx, right - half), left + half),
                        cy - 6.0 - label.height / 2.0,
                        Anchor::Middle,
                    )
                });
            }
        }
        let _ = write!(
            lines,
            "<g class=\"{p}-series {p}-line\"{id} fill=\"{color}\">"
        );
        if !d.is_empty() {
            let _ = write!(
                lines,
                "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
            );
        }
        lines.push_str(&dots);
        lines.push_str("</g>");
    }

    let mut titles = String::new();
    if left_title.height > 0.0 {
        let _ = write!(
            titles,
            "<g transform=\"translate({},{}) rotate(-90)\">{}</g>",
            num(pad + left_title.height / 2.0),
            num(top + plot_height / 2.0),
            label_svg(&left_title, 0.0, 0.0, "")
        );
    }
    titles.push_str(&label_svg(
        &bottom_title,
        left + plot_width / 2.0,
        height - pad - bottom_title.height / 2.0,
        "",
    ));

    let mut legend_out = String::new();
    let mut legend_height = 0.0;
    for (_, label) in &legend {
        legend_height += ROW.max(label.height + 6.0);
    }
    let mut ly = top + 0f64.max((plot_height - legend_height) / 2.0);
    let lx = right + reach + LEGEND_GAP;
    for (index, label) in &legend {
        let row = ROW.max(label.height + 6.0);
        if ly + row > height {
            break;
        }
        let y = ly + row / 2.0;
        let color = series_color(*index as i64);
        let swatch = if plots[*index].kind == "bar" {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\"/>",
                num(lx),
                num(y - SWATCH / 2.0),
                js::num_str(SWATCH),
                js::num_str(SWATCH)
            )
        } else {
            format!(
                "<path d=\"M{},{}h{}\" stroke=\"{color}\" stroke-width=\"2\" stroke-linecap=\"round\"/>",
                num(lx),
                num(y),
                js::num_str(SWATCH)
            )
        };
        let _ = write!(
            legend_out,
            "<g class=\"{p}-legend-item\" data-id=\"{}\">{swatch}{}</g>",
            esc_text(&plots[*index].title),
            label_svg(label, lx + SWATCH + 8.0 + label.width / 2.0, y, "")
        );
        ly += row;
    }

    let small_str = js::num_str(small);
    let tiny_str = js::num_str(tiny);
    let mut content = label_svg(
        &title,
        width / 2.0,
        pad + title.height / 2.0,
        &format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\""),
    );
    if !grid.is_empty() {
        let _ = write!(
            content,
            "<path class=\"{p}-grid\" d=\"{grid}\" stroke=\"var(--_a)\"/>"
        );
    }
    let _ = write!(content, "<g class=\"{p}-plot\">{series}{lines}</g>");
    if !axis.is_empty() {
        let _ = write!(
            content,
            "<path class=\"{p}-axis\" d=\"{axis}\" fill=\"none\" stroke=\"var(--_b)\"/>"
        );
    }
    if !labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-axis-labels\" font-size=\"{small_str}\" fill=\"var(--_m)\">{labels}</g>"
        );
    }
    if !titles.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-axis-titles\" font-size=\"{small_str}\">{titles}</g>"
        );
    }
    if !outside.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-data-labels\" font-size=\"{tiny_str}\">{outside}</g>"
        );
    }
    if !point_labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-point-labels\" font-size=\"{tiny_str}\">{point_labels}</g>"
        );
    }
    if !legend_out.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-legend\" font-size=\"{small_str}\">{legend_out}</g>"
        );
    }

    Ok(Rendered {
        svg: svg_document(
            "xychart",
            width,
            height,
            size,
            options,
            Accessible {
                acc_title: model.acc_title.as_deref(),
                acc_descr: model.acc_descr.as_deref(),
            },
            &content,
        ),
        width,
        height,
        links: Vec::new(),
    })
}
