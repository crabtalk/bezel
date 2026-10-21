//! JavaScript values and the abstract operations that convert between them.

use crate::expr::{Error, RegExp};
use gpui::SharedString;
use js::{
    number::{self, Number},
    string,
};
use std::{cmp::Ordering, fmt, ops::Add, sync::Arc};

/// A JavaScript value. Arrays, objects and regular expressions are references:
/// a clone is the same value to [`Value::strict_eq`].
#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(SharedString),
    Array(Arc<[Value]>),
    /// Keys in insertion order, each once.
    Object(Arc<[(SharedString, Value)]>),
    RegExp(Arc<RegExp>),
}

/// Structural: arrays and objects by contents, numbers as `f64`.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Undefined, Self::Undefined) | (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::RegExp(a), Self::RegExp(b)) => a.source == b.source && a.flags == b.flags,
            _ => false,
        }
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(SharedString::from(value.to_string()))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::String(value.into())
    }
}

impl From<SharedString> for Value {
    fn from(value: SharedString) -> Self {
        Self::String(value)
    }
}

impl From<Vec<Value>> for Value {
    fn from(values: Vec<Value>) -> Self {
        Self::Array(values.into())
    }
}

impl From<RegExp> for Value {
    fn from(value: RegExp) -> Self {
        Self::RegExp(Arc::new(value))
    }
}

impl Value {
    /// JavaScript's `!!value`.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Bool(b) => *b,
            Self::Number(n) => number::truthy(*n),
            Self::String(s) => !s.is_empty(),
            Self::Array(_) | Self::Object(_) | Self::RegExp(_) => true,
        }
    }

    /// `null` or `undefined`.
    pub fn is_nullish(&self) -> bool {
        matches!(self, Self::Undefined | Self::Null)
    }

    /// An array, an object or a regular expression.
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Array(_) | Self::Object(_) | Self::RegExp(_))
    }

    /// JavaScript's `+value`.
    pub fn to_number(&self) -> f64 {
        match self {
            Self::Undefined => f64::NAN,
            Self::Null => 0.0,
            Self::Bool(b) => f64::from(u8::from(*b)),
            Self::Number(n) => *n,
            Self::String(s) => Number::from(s.as_ref()).0,
            Self::Array(_) | Self::Object(_) | Self::RegExp(_) => self.to_primitive().to_number(),
        }
    }

    /// JavaScript's ToInt32: `value | 0`.
    pub fn to_int32(&self) -> i32 {
        number::to_int32(self.to_number())
    }

    /// JavaScript's ToUint32: `value >>> 0`.
    pub fn to_uint32(&self) -> u32 {
        self.to_int32() as u32
    }

    /// JavaScript's ToIntegerOrInfinity: `NaN` is 0, the rest truncate.
    pub fn to_integer(&self) -> f64 {
        number::to_integer(self.to_number())
    }

    /// JavaScript's ToPrimitive with no hint. Every value here but an array,
    /// object or regular expression is already primitive; those become their
    /// string.
    pub fn to_primitive(&self) -> Value {
        match self {
            Self::Array(_) | Self::Object(_) | Self::RegExp(_) => self.to_string().into(),
            _ => self.clone(),
        }
    }

    /// JavaScript's `===`.
    pub fn strict_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Self::Array(a), Self::Array(b)) => Arc::ptr_eq(a, b),
            (Self::Object(a), Self::Object(b)) => Arc::ptr_eq(a, b),
            (Self::RegExp(a), Self::RegExp(b)) => Arc::ptr_eq(a, b),
            (Self::Array(_) | Self::Object(_) | Self::RegExp(_), _) => false,
            _ => self == other,
        }
    }

    /// JavaScript's `==`.
    pub fn loose_eq(&self, other: &Value) -> bool {
        match (self, other) {
            _ if std::mem::discriminant(self) == std::mem::discriminant(other) => {
                self.strict_eq(other)
            }
            (Self::Undefined | Self::Null, Self::Undefined | Self::Null) => true,
            (Self::Number(_), Self::String(_)) | (Self::String(_), Self::Number(_)) => {
                self.to_number() == other.to_number()
            }
            (Self::Bool(_), _) => Value::Number(self.to_number()).loose_eq(other),
            (_, Self::Bool(_)) => self.loose_eq(&Value::Number(other.to_number())),
            (Self::Number(_) | Self::String(_), _) if other.is_object() => {
                self.loose_eq(&other.to_primitive())
            }
            (_, Self::Number(_) | Self::String(_)) if self.is_object() => {
                self.to_primitive().loose_eq(other)
            }
            _ => false,
        }
    }

    /// JavaScript's `<` ordering after ToPrimitive: strings by UTF-16 code
    /// unit, anything else as numbers. `None` when either number is `NaN`.
    pub fn compare(&self, other: &Value) -> Option<Ordering> {
        match (self.to_primitive(), other.to_primitive()) {
            (Self::String(a), Self::String(b)) => Some(string::compare(&a, &b)),
            (a, b) => a.to_number().partial_cmp(&b.to_number()),
        }
    }

    /// JavaScript's `object[key]`.
    pub fn get(&self, key: &Value) -> Result<Value, Error> {
        let name = key.to_string();
        Ok(match self {
            Self::Undefined | Self::Null => {
                return Err(Error::Runtime(format!(
                    "Cannot read properties of {self} (reading '{name}')"
                )));
            }
            Self::String(s) if name == "length" => Value::Number(string::len(s) as f64),
            Self::String(s) => match index(&name) {
                Some(i) => s.encode_utf16().nth(i).map_or(Value::Undefined, |unit| {
                    String::from_utf16_lossy(&[unit]).into()
                }),
                None => Value::Undefined,
            },
            Self::Array(items) if name == "length" => Value::Number(items.len() as f64),
            Self::Array(items) => index(&name)
                .and_then(|i| items.get(i).cloned())
                .unwrap_or_default(),
            Self::Object(fields) => fields
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            Self::RegExp(regexp) => match name.as_str() {
                "source" => regexp.source.as_str().into(),
                "flags" => regexp.flags.as_str().into(),
                "global" => regexp.global().into(),
                _ => Value::Undefined,
            },
            Self::Bool(_) | Self::Number(_) => Value::Undefined,
        })
    }
}

/// JavaScript's `+`: concatenation if either side's primitive is a string,
/// addition otherwise.
impl Add for &Value {
    type Output = Value;

    fn add(self, other: &Value) -> Value {
        match (self.to_primitive(), other.to_primitive()) {
            (a @ Value::String(_), b) | (a, b @ Value::String(_)) => format!("{a}{b}").into(),
            (a, b) => Value::Number(a.to_number() + b.to_number()),
        }
    }
}

/// `name` as an array index: a canonical non-negative integer.
fn index(name: &str) -> Option<usize> {
    let i: usize = name.parse().ok()?;
    (i.to_string() == name).then_some(i)
}

/// JavaScript's ToString.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Undefined => f.write_str("undefined"),
            Self::Null => f.write_str("null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Number(n) => write!(f, "{}", Number(*n)),
            Self::String(s) => f.write_str(s),
            Self::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    if !item.is_nullish() {
                        write!(f, "{item}")?;
                    }
                }
                Ok(())
            }
            Self::Object(_) => f.write_str("[object Object]"),
            Self::RegExp(regexp) => write!(f, "{regexp}"),
        }
    }
}
