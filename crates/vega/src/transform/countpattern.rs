//! vega-transforms' `CountPattern` and `DotBin`.

use crate::{
    Error,
    spec::{Countpattern, Dotbin},
    statistics::dotbin,
    transform::{Accessor, Key, Tuple, groups, or},
    value::{Value, regexp::RegExp},
    view::View,
};
use std::cmp::Ordering;

/// `countpattern`: each token of the field's text that is not a stopword,
/// and how many times it occurs, tokens in the order a JavaScript object
/// keeps them.
pub fn countpattern(
    view: &View,
    def: &Countpattern,
    tuples: &[Tuple],
) -> Result<Vec<Tuple>, Error> {
    let field = Accessor::new(view, &def.field)?;
    let case = or(view, &def.case, "mixed".to_string())?;
    let pattern = or(view, &def.pattern, String::new())?;
    let stopwords = or(view, &def.stopwords, String::new())?;
    let [text, count] = or(view, &def.as_, ["text".into(), "count".into()])?;
    let syntax = |e: crate::value::regexp::SyntaxError| Error::Spec(e.0);
    let matcher = RegExp::new(
        if pattern.is_empty() {
            "[\\w']+"
        } else {
            &pattern
        },
        "g",
    )
    .map_err(syntax)?;
    let stop = RegExp::new(&format!("^{stopwords}$"), "i").map_err(syntax)?;
    let mut counts: Vec<(String, f64)> = Vec::new();
    for t in tuples {
        let mut value = field.get(view, &t.datum)?.to_string();
        match case.as_str() {
            "upper" => value = value.to_uppercase(),
            "lower" => value = value.to_lowercase(),
            _ => {}
        }
        for token in matcher.matches(&value) {
            if stop.test(token) {
                continue;
            }
            match counts.iter_mut().find(|(t, _)| t == token) {
                Some((_, n)) => *n += 1.0,
                None => counts.push((token.to_string(), 1.0)),
            }
        }
    }
    // Integer keys first, ascending, then the rest as inserted.
    let index = |k: &str| {
        k.parse::<u32>()
            .ok()
            .filter(|i| i.to_string() == k && *i != u32::MAX)
    };
    counts.sort_by(|(a, _), (b, _)| match (index(a), index(b)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    Ok(counts
        .into_iter()
        .map(|(token, n)| {
            let datum = Value::Object(Vec::new().into())
                .with_field(&text, Value::from(token))
                .with_field(&count, Value::Number(n));
            view.ingest(datum)
        })
        .collect())
}

/// `dotbin`: each tuple's dot stack, within its group, the transform's value
/// the stacks' extent and step.
pub fn dotbin(
    view: &View,
    def: &Dotbin,
    mut tuples: Vec<Tuple>,
) -> Result<(Vec<Tuple>, Value), Error> {
    let field = Accessor::new(view, &def.field)?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let smooth = or(view, &def.smooth, false)?;
    let as_ = or(view, &def.as_, "bin".to_string())?;
    let values = tuples
        .iter()
        .map(|t| field.get(view, &t.datum).map(|v| v.to_number()))
        .collect::<Result<Vec<_>, _>>()?;
    let step = match or(view, &def.step, 0.0)? {
        s if s != 0.0 && !s.is_nan() => s,
        _ => {
            // vega-util's `span` of `extent`, over thirty.
            let (lo, hi) =
                values
                    .iter()
                    .filter(|v| !v.is_nan())
                    .fold((f64::NAN, f64::NAN), |(lo, hi), &v| {
                        (
                            if lo.is_nan() || v < lo { v } else { lo },
                            if hi.is_nan() || v > hi { v } else { hi },
                        )
                    });
            (hi - lo) / 30.0
        }
    };
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for mut group in groups(view, &groupby, Key::Partition, &tuples)? {
        group.sort_by(|&a, &b| match (values[a] - values[b]).partial_cmp(&0.0) {
            Some(Ordering::Equal) | None => tuples[a].id.cmp(&tuples[b].id),
            Some(order) => order,
        });
        let sorted: Vec<f64> = group.iter().map(|&i| values[i]).collect();
        for (&i, v) in group.iter().zip(dotbin::dotbin(&sorted, step, smooth)) {
            if v < min {
                min = v;
            }
            if v > max {
                max = v;
            }
            tuples[i].datum = tuples[i].datum.with_field(&as_, Value::Number(v));
        }
    }
    let value = Value::Object(
        vec![
            ("start".into(), Value::Number(min)),
            ("stop".into(), Value::Number(max)),
            ("step".into(), Value::Number(step)),
        ]
        .into(),
    );
    Ok((tuples, value))
}
