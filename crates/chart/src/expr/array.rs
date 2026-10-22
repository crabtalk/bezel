//! Vega's array functions, after `vega-util` and `d3-array`.

use crate::{
    expr::{Error, string},
    path::Path,
};
use js::{math, number, value::Value};
use std::cmp::Ordering;

/// `value`'s items, or the TypeError calling array method `name` on it throws.
pub(crate) fn items<'a>(value: &'a Value, name: &str) -> Result<&'a [Value], Error> {
    match value {
        Value::Array(items) => Ok(items),
        _ => Err(Error::Runtime(format!("{name}: {value} is not an array"))),
    }
}

/// `[min, max]` of the items that are neither null nor `NaN`, by `<`.
pub(crate) fn extent(value: &Value) -> Result<Value, Error> {
    let items = if value.is_nullish() {
        &[]
    } else {
        items(value, "extent")?
    };
    let mut valid = items
        .iter()
        .skip_while(|v| v.is_nullish() || matches!(v, Value::Number(n) if n.is_nan()));
    let Some(first) = valid.next() else {
        return Ok(vec![Value::Undefined, Value::Undefined].into());
    };
    let (mut min, mut max) = (first, first);
    for item in valid.filter(|v| !v.is_nullish()) {
        if item.compare(min) == Some(Ordering::Less) {
            min = item;
        }
        if item.compare(max) == Some(Ordering::Greater) {
            max = item;
        }
    }
    Ok(vec![min.clone(), max.clone()].into())
}

/// `range` moved, keeping its span, to lie inside `[min, max]`; `[min, max]`
/// if it spans at least that much.
pub(crate) fn clamp_range(range: &Value, min: f64, max: f64) -> Result<Value, Error> {
    let mut lo = range.get(&Value::Number(0.0))?.to_number();
    let mut hi = range.get(&Value::Number(1.0))?.to_number();
    if hi < lo {
        std::mem::swap(&mut lo, &mut hi);
    }
    let span = hi - lo;
    let (lo, hi) = if span >= max - min {
        (min, max)
    } else {
        let lo = lo.max(min).min(max - span);
        (lo, lo + span)
    };
    Ok(vec![Value::Number(lo), Value::Number(hi)].into())
}

/// `haystack.indexOf(needle)`, or `lastIndexOf` if `last`: by `===` in an
/// array, by UTF-16 code unit in a string.
pub(crate) fn index_of(haystack: &Value, needle: &Value, last: bool) -> Result<Value, Error> {
    let found = match haystack {
        Value::Array(items) => {
            let hit = |item: &Value| item.strict_eq(needle);
            if last {
                items.iter().rposition(hit)
            } else {
                items.iter().position(hit)
            }
        }
        Value::String(text) => string::index_of(text, &needle.to_string(), last),
        _ => {
            return Err(Error::Runtime(format!(
                "indexof: {haystack} is not an array or a string"
            )));
        }
    };
    Ok(Value::Number(found.map_or(-1.0, |i| i as f64)))
}

/// Whether `value` lies between the first and last items of `range`,
/// inclusive, in either order.
pub(crate) fn in_range(value: &Value, range: &Value) -> Result<Value, Error> {
    let length = range.get(&"length".into())?.to_number();
    let mut lo = range.get(&Value::Number(0.0))?;
    let mut hi = range.get(&Value::Number(length - 1.0))?;
    if lo.compare(&hi) == Some(Ordering::Greater) {
        std::mem::swap(&mut lo, &mut hi);
    }
    let le = |a: &Value, b: &Value| matches!(a.compare(b), Some(Ordering::Less | Ordering::Equal));
    Ok((le(&lo, value) && le(value, &hi)).into())
}

/// `items.join(separator)`: `,` if `separator` is undefined, null and
/// undefined items empty.
pub(crate) fn join(items: &[Value], separator: &Value) -> Value {
    let separator = match separator {
        Value::Undefined => ",".to_string(),
        other => other.to_string(),
    };
    items
        .iter()
        .map(|item| {
            if item.is_nullish() {
                String::new()
            } else {
                item.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(&separator)
        .into()
}

/// The point `fraction` of the way from the first item to the last.
pub(crate) fn lerp(items: &[Value], fraction: &Value) -> Value {
    let lo = items.first().cloned().unwrap_or_default();
    let hi = items.last().cloned().unwrap_or_default();
    match fraction.to_number() {
        f if f == 0.0 || f.is_nan() => lo,
        1.0 => hi,
        f => &lo + &Value::Number(f * (hi.to_number() - lo.to_number())),
    }
}

/// Each item's value at `field`, a Vega access path such as `a.b` or
/// `a["b"][0]`.
pub(crate) fn pluck(items: &[Value], field: &str) -> Result<Value, Error> {
    let path: Path = field.parse().map_err(Error::Runtime)?;
    Ok(items
        .iter()
        .map(|item| path.get(item))
        .collect::<Result<Vec<_>, _>>()?
        .into())
}

/// d3's `range`: `sequence(stop)`, `sequence(start, stop)` or
/// `sequence(start, stop, step)`.
pub(crate) fn sequence(args: &[Value]) -> Value {
    let number = |i: usize| args.get(i).map_or(f64::NAN, Value::to_number);
    let (start, stop, step) = match args.len() {
        0 | 1 => (0.0, number(0), 1.0),
        2 => (number(0), number(1), 1.0),
        _ => (number(0), number(1), number(2)),
    };
    // JavaScript's `Math.max(0, n) | 0`, which also makes `NaN` and infinite
    // counts 0.
    let count = number::to_int32(math::max(0.0, ((stop - start) / step).ceil()));
    let count = count.max(0) as usize;
    (0..count)
        .map(|i| Value::Number(start + i as f64 * step))
        .collect::<Vec<_>>()
        .into()
}

/// `value.slice(start, end)` on an array or a string.
pub(crate) fn slice(value: &Value, start: &Value, end: &Value) -> Result<Value, Error> {
    Ok(match value {
        Value::Array(items) => {
            let (from, to) = string::bounds(items.len(), start, end);
            items[from..to].to_vec().into()
        }
        Value::String(text) => string::slice_text(text, start, end).into(),
        _ => {
            return Err(Error::Runtime(format!(
                "slice: {value} is not an array or a string"
            )));
        }
    })
}

/// A sorted copy, ascending by `<` as d3's `ascending` orders, stable.
/// `undefined` sorts last; null, `NaN` and other incomparable pairs are left
/// in their order.
pub(crate) fn sort(items: &[Value]) -> Value {
    let (mut defined, undefined): (Vec<Value>, Vec<Value>) = items
        .iter()
        .cloned()
        .partition(|item| !matches!(item, Value::Undefined));
    // A merge sort, which never assumes the order is total.
    let mut width = 1;
    while width < defined.len() {
        let mut merged = Vec::with_capacity(defined.len());
        for chunk in defined.chunks(2 * width) {
            let (left, right) = chunk.split_at(width.min(chunk.len()));
            let (mut l, mut r) = (0, 0);
            while l < left.len() && r < right.len() {
                let greater = !left[l].is_nullish()
                    && !right[r].is_nullish()
                    && left[l].compare(&right[r]) == Some(Ordering::Greater);
                if greater {
                    merged.push(right[r].clone());
                    r += 1;
                } else {
                    merged.push(left[l].clone());
                    l += 1;
                }
            }
            merged.extend_from_slice(&left[l..]);
            merged.extend_from_slice(&right[r..]);
        }
        defined = merged;
        width *= 2;
    }
    defined.extend(undefined);
    defined.into()
}

/// The last item minus the first, or 0 if that is not a number other than 0.
pub(crate) fn span(value: &Value) -> Result<Value, Error> {
    if !value.truthy() {
        return Ok(Value::Number(0.0));
    }
    let length = value.get(&"length".into())?.to_number();
    let last = value.get(&Value::Number(length - 1.0))?.to_number();
    let span = last - value.get(&Value::Number(0.0))?.to_number();
    Ok(Value::Number(if span.is_nan() { 0.0 } else { span }))
}
