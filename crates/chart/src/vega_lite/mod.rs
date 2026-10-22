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
//! dropped, as are `tooltip`, `invalid`, `aria`, `description`, `cursor`,
//! `style`, `binSpacing`, `timeUnitBandSize`, `timeUnitBandPosition`, and an
//! arc's corner-specific radii. `lineBreak` is a string or an `expr` that
//! needs neither `datum` nor a param and gives a string or a regular
//! expression.
//!
//! An `encoding` holds the channels [`Encoding`] does, by their Vega-Lite
//! names, each a `field`, a `datum` or a `value`; `detail` and `order` are a
//! field definition or a list of them. A value of `x`, `y`, `x2` or `y2` is
//! pixels, or `"width"` or `"height"`. A
//! [DateTime](https://vega.github.io/vega-lite/docs/datetime.html) object is a
//! time wherever a datum, a scale's domain or a listed sort takes one, and a
//! datum of one is temporal.
//!
//! A field or datum takes its `title`, `scale`, `stack` and `sort` (`null`,
//! `"ascending"`, `"descending"`, a list, an `{op, field, order}` or
//! `{encoding, order}` object, or a channel's name with or without a `-`; on
//! `order`, `"ascending"` or `"descending"`) with Vega-Lite's defaults:
//! Vega-Lite's title (`Count of Records`, `Sum of x`, `x (binned)`,
//! `x (year-month)`), ascending for a discrete field and for `order`; a
//! continuous field sorted `"descending"` runs its scale backwards. A
//! `scale` reads `type`, `zero`, `domain` (a list, or `"unaggregated"`),
//! `domainMin`, `domainMax`, `domainMid`, `range` (a list of what the
//! channel takes, a named range, or a colour channel's `{field}`),
//! `rangeMin`, `rangeMax`, `scheme` (a name, a list of colours, or `name`,
//! `extent` and `count`), `reverse`, `clamp`, `nice` (a flag, a tick count, a
//! time interval or `interval` and `step`), `padding`, `paddingInner`,
//! `paddingOuter`, `round`, `interpolate` (a name, or `type` and `gamma`),
//! `exponent`, `base` and `constant`; a type the channel or field type does
//! not take is Vega-Lite's default. A `"unaggregated"` domain spans each
//! group's least to greatest value, as `min_<field>` and `max_<field>`, where
//! Vega-Lite takes it: on a `mean`, `average`, `median`, `q1`, `q3`, `min`,
//! `max`, `argmin` or `argmax` not on a log scale. A `scale` of `null` draws
//! each value as it is; a range `{step}` and `domainRaw` answer `None`.
//! Layers share a discrete channel's domain as Vega-Lite merges theirs.
//! `stack` is `true`, `"zero"`, `"normalize"`, `"center"`, `false` or
//! `null`. An `axis` or `legend` gives its `title` and nothing else. A
//! missing `type` is Vega-Lite's default: nominal, but quantitative for a
//! number datum, an aggregate other than `argmin` and `argmax`, a bin or a
//! numeric scale type, temporal for a time unit or a time scale, ordinal for
//! `order` or a listed sort. A counting aggregate is quantitative whatever
//! its `type`. `x2`, `y2`, `theta2` and `radius2` take their primary's type
//! and state none.
//!
//! A field definition's `aggregate`, `bin` and `timeUnit`, one of them at
//! most, derive its field as Vega-Lite's dataflow does, under Vega-Lite's
//! names (`sum_x`, `__count`, `argmax_y` with `argmax_y.x` for the field `x`
//! of each group's row of greatest `y`, `bin_maxbins_10_x` and
//! `bin_maxbins_10_x_end`, `month_date` and `month_date_end`). A `tooltip`'s
//! field definitions draw nothing but group an aggregate. A bar or rect spans
//! a quantitative bin, or a temporal time unit's step, where no `x2` or `y2`
//! ends it, a step shifted to centre on a `bandPosition` other than 0.5;
//! another mark stands at the bin's `bandPosition`, its middle unless given,
//! and at a time unit's where given. A discrete bin's channel reads its
//! edges as text, `<start>_range`, in the order the bins start. A binned
//! axis ticks every bin's edge. A quantitative bin on `color`, `fill` or
//! `stroke` takes a bin-ordinal scale, and on another channel with a
//! continuous scale one from the bins' start to their stop; the legend of
//! either lists the bins. `timeUnit` with no `unit` and the `values` op
//! answer `None`. A time unit's dates group to the second, as Vega keys a
//! date by its text.
//!
//! A colour value is a CSS colour as d3-color reads it, `null` none; a shape
//! value a Vega symbol name or SVG path data; a `strokeDash` value a list of
//! lengths. A `condition`, and a `value` on `detail`, `order` or `key`
//! answer `None`.
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
//! top level's `data`. Its ops are `aggregate`, `bin`, `density`, `loess`,
//! `quantile`, `regression` without `params` and `timeUnit`, computed by
//! [`crate::transform`]. A time unit's field is read as times.
//!
//! A spec with a transform op the pipeline lacks, nested layers, composed or
//! faceted views, params, a mark, mark property, channel or channel property
//! this does not read, or a scale, sort or stack it cannot draw as written
//! answers `None`, as does a chart [`plan`] cannot draw. The `tooltip`
//! channel draws nothing.
//!
//! [`Encoding`]: crate::model::Encoding
//! [`Properties`]: crate::model::Properties
//! [`plan`]: crate::plan::plan
//!
//! A temporal field's strings are read by [`time::parse`]; its numbers are
//! milliseconds since the epoch; so are a time unit's field's. A
//! quantitative field's strings are read as numbers, but a binned field's,
//! and an aggregated one's but for `min` and `max`, are kept as text, which
//! the transforms read as Vega does. Any other field whose values are all
//! numbers or null is numeric, and text otherwise; a field no channel types
//! is numeric with empty strings among its numbers as well. Text compares
//! as strings.

use crate::{
    data::{Column, Data},
    model::{Chart, Domain, Kind, Range},
    time,
    transform::{Random, Transform},
    vega_lite::{data::Sources, encoding::Def, field::Derive, path::Path, union::Layered},
};
use gpui::SharedString;
use serde_json::{Map, Value};
use std::str::FromStr;

mod data;
mod dataflow;
mod datetime;
mod dsv;
mod encoding;
mod field;
mod format;
mod mark;
mod path;
mod transform;
mod union;

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
        let mut random = Random::default();
        for unit in units {
            let nested = !std::ptr::eq(unit, spec) && unit.contains_key("layer");
            if nested || UNSUPPORTED.iter().any(|key| unit.contains_key(*key)) {
                return None;
            }
            let top = std::ptr::eq(unit, spec);
            let data = unit.get("data").or(spec.get("data"))?;
            let inherited = (!top && !unit.contains_key("data")).then(|| spec.get("transform"));
            let mut pipeline = match inherited.flatten() {
                Some(list) => transform::pipeline(list)?,
                None => Vec::new(),
            };
            // A layer bins its encoding's fields before its own transforms.
            let bins_at = pipeline.len();
            if let Some(list) = unit.get("transform") {
                pipeline.extend(transform::pipeline(list)?);
            }
            let bins_at = if top { pipeline.len() } else { bins_at };
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
                bins_at,
                &mut random,
            )?);
        }
        union::share(&mut layers, &mut random);
        let mut layers = layers.into_iter().map(|layer| layer.chart);
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

/// One layer: `mark` drawn from `data` through `pipeline` as `map` says,
/// the encoding's bins before the op at `bins_at`, its ops drawing from
/// `random`.
fn layer(
    mark: &Value,
    map: &Map<String, Value>,
    data: &Value,
    sources: &Sources,
    pipeline: &[Transform],
    bins_at: usize,
    random: &mut Random,
) -> Option<Layered> {
    let (mark, properties, map) = mark::read(mark, map)?;
    let mut chart = Chart::new(mark, Data::new()).properties(properties);
    let layer = &mut chart.layers[0];
    let (encoding, defs) = encoding::read(&map, Some(mark))?;
    layer.encoding = encoding;

    let rows = sources.rows(data)?;
    // A time unit's field is read as times wherever it is, and a range
    // field as the channel's values are, as text.
    let units = pipeline.iter().filter_map(|op| match op {
        Transform::TimeUnit(unit) => Some((unit.field.clone(), Some(Kind::Temporal))),
        _ => None,
    });
    let ranges: Vec<(SharedString, Option<Kind>)> = layer
        .encoding
        .channels()
        .iter()
        .filter_map(|c| match &c.scale.range {
            Some(Range::Field(field)) => Some((field.clone(), Some(Kind::Nominal))),
            _ => None,
        })
        .collect();
    let reads: Vec<(SharedString, Option<Kind>)> = defs
        .iter()
        .flat_map(Def::reads)
        .chain(units)
        .chain(ranges.iter().cloned())
        .collect();
    // Each field once, the encoding's read as paths into each row and with
    // the type they are read as.
    let mut fields: Vec<(&str, Option<Kind>, bool)> = Vec::new();
    for (field, kind, path) in reads
        .iter()
        .map(|(field, kind)| (field.as_ref(), *kind, true))
        .chain(
            rows.iter()
                .flat_map(|row| row.keys().map(|key| (key.as_str(), None, false))),
        )
    {
        match fields.iter_mut().find(|(name, ..)| *name == field) {
            // A temporal channel reads its field as times, over any other.
            Some((_, slot, _)) => {
                if kind == Some(Kind::Temporal) || slot.is_none() {
                    *slot = kind;
                }
            }
            None => fields.push((field, kind, path)),
        }
    }
    let mut source = Data::new();
    for (field, kind, path) in fields {
        let values: Vec<&Value> = match path {
            true => {
                let path: Path = field.parse().ok()?;
                rows.iter()
                    .map(|row| path.field(row).unwrap_or(&Value::Null))
                    .collect()
            }
            false => rows
                .iter()
                .map(|row| row.get(field).unwrap_or(&Value::Null))
                .collect(),
        };
        source = source.with(field.to_string(), column(&values, kind));
    }
    let flow = dataflow::run(source, pipeline, bins_at, &defs, mark, random)?;
    for (def, order) in defs.iter().zip(flow.orders) {
        if let (Some(order), Some(channel)) = (
            order,
            encoding::scaled_mut(&mut layer.encoding, &def.channel),
        ) {
            channel.sort = order;
        }
    }
    for def in &defs {
        let Derive::Bin(bin) = &def.derive else {
            continue;
        };
        let Some(&(_, bins)) = flow.bins.iter().find(|(start, _)| *start == bin.output[0]) else {
            continue;
        };
        let Some(channel) = encoding::scaled_mut(&mut layer.encoding, &def.channel) else {
            continue;
        };
        if channel.kind == Kind::Quantitative {
            channel.scale.bins = Some(bins);
        }
    }
    let source = flow.data;
    let mut data = Data::new();
    let channels = layer.encoding.channels();
    let encoded = channels.iter().flat_map(|channel| {
        let spans = match &channel.scale.domain {
            Some(Domain::Unaggregated(fields)) => &fields[..],
            _ => &[],
        };
        channel.field().into_iter().chain(spans)
    });
    for field in encoded.chain(ranges.iter().map(|(field, _)| field)) {
        if data.column(field).is_some() {
            continue;
        }
        let column = match source.column(field) {
            Some(column) => column.clone(),
            None => Column::Number(vec![f64::NAN; source.len()].into()),
        };
        data = data.adopt(&source, field.clone(), column);
    }
    layer.data = data;
    Some(Layered {
        chart,
        defs,
        raw: flow.raw,
        sorts: flow.sorts,
    })
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
        // Read as it is, an empty string is missing as null is.
        None if values
            .iter()
            .all(|v| v.is_number() || v.is_null() || v.as_str() == Some("")) =>
        {
            numbers(|value| value.as_f64().unwrap_or(f64::NAN))
        }
        _ => Column::Text(
            values
                .iter()
                .map(|value| match value {
                    Value::Null => String::new(),
                    value => format::string_of(value),
                })
                .collect(),
        ),
    }
}
