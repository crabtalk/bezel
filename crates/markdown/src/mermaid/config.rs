//! A diagram's configuration as front matter and `%%{init}%%` directives give
//! it: JSON-shaped values, keys in the order they were written.

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

use super::js;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Value>),
    Map(Map),
}

/// An object: keys in insertion order, a key written again keeping its place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Map(Vec<(String, Value)>);

/// A diagram's configuration.
pub type Config = Map;

impl Map {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        let key = key.into();
        match self.get_mut(&key) {
            Some(slot) => *slot = value,
            None => self.0.push((key, value)),
        }
    }

    pub fn contains(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The object under `key`, if that is what it holds.
    pub fn section(&self, key: &str) -> Option<&Map> {
        self.get(key).and_then(Value::as_map)
    }

    /// The object under `key`, made empty when it is missing or not an object.
    pub fn section_mut(&mut self, key: &str) -> &mut Map {
        if !matches!(self.get(key), Some(Value::Map(_))) {
            self.insert(key, Value::Map(Map::new()));
        }
        match self.get_mut(key) {
            Some(Value::Map(map)) => map,
            _ => unreachable!(),
        }
    }

    /// Mermaid's deep merge of `source` into `self`: objects merge, anything
    /// else replaces, and the keys that reach an object's prototype are
    /// dropped.
    pub fn merge(&mut self, source: &Map) {
        for (key, value) in source.iter() {
            if matches!(key, "__proto__" | "constructor" | "prototype") {
                continue;
            }
            match (value, self.get_mut(key)) {
                (Value::Map(from), Some(Value::Map(current))) => current.merge(from),
                (Value::Map(from), _) => {
                    let mut fresh = Map::new();
                    fresh.merge(from);
                    self.insert(key, Value::Map(fresh));
                }
                (other, _) => self.insert(key, other.clone()),
            }
        }
    }
}

impl Value {
    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Self::Map(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// JavaScript's truth of the value.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Number(n) => js::truthy(*n),
            Self::String(text) => !text.is_empty(),
            Self::List(_) | Self::Map(_) => true,
        }
    }

    /// `String(value)`.
    pub fn to_js_string(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => js::num_str(*n),
            Self::String(text) => text.clone(),
            Self::List(items) => items
                .iter()
                .map(|item| match item {
                    Self::Null => String::new(),
                    other => other.to_js_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            Self::Map(_) => "[object Object]".into(),
        }
    }

    /// `Number(value)`.
    pub fn to_number(&self) -> f64 {
        match self {
            Self::Null => 0.0,
            Self::Bool(b) => f64::from(u8::from(*b)),
            Self::Number(n) => *n,
            Self::String(text) => js::number(text),
            Self::List(items) => match items.as_slice() {
                [] => 0.0,
                [one] => one.to_number(),
                _ => f64::NAN,
            },
            Self::Map(_) => f64::NAN,
        }
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(b) => serializer.serialize_bool(*b),
            Self::Number(n) => js::json_number(n, serializer),
            Self::String(text) => serializer.serialize_str(text),
            Self::List(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Self::Map(map) => map.serialize(serializer),
        }
    }
}

impl Serialize for Map {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            out.serialize_entry(key, value)?;
        }
        out.end()
    }
}

/// `JSON.parse(text)`, or `None` where it would throw.
pub fn parse_json(text: &str) -> Option<Value> {
    let mut reader = Json {
        bytes: text.as_bytes(),
        text,
        at: 0,
    };
    let value = reader.value(0)?;
    reader.space();
    (reader.at == text.len()).then_some(value)
}

struct Json<'t> {
    bytes: &'t [u8],
    text: &'t str,
    at: usize,
}

impl Json<'_> {
    fn space(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > 512 {
            return None;
        }
        self.space();
        match *self.bytes.get(self.at)? {
            b'{' => {
                self.at += 1;
                let mut map = Map::new();
                self.space();
                if self.bytes.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Some(Value::Map(map));
                }
                loop {
                    self.space();
                    let key = self.string()?;
                    self.space();
                    if self.bytes.get(self.at) != Some(&b':') {
                        return None;
                    }
                    self.at += 1;
                    let value = self.value(depth + 1)?;
                    map.insert(key, value);
                    self.space();
                    match self.bytes.get(self.at)? {
                        b',' => self.at += 1,
                        b'}' => {
                            self.at += 1;
                            return Some(Value::Map(map));
                        }
                        _ => return None,
                    }
                }
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.bytes.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Some(Value::List(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.space();
                    match self.bytes.get(self.at)? {
                        b',' => self.at += 1,
                        b']' => {
                            self.at += 1;
                            return Some(Value::List(items));
                        }
                        _ => return None,
                    }
                }
            }
            b'"' => self.string().map(Value::String),
            b't' => self.word("true", Value::Bool(true)),
            b'f' => self.word("false", Value::Bool(false)),
            b'n' => self.word("null", Value::Null),
            _ => self.number(),
        }
    }

    fn word(&mut self, word: &str, value: Value) -> Option<Value> {
        if self.text[self.at..].starts_with(word) {
            self.at += word.len();
            Some(value)
        } else {
            None
        }
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        match self.bytes.get(self.at)? {
            b'0' => self.at += 1,
            b'1'..=b'9' => {
                while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
            }
            _ => return None,
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            let from = self.at;
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if self.at == from {
                return None;
            }
        }
        if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            let from = self.at;
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                self.at += 1;
            }
            if self.at == from {
                return None;
            }
        }
        self.text[start..self.at].parse().ok().map(Value::Number)
    }

    fn string(&mut self) -> Option<String> {
        if self.bytes.get(self.at) != Some(&b'"') {
            return None;
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let c = self.text[self.at..].chars().next()?;
            self.at += c.len_utf8();
            match c {
                '"' => return Some(out),
                '\\' => {
                    let e = *self.bytes.get(self.at)?;
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            if (0xd800..0xdc00).contains(&unit)
                                && self.text[self.at..].starts_with("\\u")
                            {
                                let save = self.at;
                                self.at += 2;
                                let low = self.hex4()?;
                                if (0xdc00..0xe000).contains(&low) {
                                    let code = 0x10000 + ((unit - 0xd800) << 10) + (low - 0xdc00);
                                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                                    continue;
                                }
                                self.at = save;
                            }
                            out.push(char::from_u32(unit).unwrap_or('\u{fffd}'));
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => out.push(c),
            }
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.text.get(self.at..self.at + 4)?;
        let code = u32::from_str_radix(digits, 16).ok()?;
        self.at += 4;
        Some(code)
    }
}
