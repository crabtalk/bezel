//! A spec's `transform` list as a pipeline: `aggregate`, `bin`, `density`,
//! `loess`, `quantile`, `regression` and `timeUnit`, each with Vega-Lite's
//! defaults and output names.

use crate::{
    transform::{
        Aggregate, Density, Loess, Measure, Method, Op, Quantile, Regression, Resolve, Transform,
    },
    vega_lite::{field, field::Derive},
};
use gpui::SharedString;
use serde_json::{Map, Value};

/// `None` if an op is one the pipeline lacks, or carries a key its op does
/// not take.
pub(crate) fn pipeline(list: &Value) -> Option<Vec<Transform>> {
    list.as_array()?.iter().map(op).collect()
}

fn op(value: &Value) -> Option<Transform> {
    let op = value.as_object()?;
    let takes = |keys: &[&str]| op.keys().all(|key| keys.contains(&key.as_str()));
    if op.contains_key("density") {
        takes(&[
            "density",
            "groupby",
            "cumulative",
            "counts",
            "bandwidth",
            "extent",
            "minsteps",
            "maxsteps",
            "steps",
            "as",
            "resolve",
        ])
        .then(|| density(op))?
    } else if op.contains_key("loess") {
        takes(&["loess", "on", "groupby", "bandwidth", "as"]).then(|| loess(op))?
    } else if op.contains_key("regression") {
        takes(&[
            "regression",
            "on",
            "groupby",
            "method",
            "order",
            "extent",
            "params",
            "as",
        ])
        .then(|| regression(op))?
    } else if op.contains_key("quantile") {
        takes(&["quantile", "groupby", "probs", "step", "as"]).then(|| quantile(op))?
    } else if op.contains_key("aggregate") {
        takes(&["aggregate", "groupby"]).then(|| aggregate(op))?
    } else if op.contains_key("bin") {
        takes(&["bin", "field", "as"]).then(|| bin(op))?
    } else if op.contains_key("timeUnit") {
        takes(&["timeUnit", "field", "as"]).then(|| time_unit(op))?
    } else {
        None
    }
}

fn density(op: &Map<String, Value>) -> Option<Transform> {
    let mut out = Density::new(field(op.get("density")?)?);
    out.groupby = groupby(op)?;
    out.cumulative = flag(op, "cumulative")?;
    out.counts = flag(op, "counts")?;
    if let Some(bandwidth) = op.get("bandwidth") {
        out.bandwidth = Some(bandwidth.as_f64()?);
    }
    out.extent = extent(op)?;
    if let Some(steps) = op.get("steps") {
        out.steps = Some(count(steps)?);
    }
    if let Some(steps) = op.get("minsteps") {
        out.minsteps = count(steps)?;
    }
    if let Some(steps) = op.get("maxsteps") {
        out.maxsteps = count(steps)?;
    }
    out.resolve = match op.get("resolve").map(Value::as_str) {
        None => Resolve::Shared,
        Some(Some("shared")) => Resolve::Shared,
        Some(Some("independent")) => Resolve::Independent,
        Some(_) => return None,
    };
    out.output = output(op, out.output)?;
    Some(Transform::Density(out))
}

fn loess(op: &Map<String, Value>) -> Option<Transform> {
    let mut out = Loess::new(field(op.get("loess")?)?, field(op.get("on")?)?);
    out.groupby = groupby(op)?;
    if let Some(bandwidth) = op.get("bandwidth") {
        out.bandwidth = bandwidth.as_f64()?;
    }
    out.output = output(op, out.output)?;
    Some(Transform::Loess(out))
}

fn regression(op: &Map<String, Value>) -> Option<Transform> {
    let mut out = Regression::new(field(op.get("regression")?)?, field(op.get("on")?)?);
    out.groupby = groupby(op)?;
    out.params = flag(op, "params")?;
    let order = match op.get("order") {
        None => 3,
        Some(order) => count(order)?,
    };
    out.method = match op.get("method").map(Value::as_str) {
        None | Some(Some("linear")) => Method::Linear,
        Some(Some("log")) => Method::Log,
        Some(Some("exp")) => Method::Exp,
        Some(Some("pow")) => Method::Pow,
        Some(Some("quad")) => Method::Quad,
        Some(Some("poly")) => Method::Poly(order),
        Some(_) => return None,
    };
    out.extent = extent(op)?;
    out.output = output(op, out.output)?;
    Some(Transform::Regression(out))
}

fn quantile(op: &Map<String, Value>) -> Option<Transform> {
    let mut out = Quantile::new(field(op.get("quantile")?)?);
    out.groupby = groupby(op)?;
    out.probs = match op.get("probs") {
        None => None,
        Some(probs) => Some(
            probs
                .as_array()?
                .iter()
                .map(Value::as_f64)
                .collect::<Option<_>>()?,
        ),
    };
    if let Some(step) = op.get("step") {
        out.step = step.as_f64()?;
    }
    out.output = output(op, out.output)?;
    Some(Transform::Quantile(out))
}

/// Its measures grouped by field, each field's in the order met, as
/// Vega-Lite assembles them; of its counts, the last.
fn aggregate(op: &Map<String, Value>) -> Option<Transform> {
    let mut measured: Vec<(Option<SharedString>, Vec<Measure>)> = Vec::new();
    for def in op.get("aggregate")?.as_array()? {
        let def = def.as_object()?;
        if def
            .keys()
            .any(|key| !["op", "field", "as"].contains(&key.as_str()))
        {
            return None;
        }
        let written = match def.get("field") {
            None => None,
            Some(name) => Some(name.as_str()?),
        };
        let Derive::Aggregate {
            op,
            measured: field,
            param,
            output,
        } = field::aggregate_def(def.get("op")?, written)?
        else {
            return None;
        };
        let output = match (def.get("as"), op, written) {
            (Some(name), ..) => self::field(name)?,
            (None, Op::Count, Some(written)) => format!("count_{written}").into(),
            (None, Op::Count, None) => "count".into(),
            (None, ..) => output,
        };
        let key = field.clone();
        let slot = match measured.iter_mut().find(|(f, _)| *f == key) {
            Some((_, measures)) => measures,
            None => {
                measured.push((key, Vec::new()));
                &mut measured.last_mut().expect("just pushed").1
            }
        };
        // A later count names the one count.
        match slot
            .iter_mut()
            .find(|m| op == Op::Count && m.op == Op::Count)
        {
            Some(count) => count.output = output,
            None => slot.push(Measure {
                op,
                field,
                param,
                output,
            }),
        }
    }
    Some(Transform::Aggregate(Aggregate {
        groupby: groupby(op)?,
        measures: measured.into_iter().flat_map(|(_, m)| m).collect(),
    }))
}

/// `as` a name, its end `<name>_end`, or the two names.
fn bin(op: &Map<String, Value>) -> Option<Transform> {
    let name = op.get("field")?.as_str()?;
    let Derive::Bin(mut out) = field::bin_def(op.get("bin")?, name, "")? else {
        return None;
    };
    out.output = names(op.get("as")?)?;
    Some(Transform::Bin(out))
}

fn time_unit(op: &Map<String, Value>) -> Option<Transform> {
    let name = op.get("field")?.as_str()?;
    let Derive::TimeUnit(mut out) = field::unit_def(op.get("timeUnit")?, name)? else {
        return None;
    };
    out.output = names(op.get("as")?)?;
    Some(Transform::TimeUnit(out))
}

/// A start's name and `<name>_end`, or both names.
fn names(value: &Value) -> Option<[SharedString; 2]> {
    match value {
        Value::String(name) => Some([name.clone().into(), format!("{name}_end").into()]),
        Value::Array(names) => match names.as_slice() {
            [start, end] => Some([field(start)?, field(end)?]),
            _ => None,
        },
        _ => None,
    }
}

fn field(value: &Value) -> Option<SharedString> {
    Some(value.as_str()?.to_string().into())
}

fn groupby(op: &Map<String, Value>) -> Option<Vec<SharedString>> {
    match op.get("groupby") {
        None => Some(Vec::new()),
        Some(fields) => fields.as_array()?.iter().map(field).collect(),
    }
}

/// An absent flag is `false`.
fn flag(op: &Map<String, Value>, key: &str) -> Option<bool> {
    match op.get(key) {
        None => Some(false),
        Some(value) => value.as_bool(),
    }
}

/// A whole number of steps.
fn count(value: &Value) -> Option<usize> {
    usize::try_from(value.as_u64()?).ok()
}

/// `Some(None)` for no `extent`.
fn extent(op: &Map<String, Value>) -> Option<Option<[f64; 2]>> {
    let Some(extent) = op.get("extent") else {
        return Some(None);
    };
    let [low, high] = extent.as_array()?.as_slice() else {
        return None;
    };
    Some(Some([low.as_f64()?, high.as_f64()?]))
}

/// `as`, each of its two names falling back to the op's default.
fn output(op: &Map<String, Value>, defaults: [SharedString; 2]) -> Option<[SharedString; 2]> {
    let Some(names) = op.get("as") else {
        return Some(defaults);
    };
    let names = names.as_array()?;
    if names.len() > 2 {
        return None;
    }
    let mut out = defaults;
    for (slot, name) in out.iter_mut().zip(names) {
        match name {
            Value::Null => {}
            name => *slot = field(name)?,
        }
    }
    Some(out)
}
