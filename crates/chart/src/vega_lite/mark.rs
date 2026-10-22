//! A view's `mark`: its type, and its definition's properties. A property
//! that is also a channel is that channel's value where the `encoding`
//! leaves the channel out.

use gpui::{FontStyle, FontWeight, SharedString};
use serde_json::{Map, Value, json};

use crate::{
    data::Data,
    expr::{Expr, Row, Value as ExprValue},
    model::{
        Align, Baseline, Cap, Dir, Extent, Interpolate, Join, LineBreak, Mark, Orient, Overlay,
        Properties,
    },
    vega_lite::encoding,
};

/// Mark properties that are channels' values, and the channel each sets.
const CHANNELS: [(&str, &str); 25] = [
    ("color", "color"),
    ("fill", "fill"),
    ("stroke", "stroke"),
    ("opacity", "opacity"),
    ("fillOpacity", "fillOpacity"),
    ("strokeOpacity", "strokeOpacity"),
    ("strokeWidth", "strokeWidth"),
    ("strokeDash", "strokeDash"),
    ("size", "size"),
    ("shape", "shape"),
    ("angle", "angle"),
    ("text", "text"),
    ("href", "href"),
    ("url", "url"),
    ("x", "x"),
    ("y", "y"),
    ("x2", "x2"),
    ("y2", "y2"),
    ("xOffset", "xOffset"),
    ("yOffset", "yOffset"),
    ("theta", "theta"),
    ("theta2", "theta2"),
    ("radius", "radius"),
    ("outerRadius", "radius"),
    ("innerRadius", "radius2"),
];

/// Properties that change nothing bezel draws: interaction, accessibility,
/// and what applies to bins, time units and config styles alone.
const DROPPED: [&str; 11] = [
    "tooltip",
    "invalid",
    "aria",
    "ariaRole",
    "ariaRoleDescription",
    "description",
    "cursor",
    "style",
    "binSpacing",
    "timeUnitBandSize",
    "timeUnitBandPosition",
];

/// A mark definition read: its type, its properties, and `encoding` with the
/// channel values the properties give. `None` for a mark or property this
/// does not draw.
pub(crate) fn read(
    mark: &Value,
    encoding: &Map<String, Value>,
) -> Option<(Mark, Properties, Map<String, Value>)> {
    let empty = Map::new();
    let (name, def) = match mark {
        Value::String(name) => (name.as_str(), &empty),
        Value::Object(def) => (def.get("type")?.as_str()?, def),
        _ => return None,
    };
    let mark = match name {
        "bar" => Mark::Bar,
        "line" => Mark::Line,
        "area" => Mark::Area,
        "point" => Mark::Point,
        "arc" => Mark::Arc,
        "rule" => Mark::Rule,
        "rect" => Mark::Rect,
        "text" => Mark::Text,
        "tick" => Mark::Tick,
        "trail" => Mark::Trail,
        "image" => Mark::Image,
        "circle" => Mark::Circle,
        "square" => Mark::Square,
        _ => return None,
    };
    let (properties, encoding) = properties(mark, def, encoding, true)?;
    Some((mark, properties, encoding))
}

/// `def`'s properties for `mark`, and `encoding` with the channel values
/// they give. `overlays` reads `point` and `line`.
fn properties(
    mark: Mark,
    def: &Map<String, Value>,
    encoding: &Map<String, Value>,
    overlays: bool,
) -> Option<(Properties, Map<String, Value>)> {
    let mut p = Properties::default();
    let mut encoding = encoding.clone();
    let filled = match def.get("filled") {
        Some(filled) => filled.as_bool()?,
        None => mark.filled(),
    };
    for (key, value) in def {
        let key = key.as_str();
        if key == "type" || DROPPED.contains(&key) {
            continue;
        }
        if let Some((_, channel)) = CHANNELS.iter().find(|(property, _)| *property == key) {
            // An encoded colour beats the property it would paint over.
            let beaten = match key {
                "fill" => filled && encoding.contains_key("color"),
                "stroke" => !filled && encoding.contains_key("color"),
                _ => false,
            };
            if !beaten && !encoding.contains_key(*channel) {
                let mut def = Map::new();
                def.insert("value".to_string(), value.clone());
                encoding.insert(channel.to_string(), Value::Object(def));
            }
            continue;
        }
        let number = || value.as_f64().map(|n| n as f32);
        match key {
            "filled" => p.filled = Some(value.as_bool()?),
            "clip" => p.clip = value.as_bool()?,
            "interpolate" => p.interpolate = value.as_str()?.parse().ok()?,
            "tension" => p.tension = Some(number()?),
            "orient" => {
                p.orient = Some(match value.as_str()? {
                    "horizontal" => Orient::Horizontal,
                    "vertical" => Orient::Vertical,
                    _ => return None,
                })
            }
            "cornerRadius" => {
                let radius = number()?;
                for corner in &mut p.corner_radius {
                    corner.get_or_insert(radius);
                }
            }
            // Vega rounds an arc's corners alike.
            "cornerRadiusTopLeft"
            | "cornerRadiusTopRight"
            | "cornerRadiusBottomRight"
            | "cornerRadiusBottomLeft"
            | "cornerRadiusEnd"
                if mark == Mark::Arc =>
            {
                number()?;
            }
            "cornerRadiusTopLeft" => p.corner_radius[0] = Some(number()?),
            "cornerRadiusTopRight" => p.corner_radius[1] = Some(number()?),
            "cornerRadiusBottomRight" => p.corner_radius[2] = Some(number()?),
            "cornerRadiusBottomLeft" => p.corner_radius[3] = Some(number()?),
            "cornerRadiusEnd" => p.corner_radius_end = Some(number()?),
            "strokeCap" => {
                p.stroke_cap = match value.as_str()? {
                    "butt" => Cap::Butt,
                    "round" => Cap::Round,
                    "square" => Cap::Square,
                    _ => return None,
                }
            }
            "strokeJoin" => {
                p.stroke_join = match value.as_str()? {
                    "miter" => Join::Miter,
                    "round" => Join::Round,
                    "bevel" => Join::Bevel,
                    _ => return None,
                }
            }
            "strokeMiterLimit" => p.stroke_miter_limit = Some(number()?),
            "strokeDashOffset" => p.stroke_dash_offset = number()?,
            "thickness" => p.thickness = Some(number()?),
            "width" => p.width = Some(extent(value)?),
            "height" => p.height = Some(extent(value)?),
            "aspect" => p.aspect = Some(value.as_bool()?),
            "align" => {
                p.align = Some(match value.as_str()? {
                    "left" => Align::Left,
                    "center" => Align::Center,
                    "right" => Align::Right,
                    _ => return None,
                })
            }
            "baseline" => {
                p.baseline = Some(match value.as_str()? {
                    "top" => Baseline::Top,
                    "middle" => Baseline::Middle,
                    "bottom" => Baseline::Bottom,
                    "alphabetic" => Baseline::Alphabetic,
                    "line-top" => Baseline::LineTop,
                    "line-bottom" => Baseline::LineBottom,
                    _ => return None,
                })
            }
            "dx" => p.dx = number()?,
            "dy" => p.dy = number()?,
            "font" => p.font = Some(SharedString::from(value.as_str()?.to_string())),
            "fontSize" => p.font_size = Some(number()?),
            "fontStyle" => {
                p.font_style = match value.as_str()? {
                    "normal" => FontStyle::Normal,
                    "italic" => FontStyle::Italic,
                    "oblique" => FontStyle::Oblique,
                    _ => return None,
                }
            }
            "fontWeight" => {
                p.font_weight = Some(match value {
                    Value::String(weight) if weight == "normal" => FontWeight::NORMAL,
                    Value::String(weight) if weight == "bold" => FontWeight::BOLD,
                    Value::Number(weight) => FontWeight(weight.as_f64()? as f32),
                    _ => return None,
                })
            }
            "limit" => p.limit = number()?,
            "ellipsis" => {
                let ellipsis = value.as_str()?;
                p.ellipsis = (!ellipsis.is_empty()).then(|| ellipsis.to_string().into());
            }
            "dir" => {
                p.dir = match value.as_str()? {
                    "ltr" => Dir::Ltr,
                    "rtl" => Dir::Rtl,
                    _ => return None,
                }
            }
            "lineBreak" => p.line_break = line_break(value)?,
            "lineHeight" => p.line_height = Some(number()?),
            "padAngle" if mark == Mark::Arc => p.pad_angle = number()?,
            "x2Offset" => p.x2_offset = number()?,
            "y2Offset" => p.y2_offset = number()?,
            "thetaOffset" => p.theta_offset = number()?,
            "theta2Offset" => p.theta2_offset = number()?,
            "radiusOffset" => p.radius_offset = number()?,
            "radius2Offset" => p.radius2_offset = number()?,
            "point" if overlays && is_path(mark) => {
                p.point = overlay(Mark::Point, value, &encoding)?;
            }
            "line" if overlays && mark == Mark::Area => {
                p.line = overlay(Mark::Line, value, &encoding)?;
            }
            _ => return None,
        }
    }
    // A shape channel asks a path for points, unless `point` says not.
    if overlays && is_path(mark) && !def.contains_key("point") && encoding.contains_key("shape") {
        p.point = overlay(Mark::Point, &Value::Bool(true), &encoding)?;
    }
    // A line overlay draws as its area does unless it says otherwise.
    if let Some(line) = &mut p.line
        && line.properties.interpolate == Interpolate::Linear
        && line.properties.tension.is_none()
    {
        line.properties.interpolate = p.interpolate;
        line.properties.tension = p.tension;
    }
    Some((p, encoding))
}

/// Line, area and trail, which take point overlays.
fn is_path(mark: Mark) -> bool {
    matches!(mark, Mark::Line | Mark::Area | Mark::Trail)
}

/// A `point` or `line` overlay: `true`, `false`, `"transparent"`, which
/// draws nothing, or a definition of its own.
fn overlay(
    mark: Mark,
    value: &Value,
    encoding: &Map<String, Value>,
) -> Option<Option<Box<Overlay>>> {
    let def = match value {
        Value::Bool(false) => return Some(None),
        Value::String(transparent) if transparent == "transparent" && mark == Mark::Point => {
            return Some(None);
        }
        Value::Bool(true) => &Map::new(),
        Value::Object(def) => def,
        _ => return None,
    };
    // Its channels' values: what its own properties give, over nothing
    // but the layer's colour.
    let mut own = Map::new();
    if let Some(color) = encoding.get("color") {
        own.insert("color".to_string(), color.clone());
    }
    let (mut properties, mut values) = properties(mark, def, &own, false)?;
    values.remove("color");
    // A point overlay is filled and opaque unless it says otherwise.
    if mark == Mark::Point {
        properties.filled.get_or_insert(true);
        values
            .entry("opacity")
            .or_insert_with(|| json!({"value": 1}));
    }
    let values = values
        .into_iter()
        .filter(|(_, def)| def.get("value").is_some())
        .collect();
    Some(Some(Box::new(Overlay {
        properties,
        encoding: encoding::read(&values, None)?.0,
    })))
}

/// A string, `""` for none, or an `expr` with neither `datum` nor params
/// that gives a string or a regular expression.
fn line_break(value: &Value) -> Option<Option<LineBreak>> {
    let text = |text: &str| (!text.is_empty()).then(|| LineBreak::Text(text.to_string().into()));
    match value {
        Value::String(separator) => Some(text(separator)),
        Value::Object(def) if def.len() == 1 => {
            let row = Row {
                data: &Data::new(),
                index: 0,
                params: &[],
            };
            match def
                .get("expr")?
                .as_str()?
                .parse::<Expr>()
                .ok()?
                .eval(&row)
                .ok()?
            {
                ExprValue::String(separator) => Some(text(&separator)),
                ExprValue::RegExp(regexp) => Some(Some(LineBreak::Pattern {
                    source: regexp.source.clone().into(),
                    flags: regexp.flags.clone().into(),
                })),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Pixels, or `{"band": share}`.
fn extent(value: &Value) -> Option<Extent> {
    match value {
        Value::Number(pixels) => Some(Extent::Pixels(pixels.as_f64()? as f32)),
        Value::Object(band) if band.len() == 1 => {
            Some(Extent::Band(band.get("band")?.as_f64()? as f32))
        }
        _ => None,
    }
}
