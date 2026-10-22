//! vega-transforms' `Bin`.

use crate::{
    Error,
    spec::Bin,
    statistics::bin::{self, Bins},
    transform::{Accessor, Tuple, or},
    value::Value,
    view::View,
};

/// Against floating point error at a bin's edge.
const EPSILON: f64 = 1e-14;

/// Each tuple's bin of the field: where it starts and where it ends. A
/// value outside the bins starts at an infinity. The transform's value is
/// the bins.
pub fn bin(view: &View, def: &Bin, mut tuples: Vec<Tuple>) -> Result<(Vec<Tuple>, Value), Error> {
    let field = Accessor::new(view, &def.field)?;
    let band = or(view, &def.interval, true)?;
    let extent = view.param_value(&def.extent)?;
    let extent = [0.0, 1.0].map(|i| {
        extent
            .get(&Value::Number(i))
            .unwrap_or_default()
            .to_number()
    });
    let params = bin::Params {
        maxbins: or(view, &def.maxbins, 20.0)?,
        base: or(view, &def.base, 10.0)?,
        divide: or(view, &def.divide, vec![5.0, 2.0])?,
        extent,
        span: or(view, &def.span, 0.0)?,
        step: or(view, &def.step, 0.0)?,
        steps: def.steps.as_ref().map(|s| view.param(s)).transpose()?,
        minstep: or(view, &def.minstep, 0.0)?,
        nice: or(view, &def.nice, true)?,
    };
    let bins = bin::bin(&params);
    let Bins {
        mut start, step, ..
    } = bins;
    let mut stop = start + ((bins.stop - start) / step).ceil() * step;
    if let Some(anchor) = def.anchor.as_ref().map(|a| view.param(a)).transpose()? {
        let shift = anchor - (start + step * ((anchor - start) / step).floor());
        start += shift;
        stop += shift;
    }
    let [b0, b1] = or(view, &def.as_, ["bin0".into(), "bin1".into()])?;
    for t in &mut tuples {
        let v = field.get(view, &t.datum)?;
        let v = match &v {
            v if v.is_nullish() => None,
            Value::String(s) if s.is_empty() => None,
            v => Some(v.to_number()),
        };
        let first = v.map(|v| {
            if v < start {
                f64::NEG_INFINITY
            } else if v > stop {
                f64::INFINITY
            } else {
                let v = start.max(v.min(stop - step));
                start + step * (EPSILON + (v - start) / step).floor()
            }
        });
        t.datum = t
            .datum
            .with_field(&b0, first.map_or(Value::Null, Value::Number));
        if band {
            let end = first.map(|v| start + step * (1.0 + (v - start) / step));
            t.datum = t
                .datum
                .with_field(&b1, end.map_or(Value::Null, Value::Number));
        }
    }
    let name = match &def.name {
        Some(name) => view.param(name)?,
        None => format!("bin_{}", field.name),
    };
    let value = Value::Object(
        vec![
            ("start".into(), Value::Number(start)),
            ("stop".into(), Value::Number(stop)),
            ("step".into(), Value::Number(step)),
            ("fname".into(), Value::from(name)),
        ]
        .into(),
    );
    Ok((tuples, value))
}
