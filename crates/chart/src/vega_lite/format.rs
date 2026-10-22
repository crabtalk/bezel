//! A data source's `format`: how its text becomes rows, as vega-loader reads
//! it, and the `parse` applied to them.

use crate::{path::Path, time, vega_lite::dsv};

use crate::value::Value as JsValue;
use serde_json::{Map, Number, Value};
use std::str::FromStr;

/// A row: one datum, its fields by name.
pub(crate) type Row = Map<String, Value>;

pub(crate) struct Format {
    kind: Kind,
    /// Fields in the order `parse` names them.
    parse: Vec<(String, Parse)>,
}

enum Kind {
    Json(Option<Path>),
    Dsv(char),
    /// `property`, then the TopoJSON object read as features or as a mesh.
    Topo(Option<Path>, Topo),
}

enum Topo {
    Feature(String),
    Mesh(String),
}

/// A `parse` type: vega-loader's `typeParsers`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parse {
    Boolean,
    Number,
    Date,
    String,
    Unknown,
}

impl FromStr for Parse {
    /// A type vega-loader lacks, and `date:` or `utc:` with a d3 time format.
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        match name {
            "boolean" => Ok(Self::Boolean),
            "number" | "integer" => Ok(Self::Number),
            "date" => Ok(Self::Date),
            "string" => Ok(Self::String),
            "unknown" => Ok(Self::Unknown),
            _ => Err(()),
        }
    }
}

impl Parse {
    /// vega-util's `toBoolean`, `toNumber`, `toDate` and `toString`: `null`
    /// and `""` become `null`, a number that JSON cannot hold `null`, and a
    /// date milliseconds since the epoch as [`time::parse`] reads it.
    fn apply(self, value: Value) -> Value {
        if matches!(&value, Value::Null) || value.as_str() == Some("") {
            return Value::Null;
        }
        match self {
            Self::Unknown => value,
            Self::Boolean => Value::Bool(match &value {
                Value::Bool(flag) => *flag,
                Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
                Value::String(s) => s != "false" && s != "0",
                _ => true,
            }),
            Self::Number => number(js(&value).to_number()),
            Self::Date => match &value {
                Value::Number(_) => value,
                Value::String(s) => number(time::parse(s).unwrap_or(f64::NAN)),
                _ => Value::Null,
            },
            Self::String => Value::String(match value {
                Value::String(s) => s,
                other => js(&other).to_string(),
            }),
        }
    }
}

/// The JSON as a JavaScript value, an object's keys in their order.
pub(crate) fn js(value: &Value) -> JsValue {
    match value {
        Value::Null => JsValue::Null,
        Value::Bool(flag) => JsValue::Bool(*flag),
        Value::Number(n) => JsValue::Number(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.as_str().into(),
        Value::Array(items) => JsValue::Array(items.iter().map(js).collect()),
        Value::Object(object) => JsValue::Object(
            object
                .iter()
                .map(|(key, value)| (key.as_str().into(), js(value)))
                .collect(),
        ),
    }
}

fn number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

impl Format {
    /// `format` as written, its type defaulting to the extension of `url` when
    /// that is one vega-loader reads, else to `json`. `None` for a type or key
    /// the format lacks, and for a `date:` or `utc:` parse with a time format.
    pub(crate) fn new(format: Option<&Value>, url: Option<&str>) -> Option<Self> {
        let empty = Map::new();
        let format = match format {
            Some(format) => format.as_object()?,
            None => &empty,
        };
        let extension = url
            .and_then(|url| url.rsplit_once('.'))
            .map(|(_, extension)| extension)
            .filter(|extension| ["json", "csv", "tsv", "dsv", "topojson"].contains(extension));
        let name = match format.get("type") {
            Some(name) => name.as_str()?,
            None => extension.unwrap_or("json"),
        };
        let takes = |keys: &[&str]| {
            format.keys().all(|key| {
                ["type", "parse"].contains(&key.as_str()) || keys.contains(&key.as_str())
            })
        };
        let path = |key: &str| -> Option<Option<Path>> {
            match format.get(key) {
                None => Some(None),
                Some(path) => Some(Some(path.as_str()?.parse().ok()?)),
            }
        };
        let kind = match name {
            "json" if takes(&["property"]) => Kind::Json(path("property")?),
            "csv" if takes(&[]) => Kind::Dsv(','),
            "tsv" if takes(&[]) => Kind::Dsv('\t'),
            "dsv" if takes(&["delimiter"]) => {
                let mut delimiter = format.get("delimiter")?.as_str()?.chars();
                let (Some(delimiter), None) = (delimiter.next(), delimiter.next()) else {
                    return None;
                };
                Kind::Dsv(delimiter)
            }
            "topojson" if takes(&["property", "feature", "mesh", "filter"]) => {
                let object = |key: &str| Some(format.get(key)?.as_str()?.to_owned());
                let topo = match (object("feature"), object("mesh")) {
                    (Some(feature), _) => Topo::Feature(feature),
                    (None, Some(mesh)) => Topo::Mesh(mesh),
                    (None, None) => return None,
                };
                Kind::Topo(path("property")?, topo)
            }
            _ => return None,
        };
        let parse = match format.get("parse") {
            None | Some(Value::Null) => Vec::new(),
            Some(parse) => parse
                .as_object()?
                .iter()
                .map(|(field, name)| Some((field.clone(), name.as_str()?.parse().ok()?)))
                .collect::<Option<_>>()?,
        };
        Some(Self { kind, parse })
    }

    /// The rows `text` holds. `None` for text that is not this format.
    pub(crate) fn read(&self, text: &str) -> Option<Vec<Row>> {
        match self.kind {
            Kind::Dsv(delimiter) => Some(self.parsed(dsv::parse(text, delimiter))),
            _ => self.values(&serde_json::from_str(text).ok()?),
        }
    }

    /// The rows inline `values` hold: text read as [`Self::read`] reads it,
    /// anything else as parsed JSON.
    pub(crate) fn values(&self, values: &Value) -> Option<Vec<Row>> {
        if let Value::String(text) = values {
            return self.read(text);
        }
        let rows = match &self.kind {
            Kind::Dsv(_) => return None,
            Kind::Json(property) => rows(at(values, property.as_ref())?),
            Kind::Topo(property, topo) => topo.rows(at(values, property.as_ref())?)?,
        };
        Some(self.parsed(rows))
    }

    fn parsed(&self, mut rows: Vec<Row>) -> Vec<Row> {
        if rows.is_empty() {
            return rows;
        }
        for row in &mut rows {
            for (field, parse) in &self.parse {
                let value = row.remove(field).unwrap_or(Value::Null);
                row.insert(field.clone(), parse.apply(value));
            }
        }
        rows
    }
}

fn at<'a>(value: &'a Value, property: Option<&Path>) -> Option<&'a Value> {
    match property {
        Some(path) => path.json(value),
        None => Some(value),
    }
}

/// Vega's `ingest`: an array's items, or the one value that is not an array;
/// an item that is not an object as `{"data": item}`.
pub(crate) fn rows(values: &Value) -> Vec<Row> {
    let items = match values {
        Value::Array(items) => items.as_slice(),
        value => std::slice::from_ref(value),
    };
    items
        .iter()
        .map(|item| match item {
            Value::Object(row) => row.clone(),
            item => Map::from_iter([("data".to_owned(), item.clone())]),
        })
        .collect()
}

impl Topo {
    /// topojson-client's `feature` or `mesh` on the named object, without
    /// geometry: a feature's `type`, `id` and `properties`, and a mesh's
    /// `type`. `None` for a topology without the object.
    fn rows(&self, topology: &Value) -> Option<Vec<Row>> {
        let objects = topology.get("objects")?.as_object()?;
        let row = |pairs: &[(&str, Value)]| -> Row {
            pairs
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect()
        };
        match self {
            Self::Mesh(name) => {
                objects.get(name)?;
                Some(vec![row(&[("type", "MultiLineString".into())])])
            }
            Self::Feature(name) => {
                let object = objects.get(name)?;
                let geometries = match object.get("type").and_then(Value::as_str) {
                    Some("GeometryCollection") => object.get("geometries")?.as_array()?.as_slice(),
                    _ => std::slice::from_ref(object),
                };
                Some(
                    geometries
                        .iter()
                        .map(|geometry| {
                            let mut feature = row(&[("type", "Feature".into())]);
                            if let Some(id) = geometry.get("id").filter(|id| !id.is_null()) {
                                feature.insert("id".into(), id.clone());
                            }
                            let properties = match geometry.get("properties") {
                                None | Some(Value::Null) => Value::Object(Map::new()),
                                Some(properties) => properties.clone(),
                            };
                            feature.insert("properties".into(), properties);
                            feature
                        })
                        .collect(),
                )
            }
        }
    }
}
