//! vega-transforms' `KDE` and `Density`.

use crate::{
    Error,
    spec::{self, Density, Kde},
    statistics::{curve, distribution::Distribution},
    transform::{Accessor, Group, Tuple, or, partition},
    value::Value,
    view::View,
};
use std::cmp::Ordering;

/// `kde`: each group's kernel density sampled along its extent, or along
/// the extent of every group where they share one.
pub fn kde(view: &View, def: &Kde, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let field = Accessor::new(view, &def.field)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let bandwidth = or(view, &def.bandwidth, 0.0)?;
    let cumulative = or(view, &def.cumulative, false)?;
    let counts = or(view, &def.counts, false)?;
    let [value, density] = as_pair(or(view, &def.as_, vec!["value".into(), "density".into()])?);
    let steps = or(view, &def.steps, 0.0)?;
    let steps = |n: f64, default: f64| match steps != 0.0 && !steps.is_nan() {
        true => steps,
        false if n != 0.0 && !n.is_nan() => n,
        false => default,
    };
    let mut minsteps = steps(or(view, &def.minsteps, 25.0)?, 25.0);
    let maxsteps = steps(or(view, &def.maxsteps, 200.0)?, 200.0);
    let mut domain = def.extent.as_ref().map(|e| view.param(e)).transpose()?;
    let values = tuples
        .iter()
        .map(|t| field.get(view, &t.datum))
        .collect::<Result<Vec<_>, _>>()?;
    if or(view, &def.resolve, "independent".to_string())? == "shared" {
        domain = domain.or_else(|| Some(extent(&values)));
        minsteps = maxsteps;
    }
    let mut out = Vec::new();
    for Group { dims, rows } in partition(view, &groupby, tuples)? {
        let support: Vec<Value> = rows.iter().map(|&i| values[i].clone()).collect();
        let kde = Distribution::kde(&support, bandwidth);
        let scale = if counts { support.len() as f64 } else { 1.0 };
        let local = domain.unwrap_or_else(|| extent(&support));
        let f = |x| match cumulative {
            true => kde.cdf(x),
            false => kde.pdf(x),
        };
        for [x, y] in curve::sample(f, local, minsteps as usize, maxsteps as usize) {
            let mut datum = Value::Object(Vec::new().into());
            for (g, v) in groupby.iter().zip(&dims) {
                datum = datum.with_field(&g.name, v.clone());
            }
            datum = datum
                .with_field(&value, Value::Number(x))
                .with_field(&density, Value::Number(y * scale));
            out.push(view.ingest(datum));
        }
    }
    Ok(out)
}

/// `density`: a distribution's density or cumulative probability sampled
/// along `extent`, or along its data's extent.
pub fn density(view: &View, def: &Density, tuples: &[Tuple]) -> Result<Vec<Tuple>, Error> {
    let (dist, data) = distribution(view, &def.distribution, tuples)?;
    let steps = or(view, &def.steps, 0.0)?;
    let steps = |n: f64, default: f64| match steps != 0.0 && !steps.is_nan() {
        true => steps,
        false if n != 0.0 && !n.is_nan() => n,
        false => default,
    };
    let minsteps = steps(or(view, &def.minsteps, 25.0)?, 25.0);
    let maxsteps = steps(or(view, &def.maxsteps, 200.0)?, 200.0);
    let method = or(view, &def.method, "pdf".to_string())?;
    if method != "pdf" && method != "cdf" {
        return Err(Error::Spec(format!("Invalid density method: {method}")));
    }
    let domain = match (
        def.extent.as_ref().map(|e| view.param(e)).transpose()?,
        data,
    ) {
        (Some(extent), _) => extent,
        (None, Some(data)) => extent(&data),
        (None, None) => return Err(Error::Spec("Missing density extent parameter.".into())),
    };
    let [value, density] = as_pair(or(view, &def.as_, vec!["value".into(), "density".into()])?);
    let f = |x| match method.as_str() {
        "cdf" => dist.cdf(x),
        _ => dist.pdf(x),
    };
    Ok(
        curve::sample(f, domain, minsteps as usize, maxsteps as usize)
            .into_iter()
            .map(|[x, y]| {
                let datum = Value::Object(Vec::new().into())
                    .with_field(&value, Value::Number(x))
                    .with_field(&density, Value::Number(y));
                view.ingest(datum)
            })
            .collect(),
    )
}

/// A distribution as written, and the values of its `field` for `kde`.
fn distribution(
    view: &View,
    def: &spec::Distribution,
    tuples: &[Tuple],
) -> Result<(Distribution, Option<Vec<Value>>), Error> {
    use spec::Distribution as D;
    Ok(match def {
        D::Normal { mean, stdev } => (
            Distribution::Normal {
                mean: or(view, mean, 0.0)?,
                stdev: or(view, stdev, 1.0)?,
            },
            None,
        ),
        D::Lognormal { mean, stdev } => (
            Distribution::Lognormal {
                mean: or(view, mean, 0.0)?,
                stdev: or(view, stdev, 1.0)?,
            },
            None,
        ),
        D::Uniform { min, max } => (
            Distribution::Uniform {
                min: or(view, min, 0.0)?,
                max: or(view, max, 1.0)?,
            },
            None,
        ),
        D::Kde {
            field,
            from,
            bandwidth,
        } => {
            let field = Accessor::new(view, field)?;
            let source = match from {
                Some(from) => view.data(from)?,
                None => tuples,
            };
            let values = source
                .iter()
                .map(|t| field.get(view, &t.datum))
                .collect::<Result<Vec<_>, _>>()?;
            let kde = Distribution::kde(&values, or(view, bandwidth, 0.0)?);
            (kde, Some(values))
        }
        D::Mixture {
            distributions,
            weights,
        } => {
            let distributions = distributions
                .iter()
                .map(|d| Ok(distribution(view, d, tuples)?.0))
                .collect::<Result<Vec<_>, Error>>()?;
            let weights: Vec<Option<f64>> = or(view, weights, Vec::new())?
                .into_iter()
                .map(Some)
                .collect();
            (Distribution::mixture(distributions, &weights), None)
        }
    })
}

/// vega-util's `extent`: the least and greatest values but null and `NaN`,
/// compared as JavaScript compares them; `NaN` where there are none.
fn extent(values: &[Value]) -> [f64; 2] {
    let mut min: Option<&Value> = None;
    let mut max: Option<&Value> = None;
    for v in values {
        if v.is_nullish() || v.compare(v).is_none() {
            continue;
        }
        if min.is_none_or(|m| m.compare(v) == Some(Ordering::Greater)) {
            min = Some(v);
        }
        if max.is_none_or(|m| m.compare(v) == Some(Ordering::Less)) {
            max = Some(v);
        }
    }
    [min, max].map(|v| v.map_or(f64::NAN, Value::to_number))
}

/// The first two names of `as`.
fn as_pair(names: Vec<String>) -> [String; 2] {
    let mut names = names.into_iter();
    [
        names.next().unwrap_or_default(),
        names.next().unwrap_or_default(),
    ]
}
