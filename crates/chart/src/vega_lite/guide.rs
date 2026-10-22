//! An `axis`, a `legend` and the top-level `title`, by Vega-Lite's
//! properties. `aria`, `description` and `style` are dropped; an `encode`
//! block, a property this does not read, or a value it does not take, such
//! as an expression, answers `None`. `labelExpr`, and the `test` of an axis
//! property's `condition`, are Vega expressions of `datum.value`,
//! `datum.label` and `datum.index`.

use crate::{
    data::{Column, Data},
    model::{
        Align, Anchor, Axis, Baseline, Cap, Case, Datum, Format, Frame, GridAlign, Kind, Legend,
        LegendKind, LegendOrient, Lettering, Line, Orient, Overlap, Relabel, Side, Test, TickBand,
        TickCount, TickStyle, Title,
    },
    time,
    transform::Part,
    vega_lite::{datetime, encoding},
};
use d3::{format::Specifier, time::Unit};
use gpui::SharedString;
use serde_json::{Map, Value};
use std::sync::Arc;
use vega::{expr::Expr, value::Value as ExprValue};

/// What no guide draws differently for.
const DROPPED: [&str; 3] = ["aria", "description", "style"];
/// A legend's properties Vega draws nothing differently for.
const LEGEND_DROPPED: [&str; 1] = ["labelPadding"];
/// Vega-Lite's specifiers for a time unit's parts, and runs of them, as
/// vega-time's `timeUnitSpecifier` joins them.
const UNIT_FORMATS: [(&str, &str); 14] = [
    ("year", "%Y "),
    ("quarter", "Q%q "),
    ("month", "%b "),
    ("date", "%d "),
    ("week", "W%U "),
    ("day", "%a "),
    ("dayofyear", "%j "),
    ("hours", "%H:00"),
    ("minutes", "00:%M"),
    ("seconds", ":%S"),
    ("milliseconds", ".%L"),
    ("year-month", "%b %Y "),
    ("year-month-date", "%b %d, %Y "),
    ("hours-minutes", "%H:%M"),
];

/// How a field's values read on its guides, as Vega-Lite derives it from
/// the guide's `format` and `formatType`, the field's `kind` and its time
/// unit's `parts`. `None` for a format this does not read.
fn format(
    guide: &Map<String, Value>,
    kind: Kind,
    parts: Option<&[Part]>,
) -> Option<Option<Format>> {
    let written = match guide.get("format") {
        None => None,
        Some(Value::String(format)) => Some(format.as_str()),
        Some(_) => return None,
    };
    let time = match guide.get("formatType") {
        None => kind == Kind::Temporal || parts.is_some(),
        Some(Value::String(kind)) => match kind.as_str() {
            "number" => false,
            "time" | "utc" => true,
            _ => return None,
        },
        Some(_) => return None,
    };
    Some(match (written, time) {
        (Some(format), true) => Some(Format::Time(format.to_string().into())),
        (Some(format), false) => Some(Format::Number(format.parse::<Specifier>().ok()?)),
        (None, _) => parts.map(|parts| Format::Time(unit_format(parts).into())),
    })
}

/// vega-time's `timeUnitSpecifier` with Vega-Lite's specifiers: each run of
/// `parts` written as the longest run it names.
pub(crate) fn unit_format(parts: &[Part]) -> String {
    let names: Vec<&str> = parts.iter().map(|part| part.name()).collect();
    let mut out = String::new();
    let mut start = 0;
    while start < names.len() {
        let found = (start + 1..=names.len()).rev().find_map(|end| {
            let key = names[start..end].join("-");
            let (_, format) = UNIT_FORMATS.iter().find(|(name, _)| *name == key)?;
            Some((end, *format))
        });
        match found {
            Some((end, format)) => {
                out.push_str(format);
                start = end;
            }
            None => start += 1,
        }
    }
    out.trim().to_string()
}

/// A guide's tick as an expression's `datum`: its `value`, `label` and
/// `index`.
fn tick(value: &Datum, label: &str, index: f64) -> Data {
    let value = match value {
        Datum::Number(number) => Column::Number(vec![*number].into()),
        Datum::Text(text) => Column::Text(std::iter::once(Some(text.clone())).collect()),
    };
    Data::new()
        .with("value", value)
        .with(
            "label",
            Column::Text(std::iter::once(Some(label.to_string())).collect()),
        )
        .with("index", Column::Number(vec![index].into()))
}

/// A `labelExpr` as a guide's relabelling. `None` for one that does not
/// parse.
fn relabel(source: &Value) -> Option<Relabel> {
    let expr: Expr = source.as_str()?.parse().ok()?;
    Some(Relabel(Arc::new(move |value, label, index| {
        let data = tick(value, label, index);
        match data.eval(&expr, 0) {
            Ok(ExprValue::Array(lines)) => lines
                .iter()
                .map(ExprValue::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
            Ok(ExprValue::Null | ExprValue::Undefined) | Err(_) => SharedString::default(),
            Ok(text) => text.to_string().into(),
        }
    })))
}

/// A condition's `test` as a tick's: an expression of `datum.value` and
/// `datum.label`, which holds where it is truthy.
fn test(source: &Value) -> Option<Test> {
    let expr: Expr = source.as_str()?.parse().ok()?;
    Some(Test(Arc::new(move |value, label| {
        let data = tick(value, label, 0.0);
        data.eval(&expr, 0).is_ok_and(|value| value.truthy())
    })))
}

/// Reads `key` into `style` where it is a property of each tick, as
/// [`lettering`] does.
fn tick_style(style: &mut TickStyle, key: &str, value: &Value) -> Option<bool> {
    if lettering(&mut style.label, "label", key, value)?
        || line(&mut style.grid, "grid", "grid", key, value)?
        || line(&mut style.tick, "ticks", "tick", key, value)?
    {
        return Some(true);
    }
    match key {
        "tickSize" => style.size = Some(number(value)?),
        "labelOffset" => style.label_offset = Some(number(value)?),
        "labelPadding" => style.label_padding = Some(number(value)?),
        _ => return Some(false),
    }
    Some(true)
}

/// A tick property's `{condition, value}`: each condition's case, then the
/// value where none holds. `None` for one this does not read.
fn conditional(axis: &mut Axis, key: &str, def: &Map<String, Value>) -> Option<()> {
    if def.keys().any(|k| k != "condition" && k != "value") {
        return None;
    }
    let cases = match def.get("condition")? {
        Value::Array(cases) => cases.iter().collect(),
        case => vec![case],
    };
    for case in cases {
        let case = case.as_object()?;
        if case.keys().any(|k| k != "test" && k != "value") {
            return None;
        }
        let mut style = TickStyle::default();
        if !tick_style(&mut style, key, case.get("value")?)? {
            return None;
        }
        axis.cases.push(Case {
            test: test(case.get("test")?)?,
            style,
        });
    }
    if let Some(value) = def.get("value")
        && !tick_style(&mut axis.style, key, value)?
    {
        return None;
    }
    Some(())
}

/// A guide's `values`, times for a temporal field.
fn values(value: &Value, kind: Kind) -> Option<Vec<Datum>> {
    value
        .as_array()?
        .iter()
        .map(|value| match value {
            Value::Number(n) => Some(Datum::Number(n.as_f64()?)),
            Value::String(text) if kind == Kind::Temporal => {
                Some(Datum::Number(time::parse(text)?))
            }
            Value::String(text) => Some(Datum::Text(text.clone().into())),
            Value::Bool(b) => Some(Datum::Text(b.to_string().into())),
            Value::Object(date) => Some(Datum::Number(datetime::time(date)?)),
            _ => None,
        })
        .collect()
}

fn tick_count(value: &Value) -> Option<TickCount> {
    let interval = |name: &str| -> Option<Unit> {
        Some(match name {
            "millisecond" => Unit::Millisecond,
            "second" => Unit::Second,
            "minute" => Unit::Minute,
            "hour" => Unit::Hour,
            "day" => Unit::Day,
            "week" => Unit::Week,
            "month" => Unit::Month,
            "year" => Unit::Year,
            _ => return None,
        })
    };
    Some(match value {
        Value::Number(n) => TickCount::Count(n.as_f64()?),
        Value::String(name) => TickCount::Every(interval(name)?, 1.0),
        Value::Object(every) => {
            if every.keys().any(|key| key != "interval" && key != "step") {
                return None;
            }
            let step = match every.get("step") {
                Some(step) => step.as_f64()?,
                None => 1.0,
            };
            TickCount::Every(interval(every.get("interval")?.as_str()?)?, step)
        }
        _ => return None,
    })
}

fn overlap(value: &Value) -> Option<Overlap> {
    Some(match value {
        Value::Bool(false) => Overlap::Off,
        Value::Bool(true) => Overlap::Parity,
        Value::String(method) if method == "parity" => Overlap::Parity,
        Value::String(method) if method == "greedy" => Overlap::Greedy,
        _ => return None,
    })
}

fn side(value: &Value) -> Option<Side> {
    Some(match value.as_str()? {
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        "left" => Side::Left,
        "right" => Side::Right,
        _ => return None,
    })
}

fn anchor(value: &Value) -> Option<Option<Anchor>> {
    Some(match value {
        Value::Null => None,
        value => Some(match value.as_str()? {
            "start" => Anchor::Start,
            "middle" => Anchor::Middle,
            "end" => Anchor::End,
            _ => return None,
        }),
    })
}

fn number(value: &Value) -> Option<f32> {
    Some(value.as_f64()? as f32)
}

/// A flag, or a number standing for its pixels with `true` as 1.
fn tolerance(value: &Value) -> Option<Option<f32>> {
    Some(match value {
        Value::Bool(false) | Value::Null => None,
        Value::Bool(true) => Some(1.0),
        value => Some(number(value)?),
    })
}

/// Reads `key` into `lettering` where it is `prefix` and a text property:
/// `Some(true)` where it is, `Some(false)` where it is not, `None` for a
/// value it does not take.
fn lettering(lettering: &mut Lettering, prefix: &str, key: &str, value: &Value) -> Option<bool> {
    let Some(rest) = key.strip_prefix(prefix) else {
        return Some(false);
    };
    let mut chars = rest.chars();
    let property: String = match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => return Some(false),
    };
    if !prefix.is_empty() && !rest.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Some(false);
    }
    match property.as_str() {
        "color" => lettering.color = Some(encoding::color(value)?),
        "opacity" => lettering.opacity = Some(number(value)?),
        "font" => lettering.font = Some(value.as_str()?.to_string().into()),
        "fontSize" => lettering.size = Some(number(value)?),
        "fontWeight" => lettering.weight = Some(encoding::font_weight(value)?),
        "fontStyle" => lettering.style = Some(encoding::font_style(value)?),
        "limit" => lettering.limit = Some(number(value)?),
        "lineHeight" => lettering.line_height = Some(number(value)?),
        "align" => lettering.align = Some(value.as_str()?.parse::<Align>().ok()?),
        "baseline" => lettering.baseline = Some(value.as_str()?.parse::<Baseline>().ok()?),
        "angle" => lettering.angle = Some(number(value)?),
        _ => return Some(false),
    }
    Some(true)
}

/// Reads `key` into `line` where it is `show` or `prefix` and a stroke
/// property, as [`lettering`] does.
fn line(line: &mut Line, show: &str, prefix: &str, key: &str, value: &Value) -> Option<bool> {
    if key == show {
        line.show = Some(value.as_bool()?);
        return Some(true);
    }
    let Some(property) = key.strip_prefix(prefix) else {
        return Some(false);
    };
    match property {
        "Color" => line.color = Some(encoding::color(value)?),
        "Opacity" => line.opacity = Some(number(value)?),
        "Width" => line.width = Some(number(value)?),
        "Dash" => line.dash = Some(encoding::dash(value)?),
        "DashOffset" => line.dash_offset = Some(number(value)?),
        "Cap" => line.cap = Some(value.as_str()?.parse::<Cap>().ok()?),
        _ => return Some(false),
    }
    Some(true)
}

/// Vega-Lite's `durationExpr`: how long `step` of the least of `parts`
/// lasts from the start of 2001, `None` for days.
fn duration(parts: &[Part], step: f64) -> Option<f64> {
    let start = [2001.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let mut end = start;
    match parts.last()? {
        Part::Year => end[0] += step,
        Part::Quarter => end[1] += 3.0 * step,
        Part::Month => end[1] += step,
        Part::Week => end[2] += 7.0 * step,
        Part::Date | Part::DayOfYear => end[2] += step,
        Part::Day => return None,
        Part::Hours => end[3] += step,
        Part::Minutes => end[4] += step,
        Part::Seconds => end[5] += step,
        Part::Milliseconds => end[6] += step,
    }
    let time = |[y, m, d, h, min, s, ms]: [f64; 7]| time::local(y, m, d, h, min, s, ms);
    Some(time(end)? - time(start)?)
}

/// An `axis` object of a field of `kind`, its time unit's `parts` and
/// step if it has one, with Vega-Lite's defaults for them: ten ticks for a
/// month, quarter, day or hours alone, and no tick closer than a step.
pub(crate) fn axis(
    def: &Map<String, Value>,
    kind: Kind,
    units: Option<(&[Part], f64)>,
) -> Option<Axis> {
    let parts = units.map(|(parts, _)| parts);
    let mut axis = Axis {
        format: format(def, kind, parts)?,
        ..Axis::default()
    };
    if let Some((parts, step)) = units {
        if let [Part::Month | Part::Quarter | Part::Day | Part::Hours] = parts {
            axis.tick_count = Some(TickCount::Count(10.0));
        }
        axis.tick_min_step = duration(parts, step);
    }
    if def.get("format").and_then(Value::as_str) == Some("d") {
        axis.tick_min_step = Some(1.0);
    }
    for (key, value) in def {
        let key = key.as_str();
        if let Value::Object(def) = value
            && def.contains_key("condition")
        {
            conditional(&mut axis, key, def)?;
            continue;
        }
        if tick_style(&mut axis.style, key, value)?
            || lettering(&mut axis.title, "title", key, value)?
            || line(&mut axis.domain, "domain", "domain", key, value)?
        {
            continue;
        }
        match key {
            _ if DROPPED.contains(&key) => {}
            "title" | "format" | "formatType" => {}
            "orient" => axis.orient = Some(side(value)?),
            "offset" => axis.offset = Some(number(value)?),
            "position" => axis.position = Some(number(value)?),
            "translate" => axis.translate = Some(number(value)?),
            "minExtent" => axis.min_extent = Some(number(value)?),
            "maxExtent" => axis.max_extent = Some(number(value)?),
            "bandPosition" => axis.band_position = Some(number(value)?),
            "zindex" => axis.zindex = Some(number(value)?),
            "tickOffset" => axis.tick_offset = Some(number(value)?),
            "tickRound" => axis.tick_round = Some(value.as_bool()?),
            "tickBand" => {
                axis.tick_band = Some(match value.as_str()? {
                    "center" => TickBand::Center,
                    "extent" => TickBand::Extent,
                    _ => return None,
                })
            }
            "tickExtra" => axis.tick_extra = Some(value.as_bool()?),
            "tickCount" => axis.tick_count = Some(tick_count(value)?),
            "tickMinStep" => axis.tick_min_step = Some(value.as_f64()?),
            "values" => axis.values = Some(values(value, kind)?),
            "labels" => axis.labels = Some(value.as_bool()?),
            "labelExpr" => axis.relabel = Some(relabel(value)?),
            "labelBound" => axis.label_bound = tolerance(value)?,
            "labelFlush" => axis.label_flush = Some(tolerance(value)?),
            "labelFlushOffset" => axis.label_flush_offset = Some(number(value)?),
            "labelOverlap" => axis.label_overlap = Some(overlap(value)?),
            "labelSeparation" => axis.label_separation = Some(number(value)?),
            "titleAnchor" => axis.title_anchor = anchor(value)?,
            "titlePadding" => axis.title_padding = Some(number(value)?),
            "titleX" => axis.title_x = Some(number(value)?),
            "titleY" => axis.title_y = Some(number(value)?),
            _ => return None,
        }
    }
    Some(axis)
}

/// A `legend` object of a field of `kind`, its time unit's `parts` if it
/// has one.
pub(crate) fn legend(
    def: &Map<String, Value>,
    kind: Kind,
    parts: Option<&[Part]>,
) -> Option<Legend> {
    let mut legend = Legend {
        format: format(def, kind, parts)?,
        ..Legend::default()
    };
    for (key, value) in def {
        let key = key.as_str();
        if lettering(&mut legend.label, "label", key, value)?
            || lettering(&mut legend.title, "title", key, value)?
        {
            continue;
        }
        let symbols = &mut legend.symbols;
        let ramp = &mut legend.ramp;
        match key {
            _ if DROPPED.contains(&key) || LEGEND_DROPPED.contains(&key) => {}
            "title" | "format" | "formatType" => {}
            "type" => {
                legend.kind = Some(match value.as_str()? {
                    "symbol" => LegendKind::Symbol,
                    "gradient" => LegendKind::Gradient,
                    _ => return None,
                })
            }
            "orient" => {
                legend.orient = Some(match value.as_str()? {
                    "left" => LegendOrient::Left,
                    "right" => LegendOrient::Right,
                    "top" => LegendOrient::Top,
                    "bottom" => LegendOrient::Bottom,
                    "top-left" => LegendOrient::TopLeft,
                    "top-right" => LegendOrient::TopRight,
                    "bottom-left" => LegendOrient::BottomLeft,
                    "bottom-right" => LegendOrient::BottomRight,
                    "none" => LegendOrient::None,
                    _ => return None,
                })
            }
            "legendX" => legend.legend_x = Some(number(value)?),
            "legendY" => legend.legend_y = Some(number(value)?),
            "direction" => {
                legend.direction = Some(match value.as_str()? {
                    "horizontal" => Orient::Horizontal,
                    "vertical" => Orient::Vertical,
                    _ => return None,
                })
            }
            "offset" => legend.offset = Some(number(value)?),
            "padding" => legend.padding = Some(number(value)?),
            "cornerRadius" => legend.corner_radius = Some(number(value)?),
            "fillColor" => legend.fill = Some(encoding::color(value)?),
            "strokeColor" => legend.stroke = Some(encoding::color(value)?),
            "columns" => legend.columns = Some(value.as_u64()? as usize),
            "columnPadding" => legend.column_padding = Some(number(value)?),
            "rowPadding" => legend.row_padding = Some(number(value)?),
            "gridAlign" => {
                legend.grid_align = Some(match value.as_str()? {
                    "each" => GridAlign::Each,
                    "all" => GridAlign::All,
                    "none" => GridAlign::None,
                    _ => return None,
                })
            }
            "clipHeight" => legend.clip_height = Some(number(value)?),
            "tickCount" => legend.tick_count = Some(tick_count(value)?),
            "tickMinStep" => legend.tick_min_step = Some(value.as_f64()?),
            "values" => legend.values = Some(values(value, kind)?),
            "labelExpr" => legend.relabel = Some(relabel(value)?),
            "labelOffset" => legend.label_offset = Some(number(value)?),
            "labelOverlap" => legend.label_overlap = Some(overlap(value)?),
            "labelSeparation" => legend.label_separation = Some(number(value)?),
            "titleAnchor" => legend.title_anchor = anchor(value)?,
            "titleOrient" => legend.title_orient = Some(side(value)?),
            "titlePadding" => legend.title_padding = Some(number(value)?),
            "symbolType" => symbols.shape = Some(encoding::shape(value)?),
            "symbolSize" => symbols.size = Some(number(value)?),
            "symbolStrokeWidth" => symbols.stroke_width = Some(number(value)?),
            "symbolFillColor" => symbols.fill = Some(encoding::color(value)?),
            "symbolStrokeColor" => symbols.stroke = Some(encoding::color(value)?),
            "symbolOpacity" => symbols.opacity = Some(number(value)?),
            "symbolDash" => symbols.dash = Some(encoding::dash(value)?),
            "symbolDashOffset" => symbols.dash_offset = Some(number(value)?),
            "symbolOffset" => symbols.offset = Some(number(value)?),
            "symbolLimit" => symbols.limit = Some(value.as_u64()? as usize),
            "gradientLength" => ramp.length = Some(number(value)?),
            "gradientThickness" => ramp.thickness = Some(number(value)?),
            "gradientStrokeColor" => ramp.stroke = Some(encoding::color(value)?),
            "gradientStrokeWidth" => ramp.stroke_width = Some(number(value)?),
            "gradientOpacity" => ramp.opacity = Some(number(value)?),
            "zindex" => legend.zindex = Some(number(value)?),
            _ => return None,
        }
    }
    Some(legend)
}

/// The top-level `title`: a string, lines, or an object of them and their
/// properties. `None` inside for no title.
pub(crate) fn title(value: &Value) -> Option<Option<Title>> {
    let def = match value {
        Value::Object(def) => def,
        Value::Null => return Some(None),
        text => return Some(encoding::title(text).map(Title::new)),
    };
    let Some(text) = encoding::title(def.get("text")?) else {
        return Some(None);
    };
    let mut title = Title::new(text);
    for (key, value) in def {
        let key = key.as_str();
        if lettering(&mut title.subtitle_lettering, "subtitle", key, value)?
            || (!key.starts_with("subtitle") && lettering(&mut title.lettering, "", key, value)?)
        {
            continue;
        }
        match key {
            _ if DROPPED.contains(&key) => {}
            "text" => {}
            "subtitle" => title.subtitle = encoding::title(value),
            "anchor" => title.anchor = anchor(value)?,
            "orient" => title.orient = Some(side(value)?),
            "offset" => title.offset = Some(number(value)?),
            "frame" => {
                title.frame = Some(match value.as_str()? {
                    "group" => Frame::Group,
                    "bounds" => Frame::Bounds,
                    _ => return None,
                })
            }
            "dx" => title.dx = number(value)?,
            "dy" => title.dy = number(value)?,
            "subtitlePadding" => title.subtitle_padding = Some(number(value)?),
            "zindex" => title.zindex = Some(number(value)?),
            _ => return None,
        }
    }
    Some(Some(title))
}
