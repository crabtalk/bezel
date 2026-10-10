//! A [Vega-Lite](https://vega.github.io/vega-lite/) spec as a chart: inline
//! `data.values`, one `mark` or a top-level `layer` of them, and `x`, `y`,
//! `color` and `theta` channels each naming a `field` and its `type`. A layer
//! takes the top level's `data` unless it has its own, and the top level's
//! `encoding` under its own.
//!
//! A channel's `title`, `scale` (`zero`, `domain`, `nice`) and `sort`
//! (`null`, `"ascending"`, `"descending"` or a list) are read with Vega-Lite's
//! defaults: the field name for a title, ascending for a discrete field. An
//! `axis` or `legend` gives its `title` and nothing else.
//!
//! A spec with a transform, nested layers, composed or faceted views, params, an
//! aggregate, a bin, a time unit, another channel, or a scale, sort or stack
//! it cannot draw as written answers `None`. The `tooltip` channel and a
//! mark's properties past its `type` are dropped.
//!
//! A temporal field's strings are read by [`time::parse`]; its numbers are
//! milliseconds since the epoch. A field whose values are all numbers or null
//! is numeric; any other field is text.

use gpui::SharedString;
use serde_json::{Map, Value};

use crate::{
    data::{Column, Data},
    model::{Channel, Chart, Kind, Mark, Sort},
    scale, time,
};

/// What a channel may carry.
const CHANNEL: [&str; 8] = [
    "field", "type", "title", "axis", "legend", "scale", "sort", "stack",
];
/// Inside a layer as well as at the top.
const UNSUPPORTED: [&str; 8] = [
    "transform",
    "hconcat",
    "vconcat",
    "concat",
    "facet",
    "repeat",
    "spec",
    "params",
];

/// The chart `source` describes, or `None` for JSON that is not a spec this
/// reads.
pub fn import(source: &str) -> Option<Chart> {
    let spec: Value = serde_json::from_str(source).ok()?;
    let spec = spec.as_object()?;
    let empty = Map::new();
    let shared = match spec.get("encoding") {
        Some(encoding) => encoding.as_object()?,
        None => &empty,
    };
    let units: Vec<&Map<String, Value>> = match spec.get("layer") {
        Some(layers) => layers
            .as_array()?
            .iter()
            .map(Value::as_object)
            .collect::<Option<_>>()?,
        None => vec![spec],
    };
    let mut layers = Vec::with_capacity(units.len());
    for unit in units {
        let nested = !std::ptr::eq(unit, spec) && unit.contains_key("layer");
        if nested || UNSUPPORTED.iter().any(|key| unit.contains_key(*key)) {
            return None;
        }
        let data = unit.get("data").or(spec.get("data"))?;
        let mut encoding = shared.clone();
        if !std::ptr::eq(unit, spec)
            && let Some(own) = unit.get("encoding")
        {
            encoding.extend(own.as_object()?.clone());
        }
        layers.push(layer(unit.get("mark")?, &encoding, data)?);
    }
    let mut layers = layers.into_iter();
    let mut chart = layers.next()?;
    for layer in layers {
        chart = chart.layer(layer);
    }
    chart.title = match spec.get("title") {
        Some(Value::Object(title)) => title.get("text").and_then(text),
        Some(title) => text(title),
        None => None,
    };
    Some(chart)
}

/// One layer: `mark` drawn from `data` as `encoding` says.
fn layer(mark: &Value, encoding: &Map<String, Value>, data: &Value) -> Option<Chart> {
    let mark = match mark {
        Value::String(mark) => mark.as_str(),
        Value::Object(mark) => mark.get("type")?.as_str()?,
        _ => return None,
    };
    let mark = match mark {
        "bar" => Mark::Bar,
        "line" => Mark::Line,
        "area" => Mark::Area,
        "point" | "circle" | "square" => Mark::Point,
        "arc" => Mark::Arc,
        _ => return None,
    };

    let mut chart = Chart::new(mark, Data::new());
    let layer = &mut chart.layers[0];
    for (name, value) in encoding {
        let slot = match name.as_str() {
            "x" => &mut layer.encoding.x,
            "y" => &mut layer.encoding.y,
            "color" => &mut layer.encoding.color,
            "theta" => &mut layer.encoding.theta,
            "tooltip" => continue,
            _ => return None,
        };
        *slot = Some(channel(value.as_object()?)?);
    }

    let rows = data.get("values")?.as_array()?;
    let rows: Vec<&Map<String, Value>> =
        rows.iter().map(Value::as_object).collect::<Option<_>>()?;
    let channels = [
        &layer.encoding.x,
        &layer.encoding.y,
        &layer.encoding.color,
        &layer.encoding.theta,
    ];
    let mut data = Data::new();
    for channel in channels.into_iter().flatten() {
        if data.column(&channel.field).is_some() {
            continue;
        }
        let temporal = channels
            .iter()
            .filter_map(|c| c.as_ref())
            .any(|c| c.field == channel.field && c.kind == Kind::Temporal);
        let values: Vec<&Value> = rows
            .iter()
            .map(|row| row.get(channel.field.as_ref()).unwrap_or(&Value::Null))
            .collect();
        data = data.with(channel.field.clone(), column(&values, temporal));
    }
    layer.data = data;
    Some(chart)
}

fn channel(channel: &Map<String, Value>) -> Option<Channel> {
    if channel.keys().any(|key| !CHANNEL.contains(&key.as_str())) {
        return None;
    }
    let kind = match channel.get("type")?.as_str()? {
        "quantitative" => Kind::Quantitative,
        "temporal" => Kind::Temporal,
        "ordinal" => Kind::Ordinal,
        "nominal" => Kind::Nominal,
        _ => return None,
    };
    let field = channel.get("field")?.as_str()?.to_string();
    let mut out = Channel::new(field.clone(), kind);

    let guide = ["axis", "legend"]
        .into_iter()
        .filter_map(|key| channel.get(key)?.as_object()?.get("title"))
        .next();
    out.title = match guide.or(channel.get("title")) {
        None => Some(field.into()),
        Some(title) => text(title),
    };

    if let Some(value) = channel.get("scale") {
        let scale = value.as_object()?;
        for (key, value) in scale {
            match (key.as_str(), value) {
                ("zero", Value::Bool(zero)) => out.scale.zero = *zero,
                ("nice", Value::Bool(nice)) => out.scale.nice = *nice,
                ("domain", Value::Array(ends))
                    if matches!(kind, Kind::Quantitative | Kind::Temporal) =>
                {
                    let end = |value: &Value| match value {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) if kind == Kind::Temporal => time::parse(s),
                        _ => None,
                    };
                    let [low, high] = ends.as_slice() else {
                        return None;
                    };
                    out.scale.domain = Some([end(low)?, end(high)?]);
                }
                ("type", Value::String(name))
                    if matches!(
                        (kind, name.as_str()),
                        (Kind::Quantitative, "linear")
                            | (Kind::Temporal, "time" | "utc")
                            | (Kind::Ordinal | Kind::Nominal, "band" | "point" | "ordinal")
                    ) => {}
                _ => return None,
            }
        }
    }

    if matches!(kind, Kind::Ordinal | Kind::Nominal) {
        out.sort = match channel.get("sort") {
            None => Sort::Ascending,
            Some(Value::Null) => Sort::Data,
            Some(Value::String(order)) if order == "ascending" => Sort::Ascending,
            Some(Value::String(order)) if order == "descending" => Sort::Descending,
            Some(Value::Array(values)) => Sort::Explicit(
                values
                    .iter()
                    .map(|value| match value {
                        Value::String(s) => Some(s.clone().into()),
                        Value::Number(n) => Some(scale::value(n.as_f64()?).into()),
                        _ => None,
                    })
                    .collect::<Option<_>>()?,
            ),
            Some(_) => return None,
        };
    }

    match channel.get("stack") {
        None | Some(Value::Bool(true)) => {}
        Some(Value::String(stack)) if stack == "zero" => {}
        Some(_) => return None,
    }
    Some(out)
}

/// A title: a string, lines joined by spaces, or `null` for none.
fn text(value: &Value) -> Option<SharedString> {
    match value {
        Value::String(text) => Some(text.clone().into()),
        Value::Array(lines) => Some(
            lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
                .into(),
        ),
        _ => None,
    }
}

fn column(values: &[&Value], temporal: bool) -> Column {
    if temporal {
        return Column::Number(
            values
                .iter()
                .map(|value| match value {
                    Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
                    Value::String(s) => time::parse(s).unwrap_or(f64::NAN),
                    _ => f64::NAN,
                })
                .collect(),
        );
    }
    if values.iter().all(|v| v.is_number() || v.is_null()) {
        return Column::Number(
            values
                .iter()
                .map(|v| v.as_f64().unwrap_or(f64::NAN))
                .collect(),
        );
    }
    Column::Text(
        values
            .iter()
            .map(|value| match value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            })
            .collect(),
    )
}
