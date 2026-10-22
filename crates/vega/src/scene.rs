//! The scenegraph a view draws: marks of items, as vega-scenegraph holds
//! them, and its JSON as `Scenegraph.toJSON` writes it.

use crate::value::Value;
use indexmap::IndexMap;
use serde_json::{Map, Value as Json};

/// The item properties `toJSON` writes, in its order.
const KEYS: &[&str] = &[
    "marktype",
    "name",
    "role",
    "interactive",
    "clip",
    "items",
    "zindex",
    "x",
    "y",
    "width",
    "height",
    "align",
    "baseline",
    "fill",
    "fillOpacity",
    "opacity",
    "blend",
    "stroke",
    "strokeOpacity",
    "strokeWidth",
    "strokeCap",
    "strokeDash",
    "strokeDashOffset",
    "strokeForeground",
    "strokeOffset",
    "startAngle",
    "endAngle",
    "innerRadius",
    "outerRadius",
    "cornerRadius",
    "padAngle",
    "cornerRadiusTopLeft",
    "cornerRadiusTopRight",
    "cornerRadiusBottomLeft",
    "cornerRadiusBottomRight",
    "interpolate",
    "tension",
    "orient",
    "defined",
    "url",
    "aspect",
    "smooth",
    "path",
    "scaleX",
    "scaleY",
    "x2",
    "y2",
    "size",
    "shape",
    "text",
    "angle",
    "theta",
    "radius",
    "dir",
    "dx",
    "dy",
    "ellipsis",
    "limit",
    "lineBreak",
    "lineHeight",
    "font",
    "fontSize",
    "fontWeight",
    "fontStyle",
    "fontVariant",
    "description",
    "aria",
    "ariaRole",
    "ariaRoleDescription",
];

/// A box, empty until something is added to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            x1: f64::MAX,
            y1: f64::MAX,
            x2: -f64::MAX,
            y2: -f64::MAX,
        }
    }
}

impl Bounds {
    pub fn set(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> &mut Self {
        (self.x1, self.x2) = if x2 < x1 { (x2, x1) } else { (x1, x2) };
        (self.y1, self.y2) = if y2 < y1 { (y2, y1) } else { (y1, y2) };
        self
    }

    pub fn add(&mut self, x: f64, y: f64) -> &mut Self {
        if x < self.x1 {
            self.x1 = x;
        }
        if y < self.y1 {
            self.y1 = y;
        }
        if x > self.x2 {
            self.x2 = x;
        }
        if y > self.y2 {
            self.y2 = y;
        }
        self
    }

    pub fn expand(&mut self, d: f64) -> &mut Self {
        self.x1 -= d;
        self.y1 -= d;
        self.x2 += d;
        self.y2 += d;
        self
    }

    pub fn translate(&mut self, dx: f64, dy: f64) -> &mut Self {
        self.x1 += dx;
        self.x2 += dx;
        self.y1 += dy;
        self.y2 += dy;
        self
    }

    /// The corners turned `angle` radians about `(x, y)`, as `x, y` pairs.
    pub fn rotated_points(&self, angle: f64, x: f64, y: f64) -> [f64; 8] {
        let (cos, sin) = (angle.cos(), angle.sin());
        let cx = x - x * cos + y * sin;
        let cy = y - x * sin - y * cos;
        let Self { x1, y1, x2, y2 } = *self;
        [
            cos * x1 - sin * y1 + cx,
            sin * x1 + cos * y1 + cy,
            cos * x1 - sin * y2 + cx,
            sin * x1 + cos * y2 + cy,
            cos * x2 - sin * y1 + cx,
            sin * x2 + cos * y1 + cy,
            cos * x2 - sin * y2 + cx,
            sin * x2 + cos * y2 + cy,
        ]
    }

    pub fn rotate(&mut self, angle: f64, x: f64, y: f64) -> &mut Self {
        let p = self.rotated_points(angle, x, y);
        *self = Self::default();
        self.add(p[0], p[1])
            .add(p[2], p[3])
            .add(p[4], p[5])
            .add(p[6], p[7])
    }

    pub fn union(&mut self, b: &Bounds) -> &mut Self {
        if b.x1 < self.x1 {
            self.x1 = b.x1;
        }
        if b.y1 < self.y1 {
            self.y1 = b.y1;
        }
        if b.x2 > self.x2 {
            self.x2 = b.x2;
        }
        if b.y2 > self.y2 {
            self.y2 = b.y2;
        }
        self
    }

    pub fn intersect(&mut self, b: &Bounds) -> &mut Self {
        if b.x1 > self.x1 {
            self.x1 = b.x1;
        }
        if b.y1 > self.y1 {
            self.y1 = b.y1;
        }
        if b.x2 < self.x2 {
            self.x2 = b.x2;
        }
        if b.y2 < self.y2 {
            self.y2 = b.y2;
        }
        self
    }

    pub fn encloses(&self, b: &Bounds) -> bool {
        self.x1 <= b.x1 && self.x2 >= b.x2 && self.y1 <= b.y1 && self.y2 >= b.y2
    }

    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }
}

/// A mark: its items, all of one `marktype`.
#[derive(Clone, Debug, Default)]
pub struct Mark {
    pub marktype: String,
    pub name: Option<String>,
    pub role: String,
    pub interactive: bool,
    pub clip: bool,
    pub zindex: f64,
    pub aria: Option<bool>,
    pub description: Option<String>,
    pub items: Vec<Item>,
    pub bounds: Bounds,
}

/// An item: the properties its encoders set, by name, and the datum it
/// draws. A group item holds marks of its own.
#[derive(Clone, Debug, Default)]
pub struct Item {
    pub props: IndexMap<String, Value>,
    pub datum: Value,
    pub items: Vec<Mark>,
    pub bounds: Bounds,
}

impl Item {
    pub fn get(&self, name: &str) -> &Value {
        static UNDEFINED: Value = Value::Undefined;
        self.props.get(name).unwrap_or(&UNDEFINED)
    }

    /// The property as a number, `None` where it is unset or `null`.
    pub fn number(&self, name: &str) -> Option<f64> {
        match self.get(name) {
            Value::Undefined | Value::Null => None,
            value => Some(value.to_number()),
        }
    }

    /// `item[name] || 0`.
    pub fn or_zero(&self, name: &str) -> f64 {
        let value = self.get(name);
        match value.truthy() {
            true => value.to_number(),
            false => 0.0,
        }
    }

    pub fn set(&mut self, name: &str, value: Value) {
        self.props.insert(name.to_string(), value);
    }

    /// The item as a JavaScript object, for an expression that reads `item`.
    pub fn value(&self) -> Value {
        Value::Object(
            self.props
                .iter()
                .map(|(name, value)| (name.as_str().into(), value.clone()))
                .collect(),
        )
    }
}

impl Mark {
    /// The mark as `toJSON` writes it.
    pub fn to_json(&self) -> Json {
        let mut out = Map::new();
        for &key in KEYS {
            let value = match key {
                "marktype" => Json::String(self.marktype.clone()),
                "name" => match &self.name {
                    Some(name) => Json::String(name.clone()),
                    None => continue,
                },
                "role" => Json::String(self.role.clone()),
                "interactive" => Json::Bool(self.interactive),
                "clip" => Json::Bool(self.clip),
                "items" => Json::Array(
                    self.items
                        .iter()
                        .map(|item| item.to_json(self.marktype == "group"))
                        .collect(),
                ),
                "zindex" => json_number(self.zindex),
                "aria" => match self.aria {
                    Some(aria) => Json::Bool(aria),
                    None => continue,
                },
                "description" => match &self.description {
                    Some(description) => Json::String(description.clone()),
                    None => continue,
                },
                _ => continue,
            };
            out.insert(key.to_string(), value);
        }
        Json::Object(out)
    }
}

impl Item {
    fn to_json(&self, group: bool) -> Json {
        let mut out = Map::new();
        for &key in KEYS {
            if key == "items" && group {
                out.insert(
                    key.to_string(),
                    Json::Array(self.items.iter().map(Mark::to_json).collect()),
                );
                continue;
            }
            if let Some(value) = self.props.get(key)
                && !matches!(value, Value::Undefined)
            {
                out.insert(key.to_string(), json(value));
            }
        }
        Json::Object(out)
    }
}

fn json_number(n: f64) -> Json {
    match serde_json::Number::from_f64(n) {
        Some(n) => Json::Number(n),
        None => Json::Null,
    }
}

/// `value` as `JSON.stringify` writes it, under the replacer's keys.
fn json(value: &Value) -> Json {
    match value {
        Value::Undefined | Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Number(n) => json_number(*n),
        Value::String(s) => Json::String(s.to_string()),
        Value::Array(items) => Json::Array(items.iter().map(json).collect()),
        Value::Object(fields) => Json::Object(
            fields
                .iter()
                .filter(|(key, value)| KEYS.contains(&&**key) && !matches!(value, Value::Undefined))
                .map(|(key, value)| (key.to_string(), json(value)))
                .collect(),
        ),
        Value::RegExp(_) => Json::Object(Map::new()),
    }
}
