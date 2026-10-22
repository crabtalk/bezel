use crate::path::Path;
use d3::format::number::Number;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value as Json;

/// Encoding sets by name (`enter`, `update`, `exit`, …), each a block of
/// channels.
pub type Encode = IndexMap<String, Block>;

/// Channels by name, in the order written.
pub type Block = IndexMap<String, Channel>;

/// A channel: one value reference, or rules tried in order, each but the
/// last with a `test`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Channel {
    Rules(Vec<Entry>),
    Entry(Box<Entry>),
}

/// A rule of a channel's list.
pub type Rule = Entry;

/// A value reference.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Entry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    /// `Some(Null)` where `value` is written as `null`.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<FieldRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<ScaleRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub band: Option<Operand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exponent: Option<Operand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mult: Option<Operand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<Operand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<bool>,
}

/// A number, or a value reference that computes one.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Operand {
    Entry(Box<Entry>),
    Literal(Json),
}

/// A datum's field by name, or a reference that names one.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FieldRef {
    Name(String),
    Ref(Box<FieldObject>),
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FieldObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub datum: Option<FieldRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<FieldRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<FieldRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<f64>,
}

/// A scale by name, or a reference that names one.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ScaleRef {
    Name(String),
    Ref(Box<FieldObject>),
}

fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Json>, D::Error> {
    Json::deserialize(deserializer).map(Some)
}

/// vega-util's `stringValue`: a string or object as JSON, an array item by
/// item, anything else as JavaScript prints it.
pub fn string_value(value: &Json) -> String {
    match value {
        Json::Array(items) => format!(
            "[{}]",
            items.iter().map(string_value).collect::<Vec<_>>().join(",")
        ),
        Json::String(_) | Json::Object(_) => serde_json::to_string(value)
            .expect("JSON")
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029"),
        Json::Number(n) => Number(n.as_f64().unwrap_or(f64::NAN)).to_string(),
        Json::Bool(b) => b.to_string(),
        Json::Null => "null".into(),
    }
}

impl Channel {
    /// vega-parser's encoder code for the channel: `rule.js` for rules,
    /// `entry.js` for one entry.
    pub fn code(&self) -> String {
        match self {
            Self::Entry(entry) => entry.code(),
            Self::Rules(rules) => {
                let mut code = String::new();
                for rule in rules {
                    let value = rule.code();
                    match &rule.test {
                        Some(test) => code += &format!("({test})?{value}:"),
                        None => code += &value,
                    }
                }
                if code.ends_with(':') {
                    code += "null";
                }
                code
            }
        }
    }
}

impl Entry {
    /// vega-parser's `entry`: the expression the reference evaluates.
    pub fn code(&self) -> String {
        let mut value = if let Some(signal) = &self.signal {
            Some(format!("({signal})"))
        } else if let Some(field) = &self.field {
            Some(field.code())
        } else {
            self.value.as_ref().map(string_value)
        };
        if let Some(scale) = &self.scale {
            value = Some(self.scaled(scale, value));
        }
        let mut value = value.unwrap_or_else(|| "null".into());
        if let Some(exponent) = &self.exponent {
            value = format!("pow({value},{})", exponent.code());
        }
        if let Some(mult) = &self.mult {
            value += &format!("*{}", mult.code());
        }
        if let Some(offset) = &self.offset {
            value += &format!("+{}", offset.code());
        }
        if self.round == Some(true) {
            value = format!("round({value})");
        }
        value
    }

    fn scaled(&self, scale: &ScaleRef, value: Option<String>) -> String {
        let scale = scale.code();
        if let Some(range) = &self.range {
            return format!("lerp(_range({scale}), {})", Number(json_number(range)));
        }
        let mut value = value.map(|value| format!("_scale({scale}, {value})"));
        if let Some(band) = &self.band {
            let mult = match band {
                Operand::Literal(b) if json_number(b) == 1.0 => String::new(),
                band => format!("*{}", band.code()),
            };
            let width = format!("_bandwidth({scale}){mult}");
            let mut banded = match value {
                Some(value) => format!("{value}+{width}"),
                None => width,
            };
            if self.extra.as_ref().is_some_and(truthy) {
                banded = format!("(datum.extra ? _scale({scale}, datum.extra.value) : {banded})");
            }
            value = Some(banded);
        }
        value.unwrap_or_else(|| "0".into())
    }
}

impl Operand {
    /// vega-parser's `property`: a reference's code in parentheses, a
    /// literal as JavaScript prints it.
    fn code(&self) -> String {
        match self {
            Self::Entry(entry) => format!("({})", entry.code()),
            Self::Literal(Json::String(s)) => s.clone(),
            Self::Literal(literal) => string_value(literal),
        }
    }
}

impl FieldRef {
    /// vega-parser's `field`: the expression reading the field.
    fn code(&self) -> String {
        match self {
            Self::Name(name) => FieldObject {
                datum: Some(Self::Name(name.clone())),
                ..FieldObject::default()
            }
            .code(),
            Self::Ref(object) => object.code(),
        }
    }
}

impl FieldObject {
    fn code(&self) -> String {
        let (object, field) = if let Some(signal) = &self.signal {
            return format!("datum[{signal}]");
        } else if self.group.is_some() || self.parent.is_some() {
            let level = self.level.unwrap_or(1.0).max(1.0) as usize;
            let mut object = "item".to_string();
            for _ in 0..level {
                object += ".mark.group";
            }
            match &self.parent {
                Some(parent) => (object + ".datum", parent),
                None => (object, self.group.as_ref().expect("group")),
            }
        } else {
            (
                "datum".to_string(),
                self.datum.as_ref().expect("datum field"),
            )
        };
        let field = match field {
            FieldRef::Name(name) => {
                let keys: Vec<String> = match name.parse::<Path>() {
                    Ok(path) => path.to_vec(),
                    Err(_) => vec![name.clone()],
                };
                keys.iter()
                    .map(|key| string_value(&Json::String(key.clone())))
                    .collect::<Vec<_>>()
                    .join("][")
            }
            FieldRef::Ref(nested) => nested.code(),
        };
        format!("{object}[{field}]")
    }
}

impl ScaleRef {
    /// vega-parser's `scaleRef`.
    fn code(&self) -> String {
        match self {
            Self::Name(name) => string_value(&Json::String(name.clone())),
            Self::Ref(object) => match &object.signal {
                Some(signal) => format!("({signal})"),
                None => object.code(),
            },
        }
    }
}

fn json_number(value: &Json) -> f64 {
    match value {
        Json::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Json::Bool(b) => f64::from(u8::from(*b)),
        Json::String(s) => Number::from(s.as_str()).0,
        Json::Null => 0.0,
        _ => f64::NAN,
    }
}

fn truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Json::String(s) => !s.is_empty(),
        _ => true,
    }
}
