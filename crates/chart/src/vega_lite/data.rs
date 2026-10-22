//! A view's `data`: inline `values`, a `url` the host loads, a `name` in the
//! top level's `datasets`, or a generator — `sequence`, `sphere` or
//! `graticule`.
//!
//! A `sphere` is one row `{"type": "Sphere"}` and a `graticule` one row
//! `{"type": "MultiLineString"}`, without coordinates.

use crate::{
    data::{Column, Data, Name},
    model::Kind,
    path::Path,
    time,
    vega_lite::format::{self, Format, Row},
};
use gpui::SharedString;
use js::value::Value as JsValue;
use serde_json::{Map, Number, Value};

/// What a `data` object may carry.
const DATA: [&str; 7] = [
    "values",
    "url",
    "name",
    "format",
    "sequence",
    "sphere",
    "graticule",
];
/// What a `graticule` object may carry.
const GRATICULE: [&str; 7] = [
    "extent",
    "extentMajor",
    "extentMinor",
    "step",
    "stepMajor",
    "stepMinor",
    "precision",
];

/// Where a view's rows come from.
pub(crate) struct Sources<'s, 'a> {
    /// The top level's `datasets`.
    pub datasets: Option<&'s Map<String, Value>>,
    pub load: &'s dyn Fn(&str) -> Option<&'a str>,
}

impl Sources<'_, '_> {
    /// The rows `data` names. `None` for a key or generator setting `data`
    /// does not take, a `url` the loader has no text for or a `name` with no
    /// dataset, and text its format does not read.
    pub(crate) fn rows(&self, data: &Value) -> Option<Vec<Row>> {
        let data = data.as_object()?;
        if data.keys().any(|key| !DATA.contains(&key.as_str())) {
            return None;
        }
        if let Some(sequence) = data.get("sequence") {
            return sequence_rows(sequence.as_object()?);
        }
        if let Some(sphere) = data.get("sphere") {
            return generated(sphere, &[]).then(|| shape("Sphere"));
        }
        if let Some(graticule) = data.get("graticule") {
            return generated(graticule, &GRATICULE).then(|| shape("MultiLineString"));
        }
        let url = data.get("url").map(Value::as_str);
        let format = Format::new(data.get("format"), url.flatten())?;
        if let Some(url) = url {
            return format.read((self.load)(url?)?);
        }
        if let Some(values) = data.get("values") {
            return format.values(values);
        }
        let name = data.get("name")?.as_str()?;
        format.values(self.datasets?.get(name)?)
    }

    /// The rows `data` names as a table: each field of `reads` once, read as
    /// a path into each row with the type given, a temporal one over any
    /// other and otherwise the first given; then every other key of the
    /// rows as it is. `None` as for [`Self::rows`].
    pub(crate) fn table(
        &self,
        data: &Value,
        reads: &[(SharedString, Option<Kind>)],
    ) -> Option<Data> {
        let rows = self.rows(data)?;
        let mut fields: Vec<(&str, Option<Kind>, bool)> = Vec::new();
        for (field, kind, path) in reads
            .iter()
            .map(|(field, kind)| (field.as_ref(), *kind, true))
            .chain(
                rows.iter()
                    .flat_map(|row| row.keys().map(|key| (key.as_str(), None, false))),
            )
        {
            match fields.iter_mut().find(|(name, ..)| *name == field) {
                Some((_, slot, _)) => {
                    if kind == Some(Kind::Temporal) || slot.is_none() {
                        *slot = kind;
                    }
                }
                None => fields.push((field, kind, path)),
            }
        }
        let mut table = Data::new();
        for (field, kind, path) in fields {
            let values: Vec<Option<&Value>> = match path {
                true => {
                    let path: Path = field.parse().ok()?;
                    rows.iter().map(|row| path.field(row)).collect()
                }
                false => rows.iter().map(|row| row.get(field)).collect(),
            };
            table = table.with(field.to_string(), column(&values, kind));
        }
        Some(table)
    }
}

/// A field's values as the channel of `kind` that encodes it reads them,
/// and as they are where no channel types it or they hold an array or
/// object. `None` is a row that lacks the field.
fn column(values: &[Option<&Value>], kind: Option<Kind>) -> Column {
    let numbers = |read: fn(&Value) -> f64| {
        Column::Number(values.iter().map(|v| v.map_or(f64::NAN, read)).collect())
    };
    let js = || {
        values
            .iter()
            .map(|v| v.map_or(JsValue::Undefined, format::js))
    };
    let all = |test: fn(&Value) -> bool| values.iter().flatten().all(|v| test(v));
    match kind {
        Some(Kind::Temporal) => numbers(|value| match value {
            Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
            Value::String(s) => time::parse(s).unwrap_or(f64::NAN),
            _ => f64::NAN,
        }),
        Some(Kind::Quantitative) => numbers(|value| match value {
            Value::Null => f64::NAN,
            value => format::js(value).to_number(),
        }),
        _ if all(|v| v.is_number() || v.is_null()) => {
            numbers(|value| value.as_f64().unwrap_or(f64::NAN))
        }
        // Read as it is, an empty string is missing as null is.
        None if all(|v| v.is_number() || v.is_null() || v.as_str() == Some("")) => {
            numbers(|value| value.as_f64().unwrap_or(f64::NAN))
        }
        // An array or object is kept as it is.
        Some(_) if !all(|v| !v.is_array() && !v.is_object()) => js().collect(),
        Some(_) => Column::Text(js().map(|value| Name::from(&value)).collect()),
        None => js().collect(),
    }
}

/// `true`, or an object of `keys`.
fn generated(setting: &Value, keys: &[&str]) -> bool {
    match setting {
        Value::Bool(on) => *on,
        Value::Object(setting) => setting.keys().all(|key| keys.contains(&key.as_str())),
        _ => false,
    }
}

fn shape(kind: &str) -> Vec<Row> {
    vec![Map::from_iter([(
        "type".to_owned(),
        Value::String(kind.to_owned()),
    )])]
}

/// d3's `range(start, stop, step)` as a field named by `as`, `data` by default.
fn sequence_rows(sequence: &Map<String, Value>) -> Option<Vec<Row>> {
    if sequence
        .keys()
        .any(|key| !["start", "stop", "step", "as"].contains(&key.as_str()))
    {
        return None;
    }
    let start = sequence.get("start")?.as_f64()?;
    let stop = sequence.get("stop")?.as_f64()?;
    let step = match sequence.get("step") {
        Some(step) => step.as_f64()?,
        None => 1.0,
    };
    let name = match sequence.get("as") {
        Some(name) => name.as_str()?,
        None => "data",
    };
    let count = ((stop - start) / step).ceil();
    let count = if count.is_finite() && count > 0.0 {
        count as usize
    } else {
        0
    };
    (0..count)
        .map(|i| {
            let value = Number::from_f64(start + i as f64 * step)?;
            Some(Map::from_iter([(name.to_owned(), Value::Number(value))]))
        })
        .collect()
}
