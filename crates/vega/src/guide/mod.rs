//! vega-parser's guides, each a mark spec of the marks it draws.

use serde_json::Value as Json;

pub mod axis;

/// JavaScript's `!!value` on a JSON value.
pub fn truthy(value: &Json) -> bool {
    match value {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Number(n) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Json::String(s) => !s.is_empty(),
        _ => true,
    }
}
