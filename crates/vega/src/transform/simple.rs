//! The vega-transforms that take each tuple on its own: filter, formula,
//! extent, collect, fold, flatten, project, identifier, sequence, cross,
//! sample and lookup.

use crate::{
    Error,
    spec::{
        Collect, Cross, Extent, Filter, Flatten, Fold, Formula, Identifier, Lookup, Project,
        Sample, Sequence,
    },
    transform::{Accessor, Comparator, Tuple, or},
    value::Value,
    view::View,
};
use std::collections::HashMap;

pub fn filter(view: &View, def: &Filter, tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let expr = view.parse(&def.expr)?;
    let mut kept = Vec::with_capacity(tuples.len());
    for t in tuples {
        if view.eval(&expr, &t.datum)?.truthy() {
            kept.push(t);
        }
    }
    Ok(kept)
}

pub fn formula(view: &View, def: &Formula, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let expr = view.parse(&def.expr)?;
    for t in &mut tuples {
        let value = view.eval(&expr, &t.datum)?;
        t.datum = t.datum.with_field(&def.as_, value);
    }
    Ok(tuples)
}

/// `[min, max]` of the field read as numbers, null and `""` skipped;
/// `[undefined, undefined]` where either is not finite.
pub fn extent(view: &View, def: &Extent, tuples: &[Tuple]) -> Result<Value, Error> {
    let field = Accessor::new(view, &def.field)?;
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for t in tuples {
        let v = field.get(view, &t.datum)?;
        if v.is_nullish() || matches!(&v, Value::String(s) if s.is_empty()) {
            continue;
        }
        let v = v.to_number();
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    Ok(if min.is_finite() && max.is_finite() {
        vec![Value::Number(min), Value::Number(max)].into()
    } else {
        vec![Value::Undefined, Value::Undefined].into()
    })
}

pub fn collect(view: &View, def: &Collect, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    if let Some(sort) = def
        .sort
        .as_ref()
        .map(Comparator::new)
        .transpose()?
        .flatten()
    {
        sort.sort(view, &mut tuples)?;
    }
    Ok(tuples)
}

pub fn fold(view: &View, def: &Fold, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let fields = Accessor::many(view, &def.fields)?;
    let [k, v] = or(view, &def.as_, ["key".into(), "value".into()])?;
    let mut out = Vec::with_capacity(tuples.len() * fields.len());
    for t in tuples {
        for field in &fields {
            let value = field.get(view, &t.datum)?;
            let datum = t
                .datum
                .with_field(&k, field.name.as_str().into())
                .with_field(&v, value);
            out.push(view.ingest(datum));
        }
    }
    Ok(out)
}

pub fn flatten(view: &View, def: &Flatten, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let fields = Accessor::many(view, &def.fields)?;
    let as_ = or(view, &def.as_, Vec::new())?;
    let names: Vec<&str> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| as_.get(i).map_or(f.name.as_str(), String::as_str))
        .collect();
    let index = def.index.as_ref().map(|i| view.param(i)).transpose()?;
    let mut out = Vec::new();
    for t in tuples {
        let arrays = fields
            .iter()
            .map(|f| f.get(view, &t.datum))
            .collect::<Result<Vec<_>, _>>()?;
        let len = arrays
            .iter()
            .map(|a| a.get(&"length".into()).map_or(0.0, |l| l.to_number()))
            .fold(0.0, f64::max);
        for i in 0..len as usize {
            let mut datum = t.datum.clone();
            for (name, array) in names.iter().zip(&arrays) {
                let v = array
                    .get(&Value::Number(i as f64))
                    .map_err(|e| Error::Eval(e.into()))?;
                datum = datum.with_field(name, if v.is_nullish() { Value::Null } else { v });
            }
            if let Some(index) = &index {
                datum = datum.with_field(index, Value::Number(i as f64));
            }
            out.push(view.ingest(datum));
        }
    }
    Ok(out)
}

pub fn project(view: &View, def: &Project, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let fields = Accessor::many(view, &def.fields)?;
    let as_ = or(view, &def.as_, Vec::new())?;
    let mut out = Vec::with_capacity(tuples.len());
    for t in tuples {
        let datum = if fields.is_empty() {
            t.datum.clone()
        } else {
            let mut datum = Value::Object(Vec::new().into());
            for (i, field) in fields.iter().enumerate() {
                let name = as_.get(i).cloned().flatten().unwrap_or(field.name.clone());
                datum = datum.with_field(&name, field.get(view, &t.datum)?);
            }
            datum
        };
        out.push(view.ingest(datum));
    }
    Ok(out)
}

/// Each tuple without the field, or with a falsy one, numbered after the
/// last number the view gave out.
pub fn identifier(view: &View, def: &Identifier, mut tuples: Vec<Tuple>) -> Vec<Tuple> {
    for t in &mut tuples {
        let current = t.datum.get(&def.as_.as_str().into()).unwrap_or_default();
        if !current.truthy() {
            t.datum = t
                .datum
                .with_field(&def.as_, Value::Number(view.next_identifier()));
        }
    }
    tuples
}

/// d3-array's `range(start, stop, step)` as tuples, after the input's.
pub fn sequence(view: &View, def: &Sequence, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let start = view.param(&def.start)?;
    let stop = view.param(&def.stop)?;
    let step = or(view, &def.step, 1.0)?;
    let step = if step == 0.0 || step.is_nan() {
        1.0
    } else {
        step
    };
    let as_ = or(view, &def.as_, "data".to_string())?;
    let n = ((stop - start) / step).ceil().max(0.0);
    let n = if n.is_nan() { 0.0 } else { n };
    for i in 0..n as usize {
        let datum = Value::Object(Vec::new().into())
            .with_field(&as_, Value::Number(start + i as f64 * step));
        tuples.push(view.ingest(datum));
    }
    Ok(tuples)
}

/// Every pair of tuples, `[a, b]`, that `filter` keeps.
pub fn cross(view: &View, def: &Cross, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let [a, b] = or(view, &def.as_, ["a".into(), "b".into()])?;
    let filter = def.filter.as_deref().map(|f| view.parse(f)).transpose()?;
    let mut out = Vec::new();
    for left in tuples {
        for right in tuples {
            let datum = Value::Object(Vec::new().into())
                .with_field(&a, left.datum.clone())
                .with_field(&b, right.datum.clone());
            let keep = match &filter {
                Some(filter) => view.eval(filter, &datum)?.truthy(),
                None => true,
            };
            if keep {
                out.push(view.ingest(datum));
            }
        }
    }
    Ok(out)
}

/// A reservoir sample of `size` tuples, drawn from the view's random
/// numbers.
pub fn sample(view: &View, def: &Sample, tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let size = or(view, &def.size, 1000.0)?;
    let mut reservoir: Vec<Tuple> = Vec::new();
    let mut count = 0.0;
    for t in tuples {
        if (reservoir.len() as f64) < size {
            reservoir.push(t);
        } else {
            let index = ((count + 1.0) * view.random()).trunc();
            if index < reservoir.len() as f64 && index >= 0.0 {
                reservoir[index as usize] = t;
            }
        }
        count += 1.0;
    }
    Ok(reservoir)
}

/// Each tuple's `fields` looked up in `from` by its `key`: the tuple found
/// or its `values`, `default` where none is.
pub fn lookup(view: &View, def: &Lookup, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let key = Accessor::new(view, &def.key)?;
    let keys = Accessor::many(view, &def.fields)?;
    let values = Accessor::many(view, &def.values)?;
    let default = match def.default.as_ref().map(|d| view.param(d)).transpose()? {
        Some(d) if !d.is_null() => Value::from(&d),
        _ => Value::Null,
    };
    let mut index: HashMap<String, Value> = HashMap::new();
    for t in view.data(&def.from)? {
        index.insert(key.get(view, &t.datum)?.to_string(), t.datum.clone());
    }
    let as_ = def.as_.as_ref().map(|a| view.param(a)).transpose()?;
    let n = keys.len();
    let names: Vec<String> = if values.is_empty() {
        as_.ok_or_else(|| Error::Spec("Missing output field names.".into()))?
    } else {
        if n > 1 && as_.is_none() {
            return Err(Error::Spec(
                "Multi-field lookup requires explicit \"as\" parameter.".into(),
            ));
        }
        match as_ {
            Some(as_) if as_.len() != n * values.len() => {
                return Err(Error::Spec(
                    "The \"as\" parameter has too few output field names.".into(),
                ));
            }
            Some(as_) => as_,
            None => values.iter().map(|v| v.name.clone()).collect(),
        }
    };
    for t in &mut tuples {
        let mut k = 0;
        for field in &keys {
            let found = index.get(&field.get(view, &t.datum)?.to_string());
            if values.is_empty() {
                let v = found.cloned().filter(|v| !v.is_nullish());
                t.datum = t.datum.with_field(&names[k], v.unwrap_or(default.clone()));
                k += 1;
                continue;
            }
            for value in &values {
                let v = match found {
                    Some(found) if !found.is_nullish() => value.get(view, found)?,
                    _ => default.clone(),
                };
                t.datum = t.datum.with_field(&names[k], v);
                k += 1;
            }
        }
    }
    Ok(tuples)
}
