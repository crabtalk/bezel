//! vega-regression's `Loess` and `Regression`, and vega-transforms'
//! `Quantile`.

use crate::{
    Error,
    spec::{Loess, Quantile, Regression},
    statistics::{
        self, curve,
        regression::{self, Fit, Method},
    },
    transform::{Accessor, Group, Tuple, or, partition},
    value::Value,
    view::View,
};
use std::cmp::Ordering;

/// A value as `visitPoints` reads it: `NaN` for null and undefined.
fn point(v: &Value) -> f64 {
    match v.is_nullish() {
        true => f64::NAN,
        false => v.to_number(),
    }
}

/// A tuple of a group's `groupby` values, then `[x, y]` as `as`.
fn tuple(
    view: &View,
    groupby: &[Accessor],
    dims: &[Value],
    as_: &[String; 2],
    [x, y]: [Value; 2],
) -> Tuple {
    let mut datum = Value::Object(Vec::new().into());
    for (g, v) in groupby.iter().zip(dims) {
        datum = datum.with_field(&g.name, v.clone());
    }
    datum = datum.with_field(&as_[0], x).with_field(&as_[1], y);
    view.ingest(datum)
}

/// The output names: `as`, else the accessors' names.
fn names(
    view: &View,
    as_: &Option<crate::spec::Param<Vec<String>>>,
    x: &Accessor,
    y: &Accessor,
) -> Result<[String; 2], Error> {
    let as_ = or(view, as_, vec![x.name.clone(), y.name.clone()])?;
    let mut as_ = as_.into_iter();
    Ok([
        as_.next().unwrap_or_default(),
        as_.next().unwrap_or_default(),
    ])
}

pub fn loess(view: &View, def: &Loess, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let x = Accessor::new(view, &def.x)?;
    let y = Accessor::new(view, &def.y)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let bandwidth = or(view, &def.bandwidth, 0.3)?;
    let bandwidth = if bandwidth == 0.0 || bandwidth.is_nan() {
        0.3
    } else {
        bandwidth
    };
    let as_ = names(view, &def.as_, &x, &y)?;
    let mut out = Vec::new();
    for Group { dims, rows } in partition(view, &groupby, tuples)? {
        let mut pairs = Vec::with_capacity(rows.len());
        for &i in &rows {
            let datum = &tuples[i].datum;
            let p = [point(&x.get(view, datum)?), point(&y.get(view, datum)?)];
            if !p[0].is_nan() && !p[1].is_nan() {
                pairs.push(p);
            }
        }
        pairs.sort_by(|a, b| (a[0] - b[0]).partial_cmp(&0.0).unwrap_or(Ordering::Equal));
        for [px, py] in regression::loess(&pairs, bandwidth) {
            out.push(tuple(
                view,
                &groupby,
                &dims,
                &as_,
                [Value::Number(px), Value::Number(py)],
            ));
        }
    }
    Ok(out)
}

pub fn regression(view: &View, def: &Regression, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let x = Accessor::new(view, &def.x)?;
    let y = Accessor::new(view, &def.y)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let order = or(view, &def.order, 3.0)?;
    let method = match or(view, &def.method, "linear".to_string())?.as_str() {
        "constant" => Method::Constant,
        "linear" => Method::Linear,
        "log" => Method::Log,
        "exp" => Method::Exp,
        "pow" => Method::Pow,
        "quad" => Method::Quad,
        "poly" => Method::Poly(order as usize),
        other => return Err(Error::Spec(format!("Invalid regression method: {other}"))),
    };
    let dof = match method {
        Method::Poly(_) => order,
        Method::Quad => 2.0,
        _ => 1.0,
    };
    let domain = def
        .extent
        .as_ref()
        .map(|e| view.param(e))
        .transpose()?
        .filter(|[low, _]| !(method == Method::Log && *low <= 0.0));
    let params = or(view, &def.params, false)?;
    let as_ = names(view, &def.as_, &x, &y)?;
    let mut out = Vec::new();
    for Group { dims, rows } in partition(view, &groupby, tuples)? {
        if rows.len() as f64 <= dof {
            continue;
        }
        let mut xs = Vec::with_capacity(rows.len());
        let mut ys = Vec::with_capacity(rows.len());
        let mut raw = Vec::with_capacity(rows.len());
        for &i in &rows {
            let datum = &tuples[i].datum;
            let (xv, yv) = (x.get(view, datum)?, y.get(view, datum)?);
            xs.push(point(&xv));
            ys.push(point(&yv));
            raw.push(xv);
        }
        let fit = Fit::new(method, &xs, &ys);
        if params {
            let mut datum = Value::Object(Vec::new().into());
            if !groupby.is_empty() {
                datum = datum.with_field("keys", Value::from(dims.clone()));
            }
            let coef = fit
                .coef()
                .into_iter()
                .map(Value::Number)
                .collect::<Vec<_>>();
            datum = datum
                .with_field("coef", Value::from(coef))
                .with_field("rSquared", Value::Number(fit.r_squared(&xs, &ys)));
            out.push(view.ingest(datum));
            continue;
        }
        let domain = domain.unwrap_or_else(|| extent(&raw));
        let points = match method {
            Method::Linear | Method::Constant => domain.map(|x| [x, fit.predict(x)]).to_vec(),
            _ => curve::sample(|x| fit.predict(x), domain, 25, 200),
        };
        for [px, py] in points {
            out.push(tuple(
                view,
                &groupby,
                &dims,
                &as_,
                [Value::Number(px), Value::Number(py)],
            ));
        }
    }
    Ok(out)
}

pub fn quantile(view: &View, def: &Quantile, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let field = Accessor::new(view, &def.field)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let as_ = or(view, &def.as_, ["prob".into(), "value".into()])?;
    let probs = match def.probs.as_ref().map(|p| view.param(p)).transpose()? {
        Some(probs) => probs,
        None => {
            let step = or(view, &def.step, 0.01)?;
            let step = if step == 0.0 || step.is_nan() {
                0.01
            } else {
                step
            };
            let start = step / 2.0;
            let count = ((1.0 - 1e-14 - start) / step).ceil().max(0.0) as usize;
            (0..count).map(|i| start + i as f64 * step).collect()
        }
    };
    let mut out = Vec::new();
    for Group { dims, rows } in partition(view, &groupby, tuples)? {
        let values = rows
            .iter()
            .map(|&i| field.get(view, &tuples[i].datum))
            .collect::<Result<Vec<_>, _>>()?;
        let q = statistics::quantiles(&statistics::numbers(&values), &probs);
        for (p, q) in probs.iter().zip(q) {
            out.push(tuple(view, &groupby, &dims, &as_, [Value::Number(*p), q]));
        }
    }
    Ok(out)
}

/// vega-util's `extent` of numbers: the least and greatest but null and
/// `NaN`.
fn extent(values: &[Value]) -> [f64; 2] {
    let mut out: Option<[f64; 2]> = None;
    for v in values {
        if v.is_nullish() {
            continue;
        }
        let v = v.to_number();
        if v.is_nan() {
            continue;
        }
        out = Some(match out {
            None => [v, v],
            Some([lo, hi]) => [lo.min(v), hi.max(v)],
        });
    }
    out.unwrap_or([f64::NAN, f64::NAN])
}
