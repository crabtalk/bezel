//! A [Vega-Lite](https://vega.github.io/vega-lite/) spec as a chart: one
//! `mark` or a top-level `layer` of them, and `x`, `y`, `color` and `theta`
//! channels each naming a `field` and its `type`. A layer takes the top level's
//! `data` unless it has its own, and the top level's `encoding` under its own.
//!
//! A view's `data` is inline `values`, a `url` whose text the host supplies to
//! [`Spec::chart`], a `name` in the top level's `datasets`, or a `sequence`,
//! `sphere` or `graticule` generator, in a `format` of `json`, `csv`, `tsv`,
//! `dsv` or `topojson`. bezel does no I/O. A field name is a path into each
//! datum, as Vega reads it.
//!
//! TopoJSON features and the generators' shapes carry no geometry. A `parse`
//! with a d3 time format (`date:'%Y'`) answers `None`.
//!
//! A channel's `title`, `scale` (`zero`, `domain`, `nice`) and `sort`
//! (`null`, `"ascending"`, `"descending"` or a list) are read with Vega-Lite's
//! defaults: the field name for a title, ascending for a discrete field. An
//! `axis` or `legend` gives its `title` and nothing else.
//!
//! A `transform` list runs as a pipeline over its view's data before encoding,
//! in order: the top level's before a layer's own, for a layer that takes the
//! top level's `data`. Its ops are `density`, `loess`, `quantile` and
//! `regression` without `params`, computed by [`crate::transform`].
//!
//! A spec with a transform op the pipeline lacks, nested layers, composed or
//! faceted views, params, an aggregate, a bin, a time unit, another channel,
//! or a scale, sort or stack it cannot draw as written answers `None`. The `tooltip` channel and a
//! mark's properties past its `type` are dropped.
//!
//! A temporal field's strings are read by [`time::parse`]; its numbers are
//! milliseconds since the epoch. A quantitative field's strings are read as
//! numbers. Any other field whose values are all numbers or null is numeric,
//! and text otherwise.

use crate::{
    data::{Column, Data},
    model::{Channel, Chart, Kind, Mark, Sort},
    scale, time,
    transform::Transform,
    vega_lite::{data::Sources, path::Path},
};
use gpui::SharedString;
use serde_json::{Map, Value};
use std::str::FromStr;

mod data;
mod dsv;
mod format;
mod path;
mod transform;

/// What a channel may carry.
const CHANNEL: [&str; 8] = [
    "field", "type", "title", "axis", "legend", "scale", "sort", "stack",
];
/// Inside a layer as well as at the top.
const UNSUPPORTED: [&str; 7] = [
    "hconcat", "vconcat", "concat", "facet", "repeat", "spec", "params",
];

/// A Vega-Lite spec, read as JSON and not yet drawn.
#[derive(Clone, Debug)]
pub struct Spec(Map<String, Value>);

impl FromStr for Spec {
    type Err = serde_json::Error;

    /// `source` as a JSON object.
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(source).map(Self)
    }
}

impl Spec {
    /// The `data.url` of the top level and of each layer, each once, in the
    /// order written: what [`Self::chart`] asks its loader for.
    pub fn urls(&self) -> Vec<&str> {
        let layers = self.0.get("layer").and_then(Value::as_array);
        let mut urls = Vec::new();
        for view in std::iter::once(&self.0)
            .chain(layers.into_iter().flatten().filter_map(Value::as_object))
        {
            if let Some(url) = view.get("data").and_then(|data| data.get("url")?.as_str())
                && !urls.contains(&url)
            {
                urls.push(url);
            }
        }
        urls
    }

    /// The chart the spec describes, `load` answering each of [`Self::urls`]
    /// with the text at it. `None` for a spec this does not read, and for a url
    /// `load` answers `None`.
    pub fn chart<'a>(&self, load: &dyn Fn(&str) -> Option<&'a str>) -> Option<Chart> {
        let spec = &self.0;
        let sources = Sources {
            datasets: match spec.get("datasets") {
                Some(datasets) => Some(datasets.as_object()?),
                None => None,
            },
            load,
        };
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
            let top = std::ptr::eq(unit, spec);
            let data = unit.get("data").or(spec.get("data"))?;
            let mut pipeline = Vec::new();
            let inherited = (!top && !unit.contains_key("data")).then(|| spec.get("transform"));
            for list in [inherited.flatten(), unit.get("transform")]
                .into_iter()
                .flatten()
            {
                pipeline.extend(transform::pipeline(list)?);
            }
            let mut encoding = shared.clone();
            if !top && let Some(own) = unit.get("encoding") {
                encoding.extend(own.as_object()?.clone());
            }
            layers.push(layer(
                unit.get("mark")?,
                &encoding,
                data,
                &sources,
                &pipeline,
            )?);
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
}

/// The chart `source` describes, or `None` for JSON that is not a spec this
/// reads. A spec with a `data.url` is `None`: [`Spec::chart`] takes a loader.
pub fn import(source: &str) -> Option<Chart> {
    source.parse::<Spec>().ok()?.chart(&|_| None)
}

/// One layer: `mark` drawn from `data` through `pipeline` as `encoding` says.
fn layer(
    mark: &Value,
    encoding: &Map<String, Value>,
    data: &Value,
    sources: &Sources,
    pipeline: &[Transform],
) -> Option<Chart> {
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

    let rows = sources.rows(data)?;
    let channels = [
        &layer.encoding.x,
        &layer.encoding.y,
        &layer.encoding.color,
        &layer.encoding.theta,
    ];
    let encoded: Vec<(&str, Kind)> = channels
        .iter()
        .filter_map(|c| c.as_ref())
        .map(|c| (c.field.as_ref(), c.kind))
        .collect();
    let mut fields: Vec<(&str, Option<Kind>)> = Vec::new();
    for (field, kind) in encoded
        .iter()
        .map(|(field, kind)| (*field, Some(*kind)))
        .chain(
            rows.iter()
                .flat_map(|row| row.keys().map(|key| (key.as_str(), None))),
        )
    {
        match fields.iter_mut().find(|(name, _)| *name == field) {
            // A temporal channel reads its field as times, over any other.
            Some((_, slot)) => {
                if kind == Some(Kind::Temporal) {
                    *slot = kind;
                }
            }
            None => fields.push((field, kind)),
        }
    }
    let mut source = Data::new();
    for (field, kind) in fields {
        let values: Vec<&Value> = match kind {
            Some(_) => {
                let path: Path = field.parse().ok()?;
                rows.iter()
                    .map(|row| path.field(row).unwrap_or(&Value::Null))
                    .collect()
            }
            None => rows
                .iter()
                .map(|row| row.get(field).unwrap_or(&Value::Null))
                .collect(),
        };
        source = source.with(field.to_string(), column(&values, kind));
    }
    let source = source.transform(pipeline);
    let mut data = Data::new();
    for (field, _) in encoded {
        if data.column(field).is_some() {
            continue;
        }
        let column = match source.column(field) {
            Some(column) => column.clone(),
            None => Column::Number(vec![f64::NAN; source.len()].into()),
        };
        data = data.with(field.to_string(), column);
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

/// A field's values as the channel of `kind` that encodes it reads them.
fn column(values: &[&Value], kind: Option<Kind>) -> Column {
    let numbers =
        |read: fn(&Value) -> f64| Column::Number(values.iter().map(|v| read(v)).collect());
    match kind {
        Some(Kind::Temporal) => numbers(|value| match value {
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(s) => time::parse(s).unwrap_or(f64::NAN),
            _ => f64::NAN,
        }),
        Some(Kind::Quantitative) => numbers(|value| match value {
            Value::Null => f64::NAN,
            value => format::number_of(value),
        }),
        _ if values.iter().all(|v| v.is_number() || v.is_null()) => {
            numbers(|value| value.as_f64().unwrap_or(f64::NAN))
        }
        _ => Column::Text(
            values
                .iter()
                .map(|value| match value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                })
                .collect(),
        ),
    }
}
