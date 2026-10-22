//! `compile/mark`: the unit's mark group, and the encode entry of a bar or
//! rect: its colour, aria description and rect position.

use crate::{
    Error,
    compile::{Channel, Orient, Unit, field, layout_size::Sizes, scale::Component, truthy},
    spec::{FieldDef, Type},
};
use d3::format::number::Number;
use serde_json::{Map, Value as Json, json};
use vega::spec as vg;

/// A Vega value reference, as JSON.
type Ref = Map<String, Json>;

fn value_ref(value: Json) -> Ref {
    json!({"value": value})
        .as_object()
        .cloned()
        .unwrap_or_default()
}

fn signal_ref(signal: String) -> Ref {
    json!({"signal": signal})
        .as_object()
        .cloned()
        .unwrap_or_default()
}

/// `parseMarkGroups` for a bar or rect.
pub fn parse(
    unit: &Unit,
    scales: &[Component],
    sizes: &Sizes,
    main: &str,
) -> Result<Vec<vg::Mark>, Error> {
    let mut update = Map::new();
    for (channel, value) in encode_entry(unit, scales, sizes)? {
        update.insert(channel, Json::Object(value));
    }
    let mark = json!({
        "name": "marks",
        "type": "rect",
        "style": unit.styles(),
        "from": {"data": main},
        "encode": {"update": update},
    });
    Ok(vec![
        serde_json::from_value(mark).map_err(|e| Error::Spec(e.to_string()))?,
    ])
}

/// The encode entry: channels in the order Vega-Lite writes them.
fn encode_entry(
    unit: &Unit,
    scales: &[Component],
    sizes: &Sizes,
) -> Result<Vec<(String, Ref)>, Error> {
    let mut entry: Vec<(String, Ref)> = Vec::new();
    let put = |entry: &mut Vec<(String, Ref)>, channel: &str, value: Ref| match entry
        .iter_mut()
        .find(|(c, _)| c == channel)
    {
        Some((_, v)) => *v = value,
        None => entry.push((channel.to_string(), value)),
    };
    // `color`: a filled mark's fill, the config's mark colour.
    let filled = unit
        .mark_config("filled", None)
        .and_then(Json::as_bool)
        .unwrap_or(!matches!(unit.mark.as_str(), "point" | "line" | "rule"));
    let color = |vg_channel: &str| {
        unit.mark_config("color", Some(vg_channel))
            .or_else(|| unit.config.get("mark", "color"))
            .cloned()
    };
    if filled {
        if let Some(fill) = color("fill") {
            put(&mut entry, "fill", value_ref(fill));
        }
    } else if let Some(stroke) = color("stroke") {
        put(&mut entry, "stroke", value_ref(stroke));
    }
    // `aria`: a mark Vega has no type of is described by its name.
    if unit.mark_config("aria", None) != Some(&json!(false)) {
        if let Some(role) = unit.mark_config("ariaRoleDescription", None) {
            put(&mut entry, "ariaRoleDescription", value_ref(role.clone()));
        } else if unit.mark != "rect" {
            put(
                &mut entry,
                "ariaRoleDescription",
                value_ref(json!(unit.mark)),
            );
        }
        if let Some(description) = description(unit) {
            put(&mut entry, "description", signal_ref(description));
        }
    }
    for channel in [Channel::X, Channel::Y] {
        for (name, value) in rect_position(unit, scales, sizes, channel)? {
            put(&mut entry, &name, value);
        }
    }
    Ok(entry)
}

/// `description` from `tooltipDataTuples`: each field's title and value.
fn description(unit: &Unit) -> Option<String> {
    let mut tuples: Vec<(String, String)> = Vec::new();
    for channel in [Channel::X, Channel::Y] {
        let Some(def) = unit.field_def(channel) else {
            continue;
        };
        let title = field::title(def);
        let key = match &title {
            Json::String(s) => s.clone(),
            Json::Array(items) => items
                .iter()
                .map(|i| i.as_str().map_or_else(|| i.to_string(), str::to_string))
                .collect::<Vec<_>>()
                .join(", "),
            other => other.to_string(),
        }
        .replace('"', "\\\"");
        let value = tooltip_value(def);
        if !tuples.iter().any(|(k, _)| *k == key) {
            tuples.push((key, value));
        }
    }
    let tuples: Vec<_> = tuples
        .into_iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .collect();
    if tuples.is_empty() {
        return None;
    }
    Some(
        tuples
            .iter()
            .enumerate()
            .map(|(i, (key, value))| {
                format!(
                    "\"{}{key}: \" + ({})",
                    if i > 0 { "; " } else { "" },
                    value.replace("\\n", " ")
                )
            })
            .collect::<Vec<_>>()
            .join(" + "),
    )
}

/// `addLineBreaksToTooltip`: a discrete field's text, its lines joined;
/// else the field formatted.
fn tooltip_value(def: &FieldDef) -> String {
    if def.type_.is_discrete() {
        let f = format!("datum[\"{}\"]", def.field);
        return format!("isValid({f}) ? isArray({f}) ? join({f}, '\\n') : {f} : \"\"+{f}");
    }
    let f = field::vg_field(
        def,
        &field::Options {
            expr: Some("datum"),
            ..Default::default()
        },
    );
    match def.type_ {
        Type::Quantitative => format!("format({f}, \"\")"),
        _ => format!("isValid({f}) ? {f} : \"\"+{f}"),
    }
}

/// `rectPosition`.
fn rect_position(
    unit: &Unit,
    scales: &[Component],
    sizes: &Sizes,
    channel: Channel,
) -> Result<Vec<(String, Ref)>, Error> {
    let Some(def) = unit.field_def(channel) else {
        return Err(Error::Unsupported(format!(
            "a bar without {}",
            channel.name()
        )));
    };
    let scale = scales.iter().find(|s| s.channel == channel);
    let band_or_tick = unit.mark == "bar"
        && match channel {
            Channel::X => unit.orient == Some(Orient::Vertical),
            Channel::Y => unit.orient == Some(Orient::Horizontal),
        };
    if scale.is_some_and(Component::is_discrete) || band_or_tick {
        position_and_size(unit, def, scale, channel)
    } else {
        range_position(unit, def, scale, sizes, channel)
    }
}

/// A value reference to `field` through `scale`.
fn field_ref(scale: Option<&Component>, field: String, band: Option<f64>) -> Ref {
    let mut r = Ref::new();
    if let Some(scale) = scale {
        r.insert("scale".into(), json!(scale.name));
    }
    r.insert("field".into(), json!(field));
    if let Some(band) = band.filter(|b| *b != 0.0) {
        r.insert("band".into(), json!(band));
    }
    r
}

/// `positionAndSize`: a band's position and its size.
fn position_and_size(
    unit: &Unit,
    def: &FieldDef,
    scale: Option<&Component>,
    channel: Channel,
) -> Result<Vec<(String, Ref)>, Error> {
    let size = channel.size();
    let band = scale.is_some_and(|s| s.type_ == "band");
    let discrete = scale.is_some_and(Component::is_discrete);
    // `getBandSize`: a rect-based mark fills its band, or a continuous
    // scale's band size.
    let band_size = if discrete {
        None
    } else {
        unit.config
            .get(&unit.mark, "continuousBandSize")
            .and_then(Json::as_f64)
    };
    let size_ref = match band_size {
        None if band => {
            let width = format!("bandwidth('{}')", scale.map_or("", |s| &s.name));
            match unit.mark_config("minBandSize", None) {
                Some(min) if truthy(min) => signal_ref(format!("max({}, {width})", number(min))),
                _ => signal_ref(width),
            }
        }
        None => {
            return Err(Error::Unsupported(
                "a relative band size off a band scale".into(),
            ));
        }
        Some(pixels) => value_ref(json!(pixels)),
    };
    let top = band && band_size.is_none();
    let (vg_channel, center) = match (channel, top) {
        (Channel::X, true) => ("x", false),
        (Channel::X, false) => ("xc", true),
        (Channel::Y, true) => ("y", false),
        (Channel::Y, false) => ("yc", true),
    };
    let band_position = if center { 0.5 } else { 0.0 };
    let field = field::vg_field(def, &field::Options::default());
    let position = field_ref(scale, field, band.then_some(band_position));
    Ok(vec![(vg_channel.into(), position), (size.into(), size_ref)])
}

/// `rangePosition`: a measure from its baseline, or its stack.
fn range_position(
    unit: &Unit,
    def: &FieldDef,
    scale: Option<&Component>,
    _sizes: &Sizes,
    channel: Channel,
) -> Result<Vec<(String, Ref)>, Error> {
    let stacked = unit
        .stack
        .as_ref()
        .is_some_and(|s| s.field_channel == channel);
    let named = |suffix: Option<&str>| {
        field::vg_field(
            def,
            &field::Options {
                suffix,
                ..Default::default()
            },
        )
    };
    let (position, position2) = if stacked {
        (
            field_ref(scale, named(Some("end")), None),
            field_ref(scale, named(Some("start")), None),
        )
    } else {
        let scale = scale.ok_or_else(|| Error::Unsupported("a range without a scale".into()))?;
        // `scaledZeroOrMinOrMax` for the baseline.
        let base = match scale.zero {
            Some(true) => json!({"scale": scale.name, "value": 0}),
            _ => json!({"signal": format!(
                "scale('{s}', inrange(0, domain('{s}')) ? 0 : domain('{s}')[0])",
                s = scale.name
            )}),
        };
        (
            field_ref(Some(scale), named(None), None),
            base.as_object().cloned().unwrap_or_default(),
        )
    };
    let (p, p2) = match channel {
        Channel::X => ("x", "x2"),
        Channel::Y => ("y", "y2"),
    };
    Ok(vec![(p.into(), position), (p2.into(), position2)])
}

/// `signalOrStringValue` of a number.
fn number(value: &Json) -> String {
    match value.as_f64() {
        Some(n) => Number(n).to_string(),
        None => value.to_string(),
    }
}
