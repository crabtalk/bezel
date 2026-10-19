//! Pele's `diagrams/gantt/render.ts`.

use indexmap::IndexMap;
use std::fmt::Write as _;

use super::{
    axis::{auto_ticks, interval_ticks, time_format},
    dates::{Unit, add, add_days},
    db::{GanttDb, clock},
    types::GanttTask,
};
use crate::mermaid::{
    Config, Error, Link, Options, Rendered,
    config::Value,
    diagrams::common::fit_width::cap_width,
    js::{self, Lazy, date::Date},
    svg::{
        builder::{esc, esc_text, label_svg, num},
        root::{Accessible, svg_document},
        theme::{CRITICAL, RADIUS, resolve_style},
    },
    text::{Label, Measure, Style, layout_label},
    util::url::linked,
};

const WIDTH: f64 = 800.0;
const MIN_PLOT: f64 = 240.0;
const LABEL_GAP: f64 = 8.0;

static RE_BREAK: Lazy = Lazy::new(r"<br\s*\/?>", "gi");

fn gantt_value<'a>(config: &'a Config, key: &str) -> Option<&'a Value> {
    config.section("gantt").and_then(|gantt| gantt.get(key))
}

fn setting(config: &Config, key: &str, min: f64, max: f64) -> Option<f64> {
    match gantt_value(config, key) {
        Some(Value::Number(value)) if value.is_finite() => Some(max.min(min.max(*value))),
        _ => None,
    }
}

fn text<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    match gantt_value(config, key) {
        Some(Value::String(value)) => Some(value),
        _ => None,
    }
}

fn flag(config: &Config, key: &str) -> bool {
    matches!(gantt_value(config, key), Some(Value::Bool(true)))
}

/// A task label sits on one row, so only its first line is kept.
fn one_line(raw: &str, measure: &dyn Measure, size: f64) -> Label {
    let label = layout_label(
        Some(&RE_BREAK.replace(raw, |_| " ".into())),
        false,
        measure,
        size,
        4000.0,
        0,
    );
    if label.lines.len() <= 1 {
        return label;
    }
    Label {
        lines: vec![label.lines[0].clone()],
        widths: vec![label.widths[0]],
        width: label.widths[0],
        height: label.line_height,
        ..label
    }
}

fn time(date: Option<f64>) -> f64 {
    date.unwrap_or(f64::NAN)
}

/// Whether scheduling gave the task a start and an end. One that waits on a
/// task that never resolves has neither.
fn scheduled(task: &GanttTask) -> bool {
    time(task.start_time).is_finite() && time(task.end_time).is_finite()
}

/// Mermaid's compact mode: within a section, a task goes on the first row
/// that is free when it starts. `group` holds indices into `tasks`.
fn pack_rows(
    tasks: &[GanttTask],
    group: &[usize],
    offset: usize,
    row_of: &mut IndexMap<usize, usize>,
) -> usize {
    let mut sorted: Vec<usize> = group
        .iter()
        .copied()
        .filter(|&i| scheduled(&tasks[i]))
        .collect();
    sorted.sort_by(|&a, &b| {
        let (a, b) = (&tasks[a], &tasks[b]);
        let d = time(a.start_time) - time(b.start_time);
        let d = if js::truthy(d) {
            d
        } else {
            (a.order - b.order) as f64
        };
        d.partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut free: Vec<f64> = Vec::new();
    for &index in &sorted {
        let task = &tasks[index];
        let start = time(task.start_time);
        let mut row = 0;
        while row < free.len() && start < free[row] {
            row += 1;
        }
        if row == free.len() {
            free.push(time(task.end_time));
        } else {
            free[row] = time(task.end_time);
        }
        row_of.insert(index, offset + row);
    }
    let mut rows = free.len();
    for &index in group {
        if !row_of.contains_key(&index) {
            row_of.insert(index, offset + rows);
            rows += 1;
        }
    }
    rows.max(1)
}

struct Bar {
    task: usize,
    label: Label,
    row: usize,
    x0: f64,
    x1: f64,
    label_x: f64,
    inside: bool,
}

fn accessible(model: &GanttDb) -> Accessible<'_> {
    Accessible {
        acc_title: model.acc_title.as_deref(),
        acc_descr: model.acc_descr.as_deref(),
    }
}

pub fn render_gantt(
    model: &GanttDb,
    config: &Config,
    options: &Options,
) -> Result<Rendered, Error> {
    let size = options.font_size.unwrap_or(16.0);
    let measure = options.measurer();
    let measure = &*measure;
    let pad = options.padding.unwrap_or(8.0);
    let now = clock(options.now);
    let p = crate::mermaid::PREFIX;

    let label_size =
        setting(config, "fontSize", 4.0, 72.0).unwrap_or_else(|| js::round(size * 0.8125));
    let section_size = setting(config, "sectionFontSize", 4.0, 72.0).unwrap_or(label_size);
    let tick_size = js::round(size * 0.75);
    let bar_height =
        setting(config, "barHeight", 4.0, 400.0).unwrap_or_else(|| js::round(label_size * 1.85));
    let bar_gap =
        setting(config, "barGap", 0.0, 400.0).unwrap_or_else(|| js::round(bar_height / 4.0));
    let row_height = bar_height + bar_gap;
    let section_styles =
        js::round(setting(config, "numberSectionStyles", 1.0, 1000.0).unwrap_or(4.0));
    let compact = model.display_mode == "compact";
    let top_axis = model.top_axis || flag(config, "topAxis");

    let mut db = model.clone();
    db.get_tasks(Some(now))?;
    let tasks = db.tasks.clone();
    let row_tasks: Vec<usize> = (0..tasks.len()).filter(|&i| !tasks[i].vert).collect();
    let verts: Vec<usize> = (0..tasks.len())
        .filter(|&i| tasks[i].vert && scheduled(&tasks[i]))
        .collect();

    let mut categories: IndexMap<&str, usize> = IndexMap::new();
    for &i in &row_tasks {
        let next = categories.len();
        categories.entry(tasks[i].section.as_str()).or_insert(next);
    }

    let mut row_of: IndexMap<usize, usize> = IndexMap::new();
    let mut row_section: Vec<&str> = Vec::new();
    if compact {
        let mut groups: IndexMap<&str, Vec<usize>> = IndexMap::new();
        for &i in &row_tasks {
            groups.entry(tasks[i].section.as_str()).or_default().push(i);
        }
        for (section, group) in &groups {
            let rows = pack_rows(&tasks, group, row_section.len(), &mut row_of);
            for _ in 0..rows {
                row_section.push(section);
            }
        }
    } else {
        for &i in &row_tasks {
            row_of.insert(i, row_section.len());
            row_section.push(&tasks[i].section);
        }
    }
    let row_count = row_section.len();

    let mut start = f64::INFINITY;
    let mut stop = f64::NEG_INFINITY;
    for task in &tasks {
        if !scheduled(task) {
            continue;
        }
        start = start.min(time(task.start_time));
        stop = stop.max(time(task.end_time));
    }
    let has_plot = start <= stop;

    let title = layout_label(
        model.title.as_deref(),
        false,
        measure,
        size,
        4000.0,
        Style::BOLD,
    );
    let title_height = if title.height > 0.0 {
        title.height + 16.0
    } else {
        0.0
    };
    let title_attrs = format!(" class=\"{p}-title\" font-weight=\"var(--_tw)\"");

    if !has_plot {
        let width = (title.width.max(1.0) + 2.0 * pad).ceil();
        let height = (title.height.max(1.0) + 2.0 * pad).ceil();
        let content = label_svg(&title, width / 2.0, pad + title.height / 2.0, &title_attrs);
        return Ok(Rendered {
            svg: svg_document(
                "gantt",
                width,
                height,
                size,
                options,
                accessible(model),
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        });
    }

    let mut section_labels: IndexMap<&str, Label> = IndexMap::new();
    let mut section_width: f64 = 0.0;
    for &section in categories.keys() {
        let label = layout_label(Some(section), false, measure, section_size, 180.0, 0);
        section_width = section_width.max(label.width);
        section_labels.insert(section, label);
    }

    let axis_format = if !model.axis_format.is_empty() {
        model.axis_format.clone()
    } else if model.date_format == "D" {
        "%d".to_string()
    } else {
        text(config, "axisFormat").unwrap_or("%Y-%m-%d").to_string()
    };
    let tick_label = |t: f64| time_format(&axis_format, Date::new(t));
    let sample_width = measure
        .width(&tick_label(start), tick_size, 0)
        .max(measure.width(&tick_label(stop), tick_size, 0));

    let left = pad
        + setting(config, "leftPadding", 0.0, 2000.0).unwrap_or_else(|| {
            (if section_width > 0.0 {
                section_width + 24.0
            } else {
                0.0
            })
            .max(sample_width / 2.0)
        });
    let right_padding = setting(config, "rightPadding", 0.0, 2000.0)
        .unwrap_or_else(|| 16f64.max(sample_width / 2.0));
    let target = cap_width(
        options,
        setting(config, "useWidth", 100.0, 20000.0).unwrap_or(WIDTH),
        280.0,
        0.0,
    );
    let plot_width = MIN_PLOT.max(target - left - right_padding - pad);
    let right = left + plot_width;
    let scale = if stop > start {
        plot_width / (stop - start)
    } else {
        0.0
    };
    let x = |t: f64| -> f64 {
        if scale == 0.0 {
            left + plot_width / 2.0
        } else {
            left + (t - start) * scale
        }
    };

    let tick_height = js::round(tick_size * 1.75);
    let top_padding = setting(config, "topPadding", 0.0, 2000.0);
    let grid_start = setting(config, "gridLineStartPadding", 0.0, 2000.0);
    let head_height = pad + title_height + if top_axis { tick_height } else { 0.0 };
    let plot_top = head_height.max(top_padding.unwrap_or(0.0));
    // Mermaid starts its grid lines above the first row by the difference
    // between these two settings. Here that only uses room a larger
    // `topPadding` leaves free.
    let grid_lead = match (top_padding, grid_start) {
        (Some(top_padding), Some(grid_start)) => {
            0f64.max((top_padding - grid_start).min(plot_top - head_height))
        }
        _ => 0.0,
    };
    let plot_height = (row_count.max(1)) as f64 * row_height;
    let plot_bottom = plot_top + plot_height;

    // Bars and where their labels go: inside when they fit, otherwise beside
    // the bar on the side with room.
    let mut bars: Vec<Bar> = Vec::new();
    let mut extent = right + right_padding;
    for &index in &row_tasks {
        let task = &tasks[index];
        let label = one_line(&task.task, measure, label_size);
        let row = row_of[&index];
        if !scheduled(task) {
            let label_x = left + LABEL_GAP + label.width / 2.0;
            extent = extent.max(label_x + label.width / 2.0);
            bars.push(Bar {
                task: index,
                label,
                row,
                x0: f64::NAN,
                x1: f64::NAN,
                label_x,
                inside: false,
            });
            continue;
        }
        let mut x0 = x(time(task.start_time));
        let mut x1 = x(time(task.render_end_time.or(task.end_time)))
            .max(x0 + if task.milestone { 0.0 } else { 2.0 });
        if task.milestone {
            let mid = x0 + (x(time(task.end_time)) - x0) / 2.0;
            x0 = mid - bar_height / 2.0;
            x1 = mid + bar_height / 2.0;
        }
        let inside = !task.milestone && label.width + 2.0 * LABEL_GAP <= x1 - x0;
        let mut label_x = (x0 + x1) / 2.0;
        if !inside {
            let fits_right = x1 + LABEL_GAP + label.width <= right + right_padding;
            let fits_left = x0 - LABEL_GAP - label.width >= left;
            label_x = if fits_right || !fits_left {
                x1 + LABEL_GAP + label.width / 2.0
            } else {
                x0 - LABEL_GAP - label.width / 2.0
            };
            extent = extent.max(label_x + label.width / 2.0);
        }
        bars.push(Bar {
            task: index,
            label,
            row,
            x0,
            x1,
            label_x,
            inside,
        });
    }

    let interval = model
        .tick_interval
        .as_deref()
        .filter(|interval| !interval.is_empty())
        .or_else(|| text(config, "tickInterval").filter(|interval| !interval.is_empty()))
        .unwrap_or("");
    let weekday = model
        .weekday
        .as_deref()
        .or_else(|| text(config, "weekday"))
        .unwrap_or("sunday");
    let count = 2f64.max(10f64.min((plot_width / (sample_width + 24.0)).floor()));
    let ticks = interval_ticks(interval, weekday, start, stop, (plot_width / 3.0).ceil())
        .unwrap_or_else(|| auto_ticks(start, stop, count));

    let mut grid = String::new();
    let mut axis_bottom = String::new();
    let mut axis_top = String::new();
    let mut last_right = f64::NEG_INFINITY;
    let mut last_label = String::new();
    let bottom_y = plot_bottom + tick_height / 2.0 + 2.0;
    let top_y = plot_top - grid_lead - tick_height / 2.0 - 2.0;
    for &t in &ticks {
        let tx = x(t);
        let _ = write!(
            grid,
            "M{},{}V{}",
            num(tx),
            num(plot_top - grid_lead),
            num(plot_bottom)
        );
        let label = tick_label(t);
        let width = measure.width(&label, tick_size, 0);
        // A label is skipped when it would run into the one before it or only
        // repeat it; its grid line stays.
        if tx - width / 2.0 < last_right + 8.0 || label == last_label {
            continue;
        }
        last_right = tx + width / 2.0;
        extent = extent.max(last_right);
        let body = format!("text-anchor=\"middle\">{}</text>", esc(&label));
        last_label = label;
        let _ = write!(
            axis_bottom,
            "<text x=\"{}\" y=\"{}\" {body}",
            num(tx),
            num(bottom_y + tick_size * 0.35)
        );
        if top_axis {
            let _ = write!(
                axis_top,
                "<text x=\"{}\" y=\"{}\" {body}",
                num(tx),
                num(top_y + tick_size * 0.35)
            );
        }
    }

    // Vertical markers take no row. Their labels go under the axis, on as
    // many lines as it takes to keep them apart.
    let mut vert_lines = String::new();
    let mut vert_labels = String::new();
    let mut vert_ends: Vec<f64> = Vec::new();
    let vert_top = plot_bottom + tick_height + 4.0;
    let vert_line = js::round(label_size * 1.5);
    let mut sorted_verts = verts.clone();
    sorted_verts.sort_by(|&a, &b| {
        (time(tasks[a].start_time) - time(tasks[b].start_time))
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for &index in &sorted_verts {
        let task = &tasks[index];
        let vx = x(time(task.start_time));
        let label = one_line(&task.task, measure, label_size);
        let cx = (pad + label.width / 2.0).max(vx);
        let mut line = 0;
        while line < vert_ends.len() && cx - label.width / 2.0 < vert_ends[line] + 8.0 {
            line += 1;
        }
        if line == vert_ends.len() {
            vert_ends.push(cx + label.width / 2.0);
        } else {
            vert_ends[line] = cx + label.width / 2.0;
        }
        extent = extent.max(vert_ends[line]);
        let _ = write!(
            vert_lines,
            "<path class=\"{p}-vert\" data-id=\"{}\" d=\"M{},{}V{}\"/>",
            esc_text(&task.id),
            num(vx),
            num(plot_top - grid_lead),
            num(plot_bottom)
        );
        vert_labels += &label_svg(
            &label,
            cx,
            vert_top + line as f64 * vert_line + vert_line / 2.0,
            &format!(
                " class=\"{p}-vert-label\" data-id=\"{}\"",
                esc_text(&task.id)
            ),
        );
    }

    let width = (extent.max(pad + title.width) + pad).ceil();
    let height = (plot_bottom
        + tick_height
        + if !vert_ends.is_empty() {
            4.0 + vert_ends.len() as f64 * vert_line
        } else {
            0.0
        }
        + pad)
        .ceil();

    // Section bands, one per run of rows that share a section, shaded on
    // alternate sections.
    let mut bands = String::new();
    let mut section_out = String::new();
    let mut row = 0;
    while row < row_count {
        let section = row_section[row];
        let mut end = row + 1;
        while end < row_count && row_section[end] == section {
            end += 1;
        }
        let index = (categories[section] as f64) % section_styles;
        let y = plot_top + row as f64 * row_height;
        let h = (end - row) as f64 * row_height;
        if categories.len() > 1 || !section.is_empty() {
            let _ = write!(
                bands,
                "<rect class=\"{p}-section {p}-section-{}\" x=\"0\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                js::num_str(index),
                num(y),
                js::num_str(width),
                num(h),
                if index % 2.0 == 0.0 {
                    "var(--_s)"
                } else {
                    "none"
                }
            );
        }
        let label = &section_labels[section];
        section_out += &label_svg(
            label,
            pad + 8.0 + label.width / 2.0,
            y + h / 2.0,
            &format!(
                " class=\"{p}-section-label\" data-id=\"{}\"",
                esc_text(section)
            ),
        );
        row = end;
    }

    // Excluded days, as Mermaid marks them: a day at a time from the first
    // task's start, and not at all when the chart spans more than five
    // years.
    let mut excluded = String::new();
    if (!model.excludes.is_empty() || !model.includes.is_empty())
        && add(Date::new(start), 6.0, Unit::Y).time() > stop
    {
        let mut from: Option<Date> = None;
        let mut to: Option<Date> = None;
        let mut flush = |from: &mut Option<Date>, to: &mut Option<Date>| {
            if let (Some(f), Some(t)) = (*from, *to) {
                let mut day_start = f;
                day_start.set_hours(0.0, 0.0, 0.0, 0.0);
                let mut day_end = t;
                day_end.set_hours(23.0, 59.0, 59.0, 999.0);
                let ex0 = left.max(x(day_start.time()));
                let ex1 = right.min(x(day_end.time()));
                if ex1 > ex0 {
                    let _ = write!(
                        excluded,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                        num(ex0),
                        num(plot_top),
                        num(ex1 - ex0),
                        num(plot_height)
                    );
                }
            }
            *from = None;
            *to = None;
        };
        let mut day = Date::new(start);
        while day.time() <= stop {
            if db.is_invalid_date(day.time())? {
                if from.is_none() {
                    from = Some(day);
                }
                to = Some(day);
            } else {
                flush(&mut from, &mut to);
            }
            day = add_days(day, 1.0);
        }
        flush(&mut from, &mut to);
    }

    let mut links: Vec<Link> = Vec::new();
    let mut tasks_out = String::new();
    for bar in &bars {
        let task = &tasks[bar.task];
        let cy = plot_top + bar.row as f64 * row_height + row_height / 2.0;
        let state = if task.active {
            "active"
        } else if task.done {
            "done"
        } else {
            ""
        };
        let mut classes = format!("{p}-task");
        if !state.is_empty() {
            let _ = write!(classes, " {p}-{state}");
        }
        if task.crit {
            let _ = write!(classes, " {p}-crit");
        }
        if task.milestone {
            let _ = write!(classes, " {p}-milestone");
        }
        if task.classes.iter().any(|class| class == "clickable") {
            let _ = write!(classes, " {p}-clickable");
        }

        let mut shape = String::new();
        let mut text_fill = "";
        if bar.x0.is_nan() {
            let _ = write!(classes, " {p}-unscheduled");
            text_fill = " fill=\"var(--_m)\"";
        } else {
            // A critical task takes the critical color in place of the
            // accent. Done, it keeps the fill of finished work and only its
            // border is in that color.
            let color = if task.crit { CRITICAL } else { "var(--_c)" };
            let paint = match state {
                "done" => "fill=\"var(--_a)\"".to_string(),
                "active" => format!("fill=\"{color}\" fill-opacity=\"0.25\""),
                _ => format!("fill=\"{color}\""),
            };
            let stroke = format!(
                "stroke=\"{}\"",
                if state == "done" && !task.crit {
                    "var(--_b)"
                } else {
                    color
                }
            );
            if task.milestone {
                let cx = (bar.x0 + bar.x1) / 2.0;
                let r = bar_height / 2.0;
                shape = format!(
                    "<path d=\"M{},{}L{},{}L{},{}L{},{}Z\" {paint} {stroke} stroke-linejoin=\"round\"/>",
                    num(cx),
                    num(cy - r),
                    num(cx + r),
                    num(cy),
                    num(cx),
                    num(cy + r),
                    num(cx - r),
                    num(cy)
                );
            } else {
                shape = format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" {paint} {stroke}/>",
                    num(bar.x0),
                    num(cy - bar_height / 2.0),
                    num(bar.x1 - bar.x0),
                    num(bar_height)
                );
            }
            if bar.inside {
                text_fill = if state.is_empty() {
                    " fill=\"var(--_bg)\""
                } else {
                    ""
                };
            } else if state == "done" {
                text_fill = " fill=\"var(--_m)\"";
            }
        }

        let mut body = shape
            + &label_svg(
                &bar.label,
                bar.label_x,
                cy,
                &format!(" class=\"{p}-label\"{text_fill}"),
            );
        if let Some(url) = model.links.get(&task.id) {
            body = linked(&body, url, &task.id, &mut links, options, "");
        }
        let _ = write!(
            tasks_out,
            "<g class=\"{classes}\" data-id=\"{}\">{body}</g>",
            esc_text(&task.id)
        );
    }

    let mut today = String::new();
    if model.today_marker != "off" && now >= start && now <= stop && stop > start {
        let tx = x(now);
        let top = plot_top - grid_lead;
        let style = if model.today_marker.is_empty() {
            String::new()
        } else {
            let declarations: Vec<&str> = model.today_marker.split(',').collect();
            resolve_style(&declarations).line
        };
        today = format!(
            "<g class=\"{p}-today\"><path d=\"M{},{}V{}\" fill=\"none\" stroke=\"var(--_c)\" stroke-width=\"2\"{style}/>",
            num(tx),
            num(top),
            num(plot_bottom)
        );
        if style.is_empty() {
            let _ = write!(
                today,
                "<path d=\"M{},{}H{}L{},{}Z\" fill=\"var(--_c)\"/>",
                num(tx - 4.0),
                num(top - 5.0),
                num(tx + 4.0),
                num(tx),
                num(top + 1.0)
            );
        }
        today += "</g>";
    }

    let mut axis_line = format!("M{},{}H{}", num(left), num(plot_bottom), num(right));
    if top_axis {
        let _ = write!(
            axis_line,
            "M{},{}H{}",
            num(left),
            num(plot_top - grid_lead),
            num(right)
        );
    }
    let mut content = label_svg(&title, width / 2.0, pad + title.height / 2.0, &title_attrs);
    if !bands.is_empty() {
        let _ = write!(content, "<g class=\"{p}-sections\">{bands}</g>");
    }
    if !excluded.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-excluded\" fill=\"var(--_a)\">{excluded}</g>"
        );
    }
    let _ = write!(
        content,
        "<g class=\"{p}-grid\" fill=\"none\"><path d=\"{grid}\" stroke=\"var(--_b)\" stroke-dasharray=\"1 3\"/><path d=\"{axis_line}\" stroke=\"var(--_b)\"/></g>"
    );
    if !vert_lines.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-verts\" fill=\"none\" stroke=\"var(--_l)\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\">{vert_lines}</g>"
        );
    }
    content += &today;
    let _ = write!(
        content,
        "<g class=\"{p}-tasks\" font-size=\"{}\">{tasks_out}</g>",
        num(label_size)
    );
    let _ = write!(
        content,
        "<g class=\"{p}-section-labels\" font-size=\"{}\">{section_out}</g>",
        num(section_size)
    );
    let _ = write!(
        content,
        "<g class=\"{p}-axis\" font-size=\"{}\" fill=\"var(--_m)\">{axis_bottom}{axis_top}</g>",
        js::num_str(tick_size)
    );
    if !vert_labels.is_empty() {
        let _ = write!(
            content,
            "<g class=\"{p}-vert-labels\" font-size=\"{}\">{vert_labels}</g>",
            num(label_size)
        );
    }

    Ok(Rendered {
        svg: svg_document(
            "gantt",
            width,
            height,
            size,
            options,
            accessible(model),
            &content,
        ),
        width,
        height,
        links,
    })
}
