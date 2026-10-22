//! vega-transforms' and vega-encode's data transforms, on a data set's
//! tuples.

use crate::{
    Error,
    path::Path,
    spec::{self, Offset},
    value::Value,
};

/// vega-encode's `Stack`.
pub fn stack(def: &spec::Stack, tuples: Vec<Value>) -> Result<Vec<Value>, Error> {
    if def
        .sort
        .as_ref()
        .is_some_and(|s| s.field.as_ref().is_some_and(|f| !f.items().is_empty()))
    {
        return Err(Error::Unsupported("a sorted stack".into()));
    }
    let [y0, y1] = def.as_.clone().unwrap_or(["y0".into(), "y1".into()]);
    let field = def
        .field
        .as_deref()
        .map(str::parse::<Path>)
        .transpose()
        .map_err(Error::Spec)?;
    let groupby: Vec<Path> = def
        .groupby
        .iter()
        .map(|f| f.parse())
        .collect::<Result<_, _>>()
        .map_err(Error::Spec)?;
    let value = |t: &Value| -> Result<f64, Error> {
        Ok(match &field {
            Some(path) => path.get(t).map_err(|e| Error::Eval(e.into()))?.to_number(),
            None => 1.0,
        })
    };
    // `partition`: groups by the JSON of their keys, in order of first sight.
    let mut keys: Vec<String> = Vec::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (i, t) in tuples.iter().enumerate() {
        let key = groupby
            .iter()
            .map(|path| path.get(t).map(|v| v.to_json_text()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Error::Eval(e.into()))?
            .join(",");
        match keys.iter().position(|k| *k == key) {
            Some(g) => groups[g].push(i),
            None => {
                keys.push(key);
                groups.push(vec![i]);
            }
        }
    }
    let mut sums = Vec::with_capacity(groups.len());
    let mut max = 0.0f64;
    for group in &groups {
        let mut s = 0.0;
        for &i in group {
            s += value(&tuples[i])?.abs();
        }
        if s > max {
            max = s;
        }
        sums.push(s);
    }
    let mut out = tuples;
    let offset = def.offset.unwrap_or(Offset::Zero);
    for (group, sum) in groups.iter().zip(sums) {
        match offset {
            Offset::Center => {
                let mut last = (max - sum) / 2.0;
                for &i in group {
                    let v = value(&out[i])?.abs();
                    let t = out[i].with_field(&y0, Value::Number(last));
                    last += v;
                    out[i] = t.with_field(&y1, Value::Number(last));
                }
            }
            Offset::Normalize => {
                let scale = 1.0 / sum;
                let (mut last, mut v) = (0.0, 0.0);
                for &i in group {
                    let t = out[i].with_field(&y0, Value::Number(last));
                    v += value(&out[i])?.abs();
                    last = scale * v;
                    out[i] = t.with_field(&y1, Value::Number(last));
                }
            }
            Offset::Zero => {
                let (mut pos, mut neg) = (0.0, 0.0);
                for &i in group {
                    let v = value(&out[i])?;
                    let (start, end) = if v < 0.0 {
                        let start = neg;
                        neg += v;
                        (start, neg)
                    } else {
                        let start = pos;
                        pos += v;
                        (start, pos)
                    };
                    out[i] = out[i]
                        .with_field(&y0, Value::Number(start))
                        .with_field(&y1, Value::Number(end));
                }
            }
        }
    }
    Ok(out)
}
