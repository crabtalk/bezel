//! vega-transforms' `Impute` and `Pivot`.

use crate::{
    Error,
    spec::{Impute, Pivot},
    transform::{
        Accessor, Get, Tuple,
        aggregate::{Aggregation, Op},
        or,
    },
    value::Value,
    view::View,
};
use d3::array;
use std::{cmp::Ordering, collections::HashMap};

/// A tuple for each key of the domain a group has none at: the group's
/// values, the key, and the field imputed, after the tuples as they were.
pub fn impute(view: &View, def: &Impute, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let field = Accessor::new(view, &def.field)?;
    let key = Accessor::new(view, &def.key)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let method = or(view, &def.method, "value".to_string())?;
    let value = match &def.value {
        Some(value) => view.param_value(value)?,
        None => Value::Number(0.0),
    };
    if !["value", "mean", "median", "max", "min"].contains(&method.as_str()) {
        return Err(Error::Spec(format!(
            "Unrecognized imputation method: {method}"
        )));
    }
    // `partition`: the key domain, the keyvals first, and each group's tuple
    // at each key of it, the last tuple with a key winning.
    let mut domain: Vec<Value> = match &def.keyvals {
        Some(keyvals) => match view.param_value(keyvals)? {
            Value::Array(items) => items.to_vec(),
            other => vec![other],
        },
        None => Vec::new(),
    };
    let mut slots: HashMap<String, usize> = HashMap::new();
    for (i, k) in domain.iter().enumerate() {
        slots.entry(k.to_string()).or_insert(i);
    }
    let mut groups: Vec<(Vec<Value>, Vec<Option<usize>>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (i, t) in tuples.iter().enumerate() {
        let k = key.get(view, &t.datum)?;
        let j = *slots.entry(k.to_string()).or_insert_with(|| {
            domain.push(k.clone());
            domain.len() - 1
        });
        let values = groupby
            .iter()
            .map(|f| f.get(view, &t.datum))
            .collect::<Result<Vec<_>, _>>()?;
        let g = *index
            .entry(Value::from(values.clone()).to_string())
            .or_insert_with(|| {
                groups.push((values, Vec::new()));
                groups.len() - 1
            });
        let slots = &mut groups[g].1;
        if slots.len() <= j {
            slots.resize(j + 1, None);
        }
        slots[j] = Some(i);
    }
    let names: Vec<&str> = groupby.iter().map(|f| f.name.as_str()).collect();
    let mut imputed = Vec::new();
    for (values, slots) in &groups {
        let mut fill: Option<Value> = None;
        for (j, k) in domain.iter().enumerate() {
            if slots.get(j).copied().flatten().is_some() {
                continue;
            }
            let v = match &fill {
                Some(v) if !matches!(v, Value::Number(n) if n.is_nan()) => v.clone(),
                _ => {
                    let present = slots
                        .iter()
                        .flatten()
                        .map(|&i| field.get(view, &tuples[i].datum))
                        .collect::<Result<Vec<_>, _>>()?;
                    let v = imputed_value(&method, &value, &present);
                    fill = Some(v.clone());
                    v
                }
            };
            let mut datum = Value::Object(vec![("_impute".into(), Value::Bool(true))].into());
            for (name, v) in names.iter().zip(values) {
                datum = datum.with_field(name, v.clone());
            }
            datum = datum
                .with_field(&key.name, k.clone())
                .with_field(&field.name, v);
            imputed.push(view.ingest(datum));
        }
    }
    tuples.extend(imputed);
    Ok(tuples)
}

/// The imputed value: `value`, or d3-array's `mean`, `median`, `min` or
/// `max` of the group's values.
fn imputed_value(method: &str, value: &Value, values: &[Value]) -> Value {
    let numbers = || -> Vec<f64> {
        values
            .iter()
            .filter(|v| !v.is_nullish())
            .map(Value::to_number)
            .filter(|v| !v.is_nan())
            .collect()
    };
    match method {
        "value" => value.clone(),
        "mean" => {
            let numbers = numbers();
            match numbers.is_empty() {
                true => Value::Undefined,
                false => Value::Number(numbers.iter().sum::<f64>() / numbers.len() as f64),
            }
        }
        "median" => {
            let mut numbers = numbers();
            numbers.sort_by(f64::total_cmp);
            match numbers.is_empty() {
                true => Value::Undefined,
                false => Value::Number(array::quantile_sorted(&numbers, 0.5)),
            }
        }
        _ => {
            // d3's `min` and `max` compare the values as they are.
            let want = if method == "min" {
                Ordering::Greater
            } else {
                Ordering::Less
            };
            let mut best = Value::Undefined;
            for v in values {
                if v.is_nullish() {
                    continue;
                }
                let replace = match &best {
                    Value::Undefined => v.compare(v).is_some(),
                    b => b.compare(v) == Some(want),
                };
                if replace {
                    best = v.clone();
                }
            }
            best
        }
    }
}

/// `pivot`: an aggregate of `value` for each distinct `field` value,
/// ascending, at most `limit` of them.
pub fn pivot(view: &View, def: &Pivot, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let key = Accessor::new(view, &def.field)?;
    let value = Accessor::new(view, &def.value)?;
    let op = match or(view, &def.op, "sum".to_string())?.as_str() {
        "count" => Op::CountValues,
        op => Op::parse(op)?,
    };
    let limit = or(view, &def.limit, 0.0)?;
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut keys: Vec<Value> = Vec::new();
    for t in tuples {
        let k = key.get(view, &t.datum)?;
        if seen.insert(k.to_string(), ()).is_none() {
            keys.push(k);
        }
    }
    d3::array::sort_by(&mut keys, Value::ascending);
    if limit > 0.0 {
        keys.truncate(limit as usize);
    }
    let fields = keys
        .iter()
        .map(|k| {
            Some(Accessor {
                get: Get::Pivot {
                    key: Box::new(key.clone()),
                    value: Box::new(value.clone()),
                    k: k.clone(),
                },
                name: k.to_string(),
            })
        })
        .collect();
    let ops = vec![op; keys.len()];
    let as_: Vec<Option<String>> = keys.iter().map(|k| Some(k.to_string())).collect();
    let dims = Accessor::many(view, &def.groupby)?;
    let cell_key = def
        .key
        .as_ref()
        .map(|k| Accessor::new(view, k))
        .transpose()?;
    let aggregation = Aggregation::of(dims, cell_key, fields, &ops, &[], &as_)?;
    aggregation.run(view, tuples, false)
}
