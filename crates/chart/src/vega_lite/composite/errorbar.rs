//! Vega-Lite's `normalizeErrorBar` and the `errorBarParams` it shares with
//! error bands: each group's `extent` about its center from raw values
//! (`ci`, `iqr`, `stderr` or `stdev`), or bounds a row gives already, as
//! `x2` or `y2` or as an error, `xError` or `yError`, from the value, with
//! `xError2` or `yError2` the lower one.

use crate::{
    model::Orient,
    transform::Op,
    vega_lite::{
        composite,
        composite::{Extract, Factory, Unit},
        dataflow::Step,
        encoding, format,
        view::{Body, View},
    },
};
use gpui::SharedString;
use serde_json::{Map, Value, json};

/// Where an error bar's bounds come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Input {
    /// Each group's values.
    Raw,
    /// Each row's lower bound and upper one, `x` and `x2` or `y` and `y2`.
    UpperLower,
    /// Each row's value and its error.
    Error,
}

/// What an error bar and an error band share: the transforms to their
/// bounds, the continuous axis and its definition, and the encoding of the
/// other channels.
pub(crate) struct Params {
    pub(crate) transform: Vec<Step>,
    pub(crate) axis: &'static str,
    pub(crate) continuous: Map<String, Value>,
    pub(crate) encoding: Map<String, Value>,
    pub(crate) orient: Orient,
}

impl Unit {
    pub(crate) fn errorbar(&self) -> Option<View> {
        let Value::Object(config) = json!({"center": "mean", "rule": true, "ticks": false}) else {
            unreachable!("an object")
        };
        let params = self.error_params(&config)?;
        let shared = composite::without(&params.encoding, &["size"]);
        let parts = Factory {
            unit: self,
            config: &config,
            axis: params.axis,
            continuous: &params.continuous,
        };
        let thickness = self.def.get("thickness");
        let mut tick = Map::new();
        tick.insert("type".into(), "tick".into());
        tick.insert(
            "orient".into(),
            composite::across(params.orient).to_string().into(),
        );
        tick.insert("aria".into(), false.into());
        if let Some(thickness) = thickness {
            tick.insert("thickness".into(), thickness.clone());
        }
        if let Some(size) = self.def.get("size") {
            tick.insert("size".into(), size.clone());
        }
        let mut rule = Map::new();
        rule.insert("type".into(), "rule".into());
        rule.insert("ariaRoleDescription".into(), "errorbar".into());
        if let Some(thickness) = thickness {
            rule.insert("size".into(), thickness.clone());
        }
        let mut layers: Vec<View> = [
            parts.part("ticks", tick.clone(), "lower", None, &shared)?,
            parts.part("ticks", tick, "upper", None, &shared)?,
            parts.part("rule", rule, "lower", Some("upper"), &shared)?,
        ]
        .into_iter()
        .flatten()
        .collect();
        let body = match layers.len() {
            0 => return None,
            1 => layers.remove(0).body,
            _ => Body::Layer(layers),
        };
        Some(View {
            data: self.data.clone(),
            transform: params.transform,
            body,
        })
    }

    /// Vega-Lite's `errorBarParams` under the composite mark's `config`.
    /// `None` where Vega-Lite throws.
    pub(crate) fn error_params(&self, config: &Map<String, Value>) -> Option<Params> {
        let (orient, input) = self.error_orient()?;
        let axis = composite::axis(orient);
        let continuous = self.continuous_def(axis)?;
        let field = continuous.get("field")?.as_str()?.to_string();
        let [end, error, error2] = match orient {
            Orient::Horizontal => ["x2", "xError", "xError2"],
            Orient::Vertical => ["y2", "yError", "yError2"],
        };
        let def_field = |name: &str| -> Option<String> {
            Some(
                self.continuous_def(name)?
                    .get("field")?
                    .as_str()?
                    .to_string(),
            )
        };
        let mut specific = Vec::new();
        let mut calculates = Vec::new();
        let value = composite::datum(&field);
        match input {
            Input::Raw => {
                let named = |key: &str| self.def.get(key).filter(|v| format::js(v).truthy());
                let center = match (named("center"), named("extent")) {
                    (Some(center), _) => center.as_str()?,
                    (None, Some(extent)) if extent.as_str() == Some("iqr") => "median",
                    (None, Some(_)) => "mean",
                    (None, None) => config["center"].as_str()?,
                };
                let extent = match named("extent") {
                    Some(extent) => extent.as_str()?,
                    None if center == "mean" => "stderr",
                    None => "iqr",
                };
                let center_op: Op = center.parse().ok()?;
                if matches!(extent, "stderr" | "stdev") {
                    let extent_op: Op = extent.parse().ok()?;
                    specific.push(composite::measure(
                        extent_op,
                        &field,
                        format!("extent_{field}"),
                    ));
                    specific.push(composite::measure(
                        center_op,
                        &field,
                        format!("center_{field}"),
                    ));
                    let aggregated = [format!("center_{field}"), format!("extent_{field}")];
                    let [center, extent] = aggregated.clone().map(|field| composite::datum(&field));
                    calculates.push(composite::calculate(
                        &format!("{center} + {extent}"),
                        &aggregated,
                        format!("upper_{field}"),
                    )?);
                    calculates.push(composite::calculate(
                        &format!("{center} - {extent}"),
                        &aggregated,
                        format!("lower_{field}"),
                    )?);
                } else {
                    let (center, lower, upper) = match extent {
                        "ci" => (Op::Mean, Op::Ci0, Op::Ci1),
                        _ => (Op::Median, Op::Q1, Op::Q3),
                    };
                    specific.push(composite::measure(lower, &field, format!("lower_{field}")));
                    specific.push(composite::measure(upper, &field, format!("upper_{field}")));
                    specific.push(composite::measure(
                        center,
                        &field,
                        format!("center_{field}"),
                    ));
                }
            }
            Input::UpperLower => {
                let upper = composite::datum(&def_field(end)?);
                calculates.push(composite::calculate(&upper, &[], format!("upper_{field}"))?);
                calculates.push(composite::calculate(&value, &[], format!("lower_{field}"))?);
            }
            Input::Error => {
                let error = composite::datum(&def_field(error)?);
                calculates.push(composite::calculate(
                    &format!("{value} + {error}"),
                    &[],
                    format!("upper_{field}"),
                )?);
                let lower = match encoding::field_or_datum(self.encoding.get(error2)) {
                    true => format!("{value} + {}", composite::datum(&def_field(error2)?)),
                    false => format!("{value} - {error}"),
                };
                calculates.push(composite::calculate(&lower, &[], format!("lower_{field}"))?);
            }
        }
        let rest = composite::without(&self.encoding, &[axis, end, error, error2]);
        let extract = Extract::of(&rest)?;
        let mut measures = extract.measures.clone();
        measures.extend(specific);
        let groupby: Vec<SharedString> = match input {
            Input::Raw => extract.groupby.clone(),
            _ => Vec::new(),
        };
        let mut transform = self.transform.clone();
        transform.extend(extract.steps());
        if !measures.is_empty() {
            transform.push(composite::aggregate(measures, &groupby));
        }
        transform.extend(calculates);
        Some(Params {
            transform,
            axis,
            continuous,
            encoding: composite::without(&extract.encoding, &["tooltip"]),
            orient,
        })
    }

    /// Vega-Lite's `errorBarOrientAndInputType`. `None` where it throws.
    fn error_orient(&self) -> Option<(Orient, Input)> {
        let has = |name: &str| encoding::field_or_datum(self.encoding.get(name));
        let continuous = |name: &str| composite::continuous(self.encoding.get(name));
        let errors = ["xError", "xError2", "yError", "yError2"];
        if (has("x") || has("y")) && !has("x2") && !has("y2") && !errors.iter().any(|e| has(e)) {
            return Some((self.orient()?, Input::Raw));
        }
        if has("x2") || has("y2") {
            if errors.iter().any(|e| has(e)) {
                return None;
            }
            return match (has("x2"), has("y2")) {
                (true, true) => None,
                (true, false) => continuous("x").then_some((Orient::Horizontal, Input::UpperLower)),
                _ => continuous("y").then_some((Orient::Vertical, Input::UpperLower)),
            };
        }
        if has("xError2") && !has("xError") || has("yError2") && !has("yError") {
            return None;
        }
        match (has("xError"), has("yError")) {
            (true, true) => None,
            (true, false) => continuous("x").then_some((Orient::Horizontal, Input::Error)),
            (false, true) => continuous("y").then_some((Orient::Vertical, Input::Error)),
            (false, false) => None,
        }
    }
}
