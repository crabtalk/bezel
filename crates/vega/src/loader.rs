//! vega-loader's `read`: a data set's text or inline values made into data
//! objects by its `format`, and the `parse` applied to their fields.
//!
//! Dates parse in the host's zone, as `Date.parse` and d3's `timeParse`
//! read them, and become milliseconds since the epoch.

use crate::{
    Error,
    path::Path,
    spec::{Format, Parse},
    value::Value,
};
use d3::{dsv, time::date::LocalZone};
use serde_json::Value as Json;

enum Kind {
    Json,
    Dsv(char),
}

fn kind(format: &Format) -> Result<Kind, Error> {
    Ok(match format.type_.as_deref().unwrap_or("json") {
        "json" => Kind::Json,
        "csv" => Kind::Dsv(','),
        "tsv" => Kind::Dsv('\t'),
        "dsv" => {
            let delimiter = format.delimiter.as_deref().unwrap_or_default();
            let mut chars = delimiter.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Kind::Dsv(c),
                _ => {
                    return Err(Error::Spec("Delimiter must be a single character.".into()));
                }
            }
        }
        "topojson" => return Err(Error::Unsupported("topojson data".into())),
        other => return Err(Error::Spec(format!("Unknown data format type: {other}"))),
    })
}

/// The data objects `text` holds in `format`.
pub fn read(text: &str, format: &Format) -> Result<Vec<Value>, Error> {
    let data = match kind(format)? {
        Kind::Dsv(delimiter) => dsv::parse(text, delimiter)
            .into_iter()
            .map(|row| {
                Value::Object(
                    row.into_iter()
                        .map(|(k, v)| (k.into(), Value::from(v)))
                        .collect(),
                )
            })
            .collect(),
        Kind::Json => {
            let json: Json = serde_json::from_str(text).map_err(|e| Error::Load(e.to_string()))?;
            items(property(&Value::from(&json), format)?)
        }
    };
    parse(data, format)
}

/// The data objects inline `values` hold: text read as [`read`] reads it.
pub fn values(values: &Json, format: &Format) -> Result<Vec<Value>, Error> {
    if let Json::String(text) = values {
        return read(text, format);
    }
    let data = match kind(format)? {
        Kind::Json => items(property(&Value::from(values), format)?),
        Kind::Dsv(_) => return read(&Value::from(values).to_string(), format),
    };
    parse(data, format)
}

/// The value at `format.property`.
fn property(value: &Value, format: &Format) -> Result<Value, Error> {
    match &format.property {
        Some(property) => {
            let path: Path = property.parse().map_err(Error::Spec)?;
            path.get(value).map_err(|e| Error::Eval(e.into()))
        }
        None => Ok(value.clone()),
    }
}

/// An array's items, or the one value that is not an array.
fn items(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items.to_vec(),
        value => vec![value],
    }
}

/// `parse`: each field's parser over each object, fields in the order the
/// types name them; `auto` infers a type for each field of the first object.
fn parse(mut data: Vec<Value>, format: &Format) -> Result<Vec<Value>, Error> {
    let Some(types) = &format.parse else {
        return Ok(data);
    };
    if data.is_empty() {
        return Ok(data);
    }
    let types: Vec<(String, String)> = match types {
        Parse::Fields(types) => types.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Parse::Auto(_) => {
            let Value::Object(fields) = &data[0] else {
                return Ok(data);
            };
            fields
                .iter()
                .map(|(field, _)| (field.to_string(), infer(&data, field).to_string()))
                .collect()
        }
    };
    let zone = LocalZone::default();
    let utc = LocalZone::utc();
    let parsers = types
        .iter()
        .map(|(field, type_)| Parser::new(field, type_))
        .collect::<Result<Vec<_>, _>>()?;
    for datum in &mut data {
        for ((field, _), parser) in types.iter().zip(&parsers) {
            let value = datum.get(&field.as_str().into()).unwrap_or_default();
            let parsed = parser.parse(value, &zone, &utc);
            *datum = datum.with_field(field, parsed);
        }
    }
    Ok(data)
}

/// vega-loader's `typeParsers`, and a d3 time format's parser.
enum Parser {
    Boolean,
    Number,
    Date,
    String,
    Unknown,
    /// A d3 time format, read in UTC where `utc`.
    Time {
        pattern: String,
        utc: bool,
    },
}

impl Parser {
    fn new(field: &str, type_: &str) -> Result<Self, Error> {
        if let Some((kind @ ("date" | "utc"), pattern)) = type_.split_once(':') {
            let quoted =
                |q: char| pattern.len() >= 2 && pattern.starts_with(q) && pattern.ends_with(q);
            let pattern = match quoted('\'') || quoted('"') {
                true => &pattern[1..pattern.len() - 1],
                false => pattern,
            };
            return Ok(Self::Time {
                pattern: pattern.to_string(),
                utc: kind == "utc",
            });
        }
        Ok(match type_ {
            "boolean" => Self::Boolean,
            "integer" | "number" => Self::Number,
            "date" => Self::Date,
            "string" => Self::String,
            "unknown" => Self::Unknown,
            _ => {
                return Err(Error::Spec(format!(
                    "Illegal format pattern: {field}:{type_}"
                )));
            }
        })
    }

    /// vega-util's `toBoolean`, `toNumber`, `toDate` and `toString`, which
    /// make null and `""` null, or the time format's parse.
    fn parse(&self, value: Value, zone: &LocalZone, utc: &LocalZone) -> Value {
        if let Self::Time {
            pattern,
            utc: in_utc,
        } = self
        {
            let zone = if *in_utc { utc } else { zone };
            return zone
                .time_parse(pattern, &value.to_string())
                .map_or(Value::Null, |d| Value::Number(d.time()));
        }
        if value.is_nullish() || matches!(&value, Value::String(s) if s.is_empty()) {
            return Value::Null;
        }
        match self {
            Self::Unknown => value,
            Self::Boolean => Value::Bool(match &value {
                Value::String(s) if &**s == "false" || &**s == "0" => false,
                v => v.truthy(),
            }),
            Self::Number => Value::Number(value.to_number()),
            Self::Date => match value {
                Value::Number(_) => value,
                v => Value::Number(zone.parse(&v.to_string()).time()),
            },
            Self::String => Value::from(value.to_string()),
            Self::Time { .. } => unreachable!("parsed above"),
        }
    }
}

/// vega-loader's `inferType` of `field` over `data`.
fn infer(data: &[Value], field: &str) -> &'static str {
    const TYPES: [&str; 4] = ["boolean", "integer", "number", "date"];
    let zone = LocalZone::default();
    let mut possible = [true; 4];
    for datum in data {
        let value = datum.get(&field.into()).unwrap_or_default();
        let valid = !value.is_nullish() && !matches!(value, Value::Number(n) if n.is_nan());
        if !valid {
            continue;
        }
        let number = value.to_number();
        let tests = [
            matches!(&value, Value::Bool(_))
                || matches!(&value, Value::String(s) if &**s == "true" || &**s == "false"),
            !number.is_nan() && number.fract() == 0.0 && number.is_finite(),
            !number.is_nan(),
            !zone.parse(&value.to_string()).time().is_nan(),
        ];
        for (p, test) in possible.iter_mut().zip(tests) {
            *p &= test;
        }
        if possible.iter().all(|p| !p) {
            return "string";
        }
    }
    possible
        .iter()
        .position(|p| *p)
        .map_or("string", |i| TYPES[i])
}
