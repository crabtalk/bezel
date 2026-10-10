//! A [Vega-Lite](https://vega.github.io/vega-lite/) spec as a chart: inline
//! `data.values`, one `mark`, and `x`, `y`, `color` and `theta` channels each
//! naming a `field` and its `type`.
//!
//! A spec with a transform, layers, composed or faceted views, params, an
//! aggregate, a bin, a time unit or another channel answers `None`. A
//! channel's `title`, `axis`, `legend`, `scale`, `sort` and `stack`, the
//! `tooltip` channel, and a mark's properties past its `type` are dropped.
//!
//! A temporal field's strings are read by [`time::parse`]; its numbers are
//! milliseconds since the epoch. A field whose values are all numbers or null
//! is numeric; any other field is text.

use serde_json::{Map, Value};

use crate::{
    data::{Column, Data},
    model::{Channel, Chart, Kind, Mark},
    time,
};

/// What a channel may carry.
const CHANNEL: [&str; 8] = [
    "field", "type", "title", "axis", "legend", "scale", "sort", "stack",
];
const UNSUPPORTED: [&str; 9] = [
    "transform",
    "layer",
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
    if UNSUPPORTED.iter().any(|key| spec.contains_key(*key)) {
        return None;
    }
    let mark = match spec.get("mark")? {
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

    let encoding = spec.get("encoding")?.as_object()?;
    let mut chart = Chart::new(mark, Data::new());
    for (name, value) in encoding {
        let slot = match name.as_str() {
            "x" => &mut chart.encoding.x,
            "y" => &mut chart.encoding.y,
            "color" => &mut chart.encoding.color,
            "theta" => &mut chart.encoding.theta,
            "tooltip" => continue,
            _ => return None,
        };
        *slot = Some(channel(value.as_object()?)?);
    }

    let rows = spec.get("data")?.get("values")?.as_array()?;
    let rows: Vec<&Map<String, Value>> =
        rows.iter().map(Value::as_object).collect::<Option<_>>()?;
    let channels = [
        &chart.encoding.x,
        &chart.encoding.y,
        &chart.encoding.color,
        &chart.encoding.theta,
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
    chart.data = data;

    chart.title = match spec.get("title") {
        Some(Value::String(title)) => Some(title.clone().into()),
        Some(Value::Object(title)) => title
            .get("text")
            .and_then(Value::as_str)
            .map(|t| t.to_string().into()),
        _ => None,
    };
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
    Some(Channel::new(
        channel.get("field")?.as_str()?.to_string(),
        kind,
    ))
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
