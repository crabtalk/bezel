//! A [Vega-Lite](https://vega.github.io/vega-lite/) spec as a chart: one
//! `mark` or a top-level `layer` of them, and an `encoding`. A layer takes the
//! top level's `data` unless it has its own, and the top level's `encoding`
//! under its own, a channel's field or datum definition over the top level's
//! definition of it.
//!
//! A `boxplot`, `errorbar` or `errorband` mark draws the layer of part marks
//! Vega-Lite expands it to, over the transforms that compute their fields.
//!
//! A mark is a type or a definition, whose properties are those
//! [`Properties`] holds by their Vega-Lite names, `point` and `line`
//! overlays among them, and the channels' values: a property named for a
//! channel, or `outerRadius` and `innerRadius` for `radius` and `radius2`,
//! is the channel's value where the `encoding` leaves it out. A shape channel
//! on a line, area or trail with no `point` property draws points over it,
//! and a point overlay is filled and opaque unless it says otherwise. A
//! `fill` or `stroke` property that an encoded `color` paints over is
//! dropped, as are `tooltip`, `aria`, `description`, `cursor`, `style`,
//! `binSpacing`, `timeUnitBandSize`, `timeUnitBandPosition`, and an arc's
//! corner-specific radii. `lineBreak` is a string or an `expr` that needs
//! neither `datum` nor a param and gives a string or a regular expression.
//! `invalid` is read with `config.<mark>.invalid`, or else
//! `config.mark.invalid`, as Vega-Lite 6.5 reads them: the marks by the
//! mark's own mode or else the config's, the domains by the rows the marks
//! draw unless the config's mode alone parts them.
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
//! `null`. An `axis` on `x` or `y` and a `legend` on a colour, size, shape,
//! dash, stroke width or opacity channel read every property of Vega-Lite's;
//! `null` draws none. A time unit's guide writes its values in Vega-Lite's
//! format for the unit. The top level's `title` is a string, lines, or an
//! object of them and every property of Vega-Lite's. A
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
//! either lists the bins. `timeUnit` with no `unit` answers `None`. A time
//! unit's dates group to the second, as Vega keys a date by its text.
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
//! `quantile`, `regression` and `timeUnit`, computed by
//! [`crate::transform`]. A time unit's field is read as times. The ops run
//! as Vega-Lite's optimized dataflow runs them: once for every layer that
//! shares them, layers' aggregates over the same groups as one, each drawing
//! from one generator in the order Vega evaluates them.
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
//! A temporal field's strings are read by [`crate::time::parse`]; its numbers are
//! milliseconds since the epoch; so are a time unit's field's. A
//! quantitative field's strings are read as numbers, but a binned field's,
//! and an aggregated one's but for `min` and `max`, are kept as text, which
//! the transforms read as Vega does. Any other field whose values are all
//! numbers or null is numeric, and text otherwise; a field no channel types
//! is numeric with empty strings among its numbers as well, text where its
//! values are all strings or null, and holds its values as they are where
//! not, as does a field that holds an array or object. Text compares as
//! strings; its null is a value of its own, `null`, and a row's missing
//! field another, `undefined`. Layers of one source read a field one way:
//! as times where any reads it so, and otherwise as the first to type it
//! does.

use crate::{
    model::Chart,
    transform::Random,
    vega_lite::{data::Sources, dataflow::Tree, view::View},
};
use serde_json::{Map, Value};
use std::str::FromStr;

mod composite;
mod data;
mod dataflow;
mod datetime;
mod dsv;
mod encoding;
mod field;
mod format;
mod guide;
mod mark;
mod sort;
mod transform;
mod union;
mod unit;
mod view;

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
        let view = View::of(spec)?;
        let (mut tree, read, units) = Tree::parse(&view, spec.get("config"))?;
        tree.optimize();
        let mut random = Random::default();
        let ran = tree.run(
            &mut |source| {
                let source = &read[source];
                sources.table(source.data, &source.reads)
            },
            &mut random,
        )?;
        let mut layers = Vec::with_capacity(units.len());
        for unit in units {
            let raw = ran.rows(tree.tap(unit.raw))?.clone();
            let main = tree.tap(unit.main);
            let rows = ran.rows(main)?.clone();
            let bins = |field: &str| ran.bins(&tree, main, field);
            layers.push(unit.layer(raw, rows, &bins, &mut random)?);
        }
        union::share(&mut layers, &mut random);
        let mut layers = layers.into_iter().map(|layer| layer.chart);
        let mut chart = layers.next()?;
        for layer in layers {
            chart = chart.layer(layer);
        }
        chart.title = match spec.get("title") {
            Some(title) => guide::title(title)?,
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
