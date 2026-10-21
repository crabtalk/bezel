//! A [Vega-Lite](https://vega.github.io/vega-lite/) spec as a chart: one
//! `mark` or a top-level `layer` of them, and an `encoding`. A layer takes the
//! top level's `data` unless it has its own, and the top level's `encoding`
//! under its own.
//!
//! A mark is a type or a definition, whose properties are those
//! [`Properties`] holds by their Vega-Lite names, `point` and `line`
//! overlays among them, and the channels' values: a property named for a
//! channel, or `outerRadius` and `innerRadius` for `radius` and `radius2`,
//! is the channel's value where the `encoding` leaves it out. A shape channel
//! on a line, area or trail with no `point` property draws points over it,
//! and a point overlay is filled and opaque unless it says otherwise. A
//! `fill` or `stroke` property that an encoded `color` paints over is
//! dropped, as are `tooltip`, `invalid`, `aria`, `description`,
//! `cursor`, `style`, the bin and time unit properties, and an arc's
//! corner-specific radii. `lineBreak` is a string or an `expr` that needs
//! neither `datum` nor a param and gives a string or a regular expression.
//!
//! An `encoding` holds the channels [`Encoding`] does, by their Vega-Lite
//! names, each a `field`, a `datum` or a `value`; `detail` and `order` are a
//! field definition or a list of them. A value of `x`, `y`, `x2` or `y2` is
//! pixels, or `"width"` or `"height"`. A
//! [DateTime](https://vega.github.io/vega-lite/docs/datetime.html) object is a
//! time wherever a datum, a scale's domain or a listed sort takes one, and a
//! datum of one is temporal.
//!
//! A field or datum takes its `title`, `scale` (`zero`, `domain`, `nice`) and
//! `sort` (`null`, `"ascending"`, `"descending"` or a list; on `order`,
//! `"ascending"` or `"descending"`) with Vega-Lite's defaults: the field name
//! for a title, ascending for a discrete field and for `order`. An `axis` or
//! `legend` gives its `title` and nothing else. A missing `type` is
//! Vega-Lite's default: nominal, but quantitative for a number datum or a
//! numeric scale type, temporal for a time scale, ordinal for `order` or a
//! listed sort. `x2`, `y2`, `theta2` and `radius2` take their primary's type
//! and state none.
//!
//! A colour value is a CSS colour as d3-color reads it, `null` none; a shape
//! value a Vega symbol name; a `strokeDash` value a list of lengths. A shape
//! value that is an SVG path, a `condition`, and a `value` on `detail`,
//! `order` or `key` answer `None`.
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
//! A `transform` list runs as a pipeline over its view's data before encoding,
//! in order: the top level's before a layer's own, for a layer that takes the
//! top level's `data`. Its ops are `density`, `loess`, `quantile` and
//! `regression` without `params`, computed by [`crate::transform`].
//!
//! A spec with a transform op the pipeline lacks, nested layers, composed or
//! faceted views, params, an aggregate, a bin, a time unit, a mark, mark
//! property, channel or channel property this does not read, or a scale,
//! sort or stack it cannot draw as written answers `None`, as does a chart
//! [`plan`] cannot draw. The `tooltip` channel is dropped.
//!
//! [`Encoding`]: crate::model::Encoding
//! [`Properties`]: crate::model::Properties
//! [`plan`]: crate::plan::plan
//!
//! A temporal field's strings are read by [`time::parse`]; its numbers are
//! milliseconds since the epoch. A quantitative field's strings are read as
//! numbers. Any other field whose values are all numbers or null is numeric,
//! and text otherwise.

use crate::{
    data::{Column, Data},
    model::{Chart, Kind},
    time,
    transform::Transform,
    vega_lite::{data::Sources, path::Path},
};
use serde_json::{Map, Value};
use std::str::FromStr;

mod data;
mod datetime;
mod dsv;
mod encoding;
mod format;
mod mark;
mod path;
mod transform;

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
    /// with the text at it. `None` for a spec this does not read or
    /// [`plan`](crate::plan::plan) cannot draw, and for a url `load` answers
    /// `None`.
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
            Some(Value::Object(title)) => title.get("text").and_then(encoding::title),
            Some(title) => encoding::title(title),
            None => None,
        };
        chart.drawable().then_some(chart)
    }
}

/// The chart `source` describes, or `None` for JSON that is not a spec this
/// reads. A spec with a `data.url` is `None`: [`Spec::chart`] takes a loader.
pub fn import(source: &str) -> Option<Chart> {
    source.parse::<Spec>().ok()?.chart(&|_| None)
}

/// One layer: `mark` drawn from `data` through `pipeline` as `map` says.
fn layer(
    mark: &Value,
    map: &Map<String, Value>,
    data: &Value,
    sources: &Sources,
    pipeline: &[Transform],
) -> Option<Chart> {
    let (mark, properties, map) = mark::read(mark, map)?;
    let mut chart = Chart::new(mark, Data::new()).properties(properties);
    let layer = &mut chart.layers[0];
    layer.encoding = encoding::read(&map)?;

    let rows = sources.rows(data)?;
    let encoded: Vec<(&str, Kind)> = layer
        .encoding
        .channels()
        .into_iter()
        .filter_map(|c| Some((c.field()?.as_ref(), c.kind)))
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
