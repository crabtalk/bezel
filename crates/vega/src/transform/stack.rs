//! vega-encode's `Stack`.

use crate::{
    Error,
    spec::{Offset, Stack},
    transform::{Accessor, Comparator, Tuple, or},
    value::Value,
    view::View,
};
use std::collections::HashMap;

/// Each tuple's `field` stacked within its `groupby` group, in `sort` order,
/// as `as`; tuples keep their order.
pub fn stack(view: &View, def: &Stack, mut tuples: Vec<Tuple>) -> Result<Vec<Tuple>, Error> {
    let [y0, y1] = or(view, &def.as_, ["y0".into(), "y1".into()])?;
    let field = def
        .field
        .as_ref()
        .map(|f| Accessor::new(view, f))
        .transpose()?;
    let groupby = Accessor::many(view, &def.groupby)?;
    let sort = def
        .sort
        .as_ref()
        .map(Comparator::new)
        .transpose()?
        .flatten();
    let value = |t: &Tuple| -> Result<f64, Error> {
        Ok(match &field {
            Some(field) => field.get(view, &t.datum)?.to_number(),
            None => 1.0,
        })
    };
    // `partition`: groups by the JSON of their keys, in order of first sight.
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, t) in tuples.iter().enumerate() {
        let key = groupby
            .iter()
            .map(|f| f.get(view, &t.datum))
            .collect::<Result<Vec<_>, _>>()?;
        let key = Value::from(key).to_json_text();
        let group = *index.entry(key).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[group].push(i);
    }
    let mut sums = Vec::with_capacity(groups.len());
    let mut max = 0.0f64;
    for group in &mut groups {
        let mut s = 0.0;
        for &i in group.iter() {
            s += value(&tuples[i])?.abs();
        }
        if s > max {
            max = s;
        }
        sums.push(s);
        if let Some(sort) = &sort {
            sort.sort_by(view, group, |&i| &tuples[i])?;
        }
    }
    let offset = or(view, &def.offset, Offset::Zero)?;
    for (group, sum) in groups.iter().zip(sums) {
        let mut ends = Vec::with_capacity(group.len());
        match offset {
            Offset::Center => {
                let mut last = (max - sum) / 2.0;
                for &i in group {
                    let v = value(&tuples[i])?.abs();
                    let start = last;
                    last += v;
                    ends.push((i, start, last));
                }
            }
            Offset::Normalize => {
                let scale = 1.0 / sum;
                let (mut last, mut v) = (0.0, 0.0);
                for &i in group {
                    let start = last;
                    v += value(&tuples[i])?.abs();
                    last = scale * v;
                    ends.push((i, start, last));
                }
            }
            Offset::Zero => {
                let (mut pos, mut neg) = (0.0, 0.0);
                for &i in group {
                    let v = value(&tuples[i])?;
                    if v < 0.0 {
                        let start = neg;
                        neg += v;
                        ends.push((i, start, neg));
                    } else {
                        let start = pos;
                        pos += v;
                        ends.push((i, start, pos));
                    }
                }
            }
        }
        for (i, start, end) in ends {
            let datum = &mut tuples[i].datum;
            *datum = datum
                .with_field(&y0, Value::Number(start))
                .with_field(&y1, Value::Number(end));
        }
    }
    Ok(tuples)
}
