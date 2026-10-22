//! Vega-Lite's composite marks, `boxplot`, `errorbar` and `errorband`, as
//! its normalizer expands each into a layer of part marks over the
//! transforms that compute their fields, under Vega-Lite's default config.
//!
//! A part property (`box`, `median`, `whisker`'s `rule` and `ticks`,
//! `outliers`, `band`, `borders`) is `true`, `false` or mark properties
//! over the part's own. A composite mark's `color`, `opacity` and `clip`
//! go to every part; its other properties than those it reads are dropped,
//! as Vega-Lite drops them. The parts' tooltips are dropped; a `tooltip`
//! field groups and an aggregated one measures as Vega-Lite's do. A box
//! plot's median takes its own colour where Vega-Lite's `condition` would
//! paint it the box's colour over a box with no height.

use crate::{
    model::{Kind, Orient},
    transform::{Aggregate, Bin, Measure, Op, TimeUnit, Transform},
    vega_lite::{
        dataflow::{Formula, Step},
        encoding,
        field::Derive,
        format,
        view::{Body, View},
    },
};
use gpui::SharedString;
use serde_json::{Map, Value};
use std::{fmt, str::FromStr};
use vega::path::Path;

mod boxplot;
mod errorband;
mod errorbar;

/// Channels that end a primary's span, which state no type.
const SECONDARY: [&str; 6] = ["x2", "y2", "xError", "xError2", "yError", "yError2"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Composite {
    BoxPlot,
    ErrorBar,
    ErrorBand,
}

const COMPOSITES: [(Composite, &str); 3] = [
    (Composite::BoxPlot, "boxplot"),
    (Composite::ErrorBar, "errorbar"),
    (Composite::ErrorBand, "errorband"),
];

/// A composite mark by its Vega-Lite name.
impl FromStr for Composite {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        COMPOSITES
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(composite, _)| *composite)
            .ok_or(())
    }
}

/// Vega-Lite's name of the composite mark.
impl fmt::Display for Composite {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (_, name) = COMPOSITES
            .iter()
            .find(|(composite, _)| composite == self)
            .expect("every composite mark is named");
        f.write_str(name)
    }
}

impl Composite {
    /// A unit of this mark, `mark` its type or definition, as the view
    /// Vega-Lite normalizes it to: `data` and `transform` the unit's own.
    /// `None` where Vega-Lite throws, or for an encoding this does not read.
    pub(crate) fn normalize(
        self,
        mark: &Value,
        encoding: &Map<String, Value>,
        data: Option<Value>,
        transform: Vec<Step>,
    ) -> Option<View> {
        let def = match mark {
            Value::Object(def) => def.clone(),
            _ => Map::from_iter([("type".to_string(), Value::String(self.to_string()))]),
        };
        let unit = Unit {
            composite: self,
            def,
            encoding: typed(encoding)?,
            data,
            transform,
        };
        match self {
            Self::BoxPlot => unit.boxplot(),
            Self::ErrorBar => unit.errorbar(),
            Self::ErrorBand => unit.errorband(),
        }
    }
}

/// A composite mark's unit, its encoding typed as Vega-Lite's
/// `normalizeEncoding` types it.
struct Unit {
    composite: Composite,
    def: Map<String, Value>,
    encoding: Map<String, Value>,
    data: Option<Value>,
    transform: Vec<Step>,
}

/// The channel along the continuous axis of a composite mark running
/// `orient`.
fn axis(orient: Orient) -> &'static str {
    match orient {
        Orient::Horizontal => "x",
        Orient::Vertical => "y",
    }
}

/// The way a tick across a composite mark running `orient` runs.
fn across(orient: Orient) -> Orient {
    match orient {
        Orient::Horizontal => Orient::Vertical,
        Orient::Vertical => Orient::Horizontal,
    }
}

impl Unit {
    /// Vega-Lite's `compositeMarkOrient`: the mark's `orient`, else the one
    /// continuous axis, else the axis the mark aggregates, else `y` but for
    /// a temporal `y` over a `x` that is not. `None` where neither is
    /// continuous or both aggregate.
    fn orient(&self) -> Option<Orient> {
        if let Some(orient) = self.def.get("orient") {
            return orient.as_str()?.parse().ok();
        }
        let x = self.encoding.get("x");
        let y = self.encoding.get("y");
        let aggregates = |def: Option<&Value>| {
            def.and_then(|def| def.get("aggregate"))
                .and_then(Value::as_str)
                == Some(self.composite.to_string().as_str())
        };
        match (continuous(x), continuous(y)) {
            (true, true) => match (aggregates(x), aggregates(y)) {
                (false, true) => Some(Orient::Vertical),
                (true, false) => Some(Orient::Horizontal),
                (true, true) => None,
                (false, false) if temporal(y) && !temporal(x) => Some(Orient::Horizontal),
                (false, false) => Some(Orient::Vertical),
            },
            (true, false) => Some(Orient::Horizontal),
            (false, true) => Some(Orient::Vertical),
            (false, false) => None,
        }
    }

    /// The encoding's channel `name`, without an `aggregate` Vega-Lite drops
    /// from the continuous axis.
    fn continuous_def(&self, name: &str) -> Option<Map<String, Value>> {
        Some(without(
            self.encoding.get(name)?.as_object()?,
            &["aggregate"],
        ))
    }

    /// Whether the part is drawn: as the mark's property says, or by default
    /// as `config`, the composite mark's config, says.
    fn draws(&self, part: &str, config: &Map<String, Value>) -> bool {
        match self.def.get(part) {
            Some(value) => format::js(value).truthy(),
            None => config
                .get(part)
                .is_some_and(|value| format::js(value).truthy()),
        }
    }

    /// Vega-Lite's `partLayerMixins`: a unit of `mark` drawn as `encoding`
    /// says, where the part is drawn, its mark the part's `config`
    /// properties under the composite mark's `clip`, `color` and `opacity`,
    /// then `mark`'s, then the part property's own.
    fn part(
        &self,
        part: &str,
        config: &Map<String, Value>,
        mark: Map<String, Value>,
        encoding: Map<String, Value>,
        transform: Vec<Step>,
    ) -> Option<View> {
        if !self.draws(part, config) {
            return None;
        }
        let mut out = match config.get(part) {
            Some(Value::Object(config)) => config.clone(),
            _ => Map::new(),
        };
        for key in ["clip", "color", "opacity"] {
            if let Some(value) = self.def.get(key).filter(|value| format::js(value).truthy()) {
                out.insert(key.into(), value.clone());
            }
        }
        out.extend(mark);
        out.insert("style".into(), format!("{}-{part}", self.composite).into());
        if let Some(Value::Object(own)) = self.def.get(part) {
            out.extend(own.clone());
        }
        Some(View {
            data: None,
            transform,
            body: Body::Unit {
                mark: Value::Object(out),
                encoding,
            },
        })
    }
}

/// Vega-Lite's `makeCompositeAggregatePartFactory`: parts that read the
/// continuous axis's field as an aggregate derives it.
struct Factory<'a> {
    unit: &'a Unit,
    config: &'a Map<String, Value>,
    axis: &'static str,
    /// The continuous axis's definition.
    continuous: &'a Map<String, Value>,
}

impl Factory<'_> {
    /// A part that reads `<prefix>_<field>` of the continuous axis's field,
    /// spanning to `<end>_<field>` where given, over `shared`.
    fn part(
        &self,
        part: &str,
        mark: Map<String, Value>,
        prefix: &str,
        end: Option<&str>,
        shared: &Map<String, Value>,
    ) -> Option<Option<View>> {
        let continuous = self.continuous;
        let field = continuous.get("field")?.as_str()?;
        let mut position = Map::new();
        position.insert("field".into(), format!("{prefix}_{field}").into());
        if let Some(kind) = continuous.get("type") {
            position.insert("type".into(), kind.clone());
        }
        position.insert("title".into(), title(continuous)?);
        for key in ["scale", "axis"] {
            if let Some(value) = continuous.get(key) {
                position.insert(key.into(), value.clone());
            }
        }
        let mut encoding = Map::new();
        encoding.insert(self.axis.into(), Value::Object(position));
        if let Some(end) = end {
            let mut position = Map::new();
            position.insert("field".into(), format!("{end}_{field}").into());
            encoding.insert(format!("{}2", self.axis), Value::Object(position));
        }
        encoding.extend(shared.clone());
        Some(
            self.unit
                .part(part, self.config, mark, encoding, Vec::new()),
        )
    }
}

/// Vega-Lite's `getTitle`: the channel's `title`, else its field.
fn title(def: &Map<String, Value>) -> Option<Value> {
    def.get("title").or(def.get("field")).cloned()
}

/// `encoding` with a `type` on each field definition but an end's, as
/// Vega-Lite's `normalizeEncoding` gives it. `None` for a definition whose
/// type this does not read.
fn typed(encoding: &Map<String, Value>) -> Option<Map<String, Value>> {
    let mut out = Map::new();
    for (name, def) in encoding {
        let mut def = def.clone();
        if let Value::Object(fields) = &mut def
            && !SECONDARY.contains(&name.as_str())
            && fields.get("type").is_none()
            && encoding::field_def(fields)
        {
            // An aggregate a composite mark names is quantitative.
            let composite = fields
                .get("aggregate")
                .and_then(Value::as_str)
                .is_some_and(|op| op.parse::<Composite>().is_ok());
            let kind = match composite {
                true => Kind::Quantitative,
                false => encoding::kind(fields, name, &Derive::read(fields, name)?, None)?,
            };
            fields.insert("type".into(), kind.to_string().into());
        }
        out.insert(name.clone(), def);
    }
    Some(out)
}

/// Vega-Lite's `isContinuousFieldOrDatumDef`: a typed field neither nominal,
/// ordinal nor binned, or a number datum.
fn continuous(def: Option<&Value>) -> bool {
    let Some(def) = def.and_then(Value::as_object) else {
        return false;
    };
    if def.get("datum").is_some_and(Value::is_number) {
        return true;
    }
    if !encoding::field_def(def) {
        return false;
    }
    match def.get("type").and_then(Value::as_str) {
        Some("temporal") => true,
        Some("quantitative") => !def
            .get("bin")
            .is_some_and(|value| format::js(value).truthy()),
        _ => false,
    }
}

/// Vega-Lite's `isFieldOrDatumDefForTimeFormat` without a `formatType`: a
/// temporal type, or a field with a time unit.
fn temporal(def: Option<&Value>) -> bool {
    let Some(def) = def.and_then(Value::as_object) else {
        return false;
    };
    def.get("type").and_then(Value::as_str) == Some("temporal")
        || (encoding::field_def(def)
            && def
                .get("timeUnit")
                .is_some_and(|value| format::js(value).truthy()))
}

/// Vega-Lite's `accessWithDatumToUnescapedPath`.
fn datum(field: &str) -> String {
    format!("datum['{}']", field.replace('\'', "\\'"))
}

/// Vega-Lite's `removePathFromField`.
fn flat(field: &str) -> Option<String> {
    Some(field.parse::<Path>().ok()?.flat())
}

/// A measure of `field` named `output`.
fn measure(op: Op, field: &str, output: String) -> Measure {
    Measure {
        op,
        field: Some(field.to_string().into()),
        param: None,
        output: output.into(),
    }
}

/// An aggregate step.
fn aggregate(measures: Vec<Measure>, groupby: &[SharedString]) -> Step {
    Step::Transform(Transform::Aggregate(Aggregate {
        groupby: groupby.to_vec(),
        measures,
    }))
}

/// `source` as a formula, reading the missing values of the fields
/// `aggregated` names as an aggregate leaves them.
fn formula(source: &str, aggregated: &[String]) -> Option<Formula> {
    let mut formula: Formula = source.parse().ok()?;
    formula.aggregated = aggregated.iter().map(|f| f.clone().into()).collect();
    Some(formula)
}

/// A filter step of `source`.
fn filter(source: &str, aggregated: &[String]) -> Option<Step> {
    Some(Step::Filter(formula(source, aggregated)?))
}

/// A calculate step of `source` named `output`.
fn calculate(source: &str, aggregated: &[String], output: String) -> Option<Step> {
    Some(Step::Calculate(formula(source, aggregated)?, output.into()))
}

/// What Vega-Lite's `extractTransformsFromEncoding` makes of an encoding:
/// the bins, time units and aggregate its field definitions derive, the
/// fields the aggregate groups by, and the encoding of what they derive.
struct Extract {
    bins: Vec<Bin>,
    time_units: Vec<TimeUnit>,
    measures: Vec<Measure>,
    groupby: Vec<SharedString>,
    encoding: Map<String, Value>,
}

impl Extract {
    /// `None` for a derivation this does not read.
    fn of(encoding: &Map<String, Value>) -> Option<Self> {
        let mut out = Self {
            bins: Vec::new(),
            time_units: Vec::new(),
            measures: Vec::new(),
            groupby: Vec::new(),
            encoding: Map::new(),
        };
        for (channel, def) in encoding {
            let defs: Vec<&Value> = match def {
                Value::Array(defs) => defs.iter().collect(),
                def => vec![def],
            };
            for def in defs {
                out.channel(channel, def)?;
            }
        }
        Some(out)
    }

    fn channel(&mut self, channel: &str, def: &Value) -> Option<()> {
        let Some(fields) = def.as_object().filter(|def| encoding::field_def(def)) else {
            self.encoding.insert(channel.into(), def.clone());
            return Some(());
        };
        let field = fields.get("field").and_then(Value::as_str);
        let derives = ["aggregate", "bin", "timeUnit"].iter().any(|key| {
            fields
                .get(*key)
                .is_some_and(|value| format::js(value).truthy())
        });
        if !derives {
            self.groupby.push(field?.to_string().into());
            self.encoding.insert(channel.into(), def.clone());
            return Some(());
        }
        let remaining = without(fields, &["field", "aggregate", "bin", "timeUnit"]);
        let derive = Derive::read(fields, channel)?;
        let named = field.map(|field| SharedString::from(field.to_string()));
        let mut out = Map::new();
        let guide = ["axis", "legend"]
            .into_iter()
            .filter_map(|key| fields.get(key)?.as_object()?.get("title"))
            .next();
        if !guide.is_some_and(|value| format::js(value).truthy()) {
            let title = match guide.or(fields.get("title")) {
                Some(title) => title.clone(),
                None => derive.title(named.as_ref())?.to_string().into(),
            };
            out.insert("title".into(), title);
        }
        out.extend(remaining);
        let name = derive.name(named.as_ref())?;
        match derive {
            Derive::Aggregate {
                op,
                measured,
                param,
                output,
            } => {
                self.measures.push(Measure {
                    op,
                    field: measured,
                    param,
                    output,
                });
            }
            Derive::Bin(bin) => {
                self.groupby.push(name.clone());
                self.groupby.push(bin.output[1].clone());
                let kind = fields.get("type").and_then(Value::as_str);
                if matches!(kind, Some("ordinal" | "nominal")) && scaled(channel) {
                    self.groupby.push(format!("{name}_range").into());
                }
                if matches!(channel, "x" | "y") {
                    let mut end = Map::new();
                    end.insert("field".into(), bin.output[1].to_string().into());
                    self.encoding
                        .insert(format!("{channel}2"), Value::Object(end));
                }
                out.insert("bin".into(), "binned".into());
                if !SECONDARY.contains(&channel) {
                    out.insert("type".into(), Kind::Quantitative.to_string().into());
                }
                self.bins.push(bin);
            }
            Derive::TimeUnit(unit) => {
                self.groupby.push(name.clone());
                self.time_units.push(unit);
            }
            Derive::BinnedTimeUnit { .. } | Derive::Binned(_) => {
                self.groupby.push(name.clone());
            }
            Derive::None => return None,
        }
        out.insert("field".into(), name.to_string().into());
        self.encoding.insert(channel.into(), Value::Object(out));
        Some(())
    }

    /// The bins and time units, as steps.
    fn steps(&self) -> Vec<Step> {
        self.bins
            .iter()
            .cloned()
            .map(|bin| Step::Transform(Transform::Bin(bin)))
            .chain(
                self.time_units
                    .iter()
                    .cloned()
                    .map(|unit| Step::Transform(Transform::TimeUnit(unit))),
            )
            .collect()
    }
}

/// Whether the channel has a scale.
fn scaled(channel: &str) -> bool {
    !matches!(
        channel,
        "detail" | "order" | "tooltip" | "href" | "text" | "key" | "description" | "url"
    )
}

/// Vega-Lite's `filterTooltipWithAggregatedField`: `encoding` without its
/// tooltip but the aggregated fields of it.
fn without_plain_tooltip(encoding: &Map<String, Value>) -> Map<String, Value> {
    let mut out = without(encoding, &["tooltip"]);
    let Some(tooltip) = encoding.get("tooltip").cloned() else {
        return out;
    };
    let aggregated = |def: &Value| {
        def.get("aggregate")
            .is_some_and(|value| format::js(value).truthy())
    };
    match tooltip {
        Value::Array(defs) => {
            let kept: Vec<Value> = defs.into_iter().filter(aggregated).collect();
            if !kept.is_empty() {
                out.insert("tooltip".into(), Value::Array(kept));
            }
        }
        tooltip if aggregated(&tooltip) => {
            out.insert("tooltip".into(), tooltip);
        }
        _ => {}
    }
    out
}

/// `map` without `keys`, in its order.
fn without(map: &Map<String, Value>, keys: &[&str]) -> Map<String, Value> {
    map.iter()
        .filter(|(key, _)| !keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}
