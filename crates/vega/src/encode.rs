//! A mark's encoders, as vega-parser compiles them: each channel's value
//! reference an expression, with the config's and styles' defaults added.

use crate::{
    Error,
    config::Config,
    expr::{Context, Expr},
    scale::Scale,
    scene::Item,
    spec::{self, Channel, Entry},
    value::Value,
};
use indexmap::IndexMap;
use serde_json::Value as Json;

/// A set's channels, in order, with the spatial adjustment its channels
/// call for.
#[derive(Clone, Debug, Default)]
struct Set {
    channels: Vec<(String, Expr)>,
    has: [bool; 6],
}

const X: usize = 0;
const X2: usize = 1;
const XC: usize = 2;
const Y: usize = 3;
const Y2: usize = 4;
const YC: usize = 5;

/// A mark's encoding sets by name.
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    marktype: String,
    sets: IndexMap<String, Set>,
}

impl Encoder {
    /// The encoders of a `marktype` mark of `role`, its `encode` under the
    /// defaults of its `style`s and the config.
    pub fn new(
        encode: &spec::Encode,
        marktype: &str,
        role: &str,
        style: &[String],
        config: &Config,
    ) -> Result<Self, Error> {
        let encode = defaults(encode, marktype, role, style, config);
        let mut sets = IndexMap::new();
        for (name, block) in &encode {
            let mut set = Set::default();
            for (channel, value) in block {
                let code = value.code();
                let expr: Expr = code
                    .parse()
                    .map_err(|e| Error::Expression(code.clone(), e))?;
                if let Some(i) = ["x", "x2", "xc", "y", "y2", "yc"]
                    .iter()
                    .position(|c| c == channel)
                {
                    set.has[i] = true;
                }
                set.channels.push((channel.clone(), expr));
            }
            sets.insert(name.clone(), set);
        }
        Ok(Self {
            marktype: marktype.to_string(),
            sets,
        })
    }

    /// Runs the set named `name` on `item`, reading signals through
    /// `signal` and scales through `scale`.
    pub fn apply<'s>(
        &self,
        name: &str,
        item: &mut Item,
        signal: &dyn Fn(&str) -> Value,
        scale: &dyn Fn(&str) -> Option<&'s Scale>,
    ) -> Result<(), Error> {
        let Some(set) = self.sets.get(name) else {
            return Ok(());
        };
        for (channel, expr) in &set.channels {
            let value = {
                let item_ref = &*item;
                let param = |name: &str| match name {
                    "item" => item_ref.value(),
                    _ => signal(name),
                };
                let context = Context {
                    datum: &item_ref.datum,
                    param: &param,
                    scale,
                };
                expr.eval(&context).map_err(Error::Eval)?
            };
            item.set(channel, value);
        }
        self.adjust(set, item);
        Ok(())
    }

    /// vega-runtime's `adjustSpatial`.
    fn adjust(&self, set: &Set, o: &mut Item) {
        if self.marktype == "rule" {
            return;
        }
        let swap = matches!(self.marktype.as_str(), "group" | "image" | "rect");
        let num = |o: &Item, name: &str| o.get(name).to_number();
        let width_or_zero = |o: &Item, name: &str| o.or_zero(name);
        for (start, end, center, size) in [(X, X2, XC, "width"), (Y, Y2, YC, "height")] {
            let (p, p2, pc) = match start {
                X => ("x", "x2", "xc"),
                _ => ("y", "y2", "yc"),
            };
            if set.has[end] {
                if set.has[start] {
                    if swap && o.get(p).compare(o.get(p2)) == Some(std::cmp::Ordering::Greater) {
                        let (a, b) = (o.get(p).clone(), o.get(p2).clone());
                        o.set(p, b);
                        o.set(p2, a);
                    }
                    let extent = num(o, p2) - num(o, p);
                    o.set(size, Value::Number(extent));
                } else {
                    let at = num(o, p2) - width_or_zero(o, size);
                    o.set(p, Value::Number(at));
                }
            }
            if set.has[center] {
                let at = num(o, pc) - width_or_zero(o, size) / 2.0;
                o.set(p, Value::Number(at));
            }
        }
    }
}

/// Whether `encode` sets `key` in `enter` or `update`.
fn has(key: &str, encode: &spec::Encode) -> bool {
    ["enter", "update"].iter().any(|set| {
        encode
            .get(*set)
            .is_some_and(|block| block.contains_key(key))
    })
}

fn default_entry(value: &Json) -> Channel {
    Channel::Entry(Box::new(match value {
        Json::Object(object) if object.contains_key("signal") => Entry {
            signal: object["signal"].as_str().map(str::to_string),
            ..Entry::default()
        },
        value => Entry {
            value: Some(value.clone()),
            ..Entry::default()
        },
    }))
}

/// vega-parser's `encode/defaults.js`.
fn defaults(
    encode: &spec::Encode,
    marktype: &str,
    role: &str,
    style: &[String],
    config: &Config,
) -> spec::Encode {
    let mut defaults: IndexMap<String, Json> = IndexMap::new();
    if marktype == "text"
        && let Some(line_break) = config.0.get("lineBreak").filter(|v| !v.is_null())
        && !has("lineBreak", encode)
    {
        defaults.insert("lineBreak".into(), line_break.clone());
    }
    let role = match role {
        "legend" => "",
        role if role.starts_with("axis") => "",
        role => role,
    };
    let mut props = serde_json::Map::new();
    match role {
        "frame" => props.extend(config.group("group").clone()),
        "mark" => {
            props.extend(config.group("mark").clone());
            props.extend(config.group(marktype).clone());
        }
        _ => {}
    }
    for (key, value) in &props {
        let skip = has(key, encode)
            || ((key == "fill" || key == "stroke")
                && (has("fill", encode) || has("stroke", encode)));
        if !skip {
            defaults.insert(key.clone(), value.clone());
        }
    }
    for name in style {
        for (key, value) in config.style(name) {
            if !has(key, encode) {
                defaults.insert(key.clone(), value.clone());
            }
        }
    }
    let mut encode = encode.clone();
    let (mut enter, mut update) = (IndexMap::new(), IndexMap::new());
    for (key, value) in &defaults {
        let signal = matches!(value, Json::Object(o) if o.contains_key("signal"));
        match signal {
            true => update.insert(key.clone(), default_entry(value)),
            false => enter.insert(key.clone(), default_entry(value)),
        };
    }
    if let Some(block) = encode.get("enter") {
        enter.extend(block.clone());
    }
    encode.insert("enter".into(), enter);
    if !update.is_empty() {
        if let Some(block) = encode.get("update") {
            update.extend(block.clone());
        }
        encode.insert("update".into(), update);
    }
    encode
}
