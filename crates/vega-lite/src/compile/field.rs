//! `channeldef.ts`'s field names and titles.

use crate::spec::FieldDef;
use serde_json::Value as Json;
use vega::{path::Path, spec::string_value};

/// `FieldRefOption`.
#[derive(Clone, Debug, Default)]
pub struct Options<'a> {
    /// The object a field is read from, as `datum`, giving
    /// `datum["field"]`.
    pub expr: Option<&'a str>,
    pub suffix: Option<&'a str>,
}

/// The keys of `path`, as vega-util's `splitAccessPath` reads them.
fn keys(path: &str) -> Vec<String> {
    match path.parse::<Path>() {
        Ok(path) => path.to_vec(),
        Err(_) => vec![path.to_string()],
    }
}

/// `vgField`: the field a definition derives, written as `opt` asks.
pub fn vg_field(def: &FieldDef, opt: &Options) -> String {
    let mut field = def.field.clone();
    if let Some(suffix) = opt.suffix {
        field = format!("{field}_{suffix}");
    }
    match opt.expr {
        // `flatAccessWithDatum`.
        Some(expr) => format!(
            "{expr}[{}]",
            string_value(&Json::String(keys(&field).join(".")))
        ),
        // `replacePathInField`.
        None => keys(&field)
            .iter()
            .map(|key| {
                let mut escaped = String::new();
                for c in key.chars() {
                    if matches!(c, '[' | ']' | '.' | '\'' | '"') {
                        escaped.push('\\');
                    }
                    escaped.push(c);
                }
                escaped
            })
            .collect::<Vec<_>>()
            .join("\\."),
    }
}

/// `defaultTitle`, with the verbal formatter.
pub fn default_title(def: &FieldDef) -> String {
    def.field.clone()
}

/// The title a field definition writes for its channel: its own, or the
/// default one.
pub fn title(def: &FieldDef) -> Json {
    match &def.title {
        Some(title) => title.clone(),
        None => Json::String(default_title(def)),
    }
}
