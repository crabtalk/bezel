//! A spec's views as Vega-Lite normalizes them: a unit, or a layer of
//! views, each with its own `data` and transforms. A layer's `encoding`
//! goes under each of its units' own, and a composite mark becomes the
//! layer of its parts.

use crate::vega_lite::{composite::Composite, dataflow::Step, encoding, transform};
use serde_json::{Map, Value};

/// Inside a layer as well as at the top.
const UNSUPPORTED: [&str; 7] = [
    "hconcat", "vconcat", "concat", "facet", "repeat", "spec", "params",
];

#[derive(Clone, Debug)]
pub(crate) struct View {
    /// Its own data; `None` reads its parent layer's rows.
    pub(crate) data: Option<Value>,
    pub(crate) transform: Vec<Step>,
    pub(crate) body: Body,
}

#[derive(Clone, Debug)]
pub(crate) enum Body {
    Unit {
        mark: Value,
        encoding: Map<String, Value>,
    },
    Layer(Vec<View>),
}

impl View {
    /// The views of `spec`: one unit, or a top-level layer of units. `None`
    /// for a spec this does not read.
    pub(crate) fn of(spec: &Map<String, Value>) -> Option<Self> {
        if UNSUPPORTED.iter().any(|key| spec.contains_key(*key)) {
            return None;
        }
        let shared = match spec.get("encoding") {
            Some(encoding) => encoding.as_object()?.clone(),
            None => Map::new(),
        };
        let Some(layers) = spec.get("layer") else {
            return Self::unit(spec, shared);
        };
        let children = layers
            .as_array()?
            .iter()
            .map(|layer| {
                let layer = layer.as_object()?;
                if layer.contains_key("layer")
                    || UNSUPPORTED.iter().any(|key| layer.contains_key(*key))
                {
                    return None;
                }
                let encoding = match layer.get("encoding") {
                    Some(own) => merge(&shared, own.as_object()?),
                    None => shared.clone(),
                };
                Self::unit(layer, encoding)
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            data: spec.get("data").cloned(),
            transform: steps(spec)?,
            body: Body::Layer(children),
        })
    }

    /// A unit spec's `mark` drawn as `encoding` says, a composite mark as the
    /// layer of its parts.
    fn unit(spec: &Map<String, Value>, encoding: Map<String, Value>) -> Option<Self> {
        let mark = spec.get("mark")?;
        let data = spec.get("data").cloned();
        let transform = steps(spec)?;
        let name = match mark {
            Value::String(name) => name.as_str(),
            Value::Object(def) => def.get("type")?.as_str()?,
            _ => return None,
        };
        match name.parse::<Composite>() {
            Ok(composite) => composite.normalize(mark, &encoding, data, transform),
            Err(()) => Some(Self {
                data,
                transform,
                body: Body::Unit {
                    mark: mark.clone(),
                    encoding,
                },
            }),
        }
    }
}

/// Vega-Lite's `mergeEncoding` of a layer's `encoding` under its parent's:
/// a field or datum definition over the parent's definition of its channel,
/// any other in place of it.
fn merge(parent: &Map<String, Value>, own: &Map<String, Value>) -> Map<String, Value> {
    let mut merged = parent.clone();
    for (channel, def) in own {
        let def = match (
            encoding::field_or_datum(Some(def)),
            parent.get(channel),
            def,
        ) {
            (true, Some(Value::Object(under)), Value::Object(over)) => {
                let mut def = under.clone();
                def.extend(over.clone());
                Value::Object(def)
            }
            (.., def) => def.clone(),
        };
        merged.insert(channel.clone(), def);
    }
    merged
}

/// A view's `transform` list. `None` as [`transform::pipeline`] answers.
fn steps(spec: &Map<String, Value>) -> Option<Vec<Step>> {
    Some(match spec.get("transform") {
        Some(list) => transform::pipeline(list)?
            .into_iter()
            .map(Step::Transform)
            .collect(),
        None => Vec::new(),
    })
}
