//! A view's `encoding`: each channel's field, datum or value.

use crate::{
    model::{Channel, Datum, Encode, Encoding, Kind, Position, Shape, Sort, Source},
    scale, time,
    vega_lite::datetime,
};
use gpui::{Hsla, Rgba, SharedString};
use js::{color::Rgb, number::Number as JsNumber};
use serde_json::{Map, Value};

/// What a field or datum definition may carry.
const CHANNEL: [&str; 9] = [
    "field", "datum", "type", "title", "axis", "legend", "scale", "sort", "stack",
];
/// Channels that share their primary's scale and take its type.
const SECONDARY: [(&str, &str); 4] = [
    ("x2", "x"),
    ("y2", "y"),
    ("theta2", "theta"),
    ("radius2", "radius"),
];
/// Numeric scale types, which make a field with no `type` quantitative.
const NUMERIC: [&str; 9] = [
    "linear",
    "log",
    "pow",
    "sqrt",
    "symlog",
    "quantile",
    "quantize",
    "threshold",
    "bin-ordinal",
];

/// `None` for a channel or a property of one this does not read.
pub(crate) fn read(map: &Map<String, Value>) -> Option<Encoding> {
    let mut encoding = Encoding::default();
    let e = &mut encoding;
    for (name, def) in map {
        if SECONDARY.iter().any(|(secondary, _)| secondary == name) {
            continue;
        }
        let name = name.as_str();
        match name {
            "x" => e.x = Some(encode(def, name, None, across)?),
            "y" => e.y = Some(encode(def, name, None, down)?),
            "xOffset" => e.x_offset = Some(encode(def, name, None, number)?),
            "yOffset" => e.y_offset = Some(encode(def, name, None, number)?),
            "theta" => e.theta = Some(encode(def, name, None, number)?),
            "radius" => e.radius = Some(encode(def, name, None, number)?),
            "color" => e.color = Some(encode(def, name, None, color)?),
            "fill" => e.fill = Some(encode(def, name, None, color)?),
            "stroke" => e.stroke = Some(encode(def, name, None, color)?),
            "opacity" => e.opacity = Some(encode(def, name, None, number)?),
            "fillOpacity" => e.fill_opacity = Some(encode(def, name, None, number)?),
            "strokeOpacity" => e.stroke_opacity = Some(encode(def, name, None, number)?),
            "size" => e.size = Some(encode(def, name, None, number)?),
            "strokeWidth" => e.stroke_width = Some(encode(def, name, None, number)?),
            "strokeDash" => e.stroke_dash = Some(encode(def, name, None, dash)?),
            "shape" => e.shape = Some(encode(def, name, None, shape)?),
            "angle" => e.angle = Some(encode(def, name, None, number)?),
            "text" => e.text = Some(encode(def, name, None, text)?),
            "href" => e.href = Some(encode(def, name, None, string)?),
            "url" => e.url = Some(encode(def, name, None, string)?),
            "detail" => e.detail = list(def, name)?,
            "order" => e.order = list(def, name)?,
            "key" => e.key = Some(channel(def.as_object()?, name, None)?),
            "tooltip" => {}
            _ => return None,
        }
    }
    for (secondary, primary) in SECONDARY {
        let Some(def) = map.get(secondary) else {
            continue;
        };
        let primary = match primary {
            "x" => e.x.as_ref().and_then(Encode::channel),
            "y" => e.y.as_ref().and_then(Encode::channel),
            "theta" => e.theta.as_ref().and_then(Encode::channel),
            _ => e.radius.as_ref().and_then(Encode::channel),
        };
        let kind = primary.map(|primary| primary.kind);
        if kind.is_none() && def.get("value").is_none() {
            return None;
        }
        match secondary {
            "x2" => e.x2 = Some(encode(def, secondary, kind, across)?),
            "y2" => e.y2 = Some(encode(def, secondary, kind, down)?),
            "theta2" => e.theta2 = Some(encode(def, secondary, kind, number)?),
            _ => e.radius2 = Some(encode(def, secondary, kind, number)?),
        }
    }
    Some(encoding)
}

/// A value definition read by `value`, or a field or datum definition. A
/// secondary channel takes `kind` from its primary and states no type.
fn encode<V>(
    def: &Value,
    name: &str,
    kind: Option<Kind>,
    value: fn(&Value) -> Option<V>,
) -> Option<Encode<V>> {
    let def = def.as_object()?;
    match def.get("value") {
        Some(v) if def.len() == 1 => Some(Encode::Value(value(v)?)),
        Some(_) => None,
        None => Some(Encode::Scaled(channel(def, name, kind)?)),
    }
}

/// One field definition, or a list of them.
fn list(def: &Value, name: &str) -> Option<Vec<Channel>> {
    match def {
        Value::Array(defs) => defs
            .iter()
            .map(|def| channel(def.as_object()?, name, None))
            .collect(),
        def => Some(vec![channel(def.as_object()?, name, None)?]),
    }
}

fn channel(def: &Map<String, Value>, name: &str, inherited: Option<Kind>) -> Option<Channel> {
    if def.keys().any(|key| !CHANNEL.contains(&key.as_str())) {
        return None;
    }
    let source = match (def.get("field"), def.get("datum")) {
        (Some(Value::String(field)), None) => Source::Field(field.clone().into()),
        (None, Some(Value::Number(n))) => Source::Datum(Datum::Number(n.as_f64()?)),
        (None, Some(Value::String(s))) => Source::Datum(Datum::Text(s.clone().into())),
        (None, Some(Value::Object(time))) => Source::Datum(Datum::Number(datetime::time(time)?)),
        _ => return None,
    };
    let scale_type = def
        .get("scale")
        .and_then(|scale| scale.get("type"))
        .and_then(Value::as_str);
    let kind = match (def.get("type"), inherited) {
        (Some(_), Some(_)) => return None,
        (None, Some(kind)) => kind,
        (Some(kind), None) => match kind.as_str()? {
            "quantitative" => Kind::Quantitative,
            "temporal" => Kind::Temporal,
            "ordinal" => Kind::Ordinal,
            "nominal" => Kind::Nominal,
            _ => return None,
        },
        (None, None) => match (&source, name) {
            _ if def.get("datum").is_some_and(Value::is_object) => Kind::Temporal,
            (Source::Datum(Datum::Number(_)), _) => Kind::Quantitative,
            (Source::Datum(Datum::Text(_)), _) => Kind::Nominal,
            (_, "shape" | "strokeDash") => Kind::Nominal,
            (_, "order") => Kind::Ordinal,
            _ if def.get("sort").is_some_and(Value::is_array) => Kind::Ordinal,
            _ if scale_type.is_some_and(|t| NUMERIC.contains(&t)) => Kind::Quantitative,
            _ if matches!(scale_type, Some("time" | "utc")) => Kind::Temporal,
            _ => Kind::Nominal,
        },
    };
    let source = match (kind, source) {
        (Kind::Temporal, Source::Datum(Datum::Text(text))) => {
            Source::Datum(Datum::Number(time::parse(&text)?))
        }
        (_, source) => source,
    };
    let mut out = Channel {
        source,
        kind,
        title: None,
        scale: Default::default(),
        sort: Sort::default(),
    };

    let guide = ["axis", "legend"]
        .into_iter()
        .filter_map(|key| def.get(key)?.as_object()?.get("title"))
        .next();
    out.title = match guide.or(def.get("title")) {
        None => out.field().cloned(),
        Some(written) => title(written),
    };

    if let Some(value) = def.get("scale") {
        for (key, value) in value.as_object()? {
            match (key.as_str(), value) {
                ("zero", Value::Bool(zero)) => out.scale.zero = *zero,
                ("nice", Value::Bool(nice)) => out.scale.nice = *nice,
                ("domain", Value::Array(ends))
                    if matches!(kind, Kind::Quantitative | Kind::Temporal) =>
                {
                    let end = |value: &Value| match value {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) if kind == Kind::Temporal => time::parse(s),
                        Value::Object(time) => datetime::time(time),
                        _ => None,
                    };
                    let [low, high] = ends.as_slice() else {
                        return None;
                    };
                    out.scale.domain = Some([end(low)?, end(high)?]);
                }
                ("type", Value::String(name))
                    if matches!(
                        (kind, name.as_str()),
                        (Kind::Quantitative, "linear")
                            | (Kind::Temporal, "time" | "utc")
                            | (Kind::Ordinal | Kind::Nominal, "band" | "point" | "ordinal")
                    ) => {}
                _ => return None,
            }
        }
    }

    let sorted = match name {
        "order" => Some(Sort::Ascending),
        _ if kind.is_discrete() => Some(Sort::Ascending),
        _ => None,
    };
    if let Some(default) = sorted {
        out.sort = match def.get("sort") {
            None => default,
            Some(Value::Null) if name != "order" => Sort::Data,
            Some(Value::String(order)) if order == "ascending" => Sort::Ascending,
            Some(Value::String(order)) if order == "descending" => Sort::Descending,
            Some(Value::Array(values)) if name != "order" => Sort::Explicit(
                values
                    .iter()
                    .map(|value| match value {
                        Value::String(s) => Some(s.clone().into()),
                        Value::Number(n) => Some(scale::value(n.as_f64()?).into()),
                        Value::Object(time) => Some(scale::value(datetime::time(time)?).into()),
                        _ => None,
                    })
                    .collect::<Option<_>>()?,
            ),
            Some(_) => return None,
        };
    }

    match def.get("stack") {
        None | Some(Value::Bool(true)) => {}
        Some(Value::String(stack)) if stack == "zero" => {}
        Some(_) => return None,
    }
    Some(out)
}

fn number(value: &Value) -> Option<f32> {
    Some(value.as_f64()? as f32)
}

/// An x or x2 value: pixels, or `"width"`.
fn across(value: &Value) -> Option<Position> {
    match value {
        Value::String(width) if width == "width" => Some(Position::Size),
        value => Some(Position::Pixels(number(value)?)),
    }
}

/// A y or y2 value: pixels, or `"height"`.
fn down(value: &Value) -> Option<Position> {
    match value {
        Value::String(height) if height == "height" => Some(Position::Size),
        value => Some(Position::Pixels(number(value)?)),
    }
}

/// A CSS colour as d3-color reads it. `null` is no colour.
fn color(value: &Value) -> Option<Hsla> {
    let Rgb { r, g, b, opacity } = match value {
        Value::Null => return Some(Hsla::transparent_black()),
        value => value.as_str()?.parse().ok()?,
    };
    if opacity.is_nan() || r.is_nan() {
        return Some(Hsla::transparent_black());
    }
    let channel = |v: f64| (v / 255.0).clamp(0.0, 1.0) as f32;
    Some(
        Rgba {
            r: channel(r),
            g: channel(g),
            b: channel(b),
            a: opacity.clamp(0.0, 1.0) as f32,
        }
        .into(),
    )
}

fn shape(value: &Value) -> Option<Shape> {
    value.as_str()?.parse().ok()
}

fn dash(value: &Value) -> Option<Vec<f32>> {
    value.as_array()?.iter().map(number).collect()
}

fn string(value: &Value) -> Option<SharedString> {
    Some(value.as_str()?.to_string().into())
}

/// A string, a number as JavaScript writes it, or lines.
fn text(value: &Value) -> Option<SharedString> {
    match value {
        Value::String(text) => Some(text.clone().into()),
        Value::Number(n) => Some(JsNumber(n.as_f64()?).to_string().into()),
        Value::Array(lines) => Some(
            lines
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()?
                .join("\n")
                .into(),
        ),
        _ => None,
    }
}

/// A title: a string, lines joined by spaces, or `null` for none.
pub(crate) fn title(value: &Value) -> Option<SharedString> {
    match value {
        Value::String(text) => Some(text.clone().into()),
        Value::Array(lines) => Some(
            lines
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
                .into(),
        ),
        _ => None,
    }
}
