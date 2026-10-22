//! vega-parser's axis parser: an axis as the group mark it draws, of grid
//! lines, ticks, labels, a domain line and a title over its ticks.

use crate::{
    config::Config,
    guide::truthy,
    spec::{Axis, string_value},
};
use serde_json::{Map, Value as Json, json};

/// The data the axis's marks read: one datum for the group, domain and
/// title, its ticks for the rest.
pub struct Sources {
    pub datum: String,
    pub ticks: String,
}

/// `axis-config.js`: the config an axis of `orient` on a `band` scale
/// falls back to.
fn axis_config(config: &Config, orient: &Json, band: bool) -> Map<String, Json> {
    let mut result = config.group("axis").clone();
    if let Json::String(orient) = orient {
        let xy = match orient.as_str() {
            "top" | "bottom" => "axisX",
            _ => "axisY",
        };
        let mut chars = orient.chars();
        let side = format!(
            "axis{}{}",
            chars
                .next()
                .map(|c| c.to_ascii_uppercase())
                .unwrap_or_default(),
            chars.as_str()
        );
        result.extend(config.group(xy).clone());
        result.extend(config.group(&side).clone());
    }
    if band {
        result.extend(config.group("axisBand").clone());
    }
    result
}

/// `guide-util.js`'s `lookup`: the axis's property, else the config's, else
/// `default`.
struct Lookup<'a> {
    axis: &'a Axis,
    config: Map<String, Json>,
    style: &'a Config,
}

impl Lookup<'_> {
    fn get(&self, name: &str) -> Json {
        self.or(name, Json::Null)
    }

    fn or(&self, name: &str, default: Json) -> Json {
        match self.axis.get(name).filter(|v| !v.is_null()) {
            Some(value) => value.clone(),
            None => match self.config.get(name).filter(|v| !v.is_null()) {
                Some(value) => value.clone(),
                None => match self.fallback(name) {
                    Some(value) if !value.is_null() => value,
                    _ => default,
                },
            },
        }
    }

    /// `axis-config.js`'s fallback to the guide styles, for the title and
    /// label properties a style holds. The config itself was looked up
    /// already.
    fn fallback(&self, name: &str) -> Option<Json> {
        let (style, prop) = match name.strip_prefix("title") {
            Some(rest) => (
                "guide-title",
                style_prop(rest, &["Font", "FontSize", "FontWeight"])?,
            ),
            None => (
                "guide-label",
                style_prop(name.strip_prefix("label")?, &["Font", "FontSize"])?,
            ),
        };
        self.style.style(style).get(&prop).cloned()
    }
}

/// The style property `Color` and the font properties name.
fn style_prop(rest: &str, fonts: &[&str]) -> Option<String> {
    match rest {
        "Color" => Some("fill".into()),
        rest if fonts.contains(&rest) => {
            let mut chars = rest.chars();
            Some(chars.next()?.to_ascii_lowercase().to_string() + chars.as_str())
        }
        _ => None,
    }
}

fn is_signal(value: &Json) -> bool {
    matches!(value, Json::Object(o) if o.contains_key("signal"))
}

fn signal_of(value: &Json) -> String {
    value["signal"].as_str().unwrap_or_default().to_string()
}

/// `encoder(value)`: an object as it is, anything else as `{value}`.
fn encoder(value: Json) -> Json {
    match value {
        Json::Object(_) => value,
        value => json!({"value": value}),
    }
}

fn is_x(orient: &Json) -> bool {
    matches!(orient.as_str(), Some("top" | "bottom"))
}

/// `getSign`: `a` on the left or top, else `b`.
fn sign(orient: &Json, a: i32, b: i32) -> Json {
    if is_signal(orient) {
        let o = signal_of(orient);
        json!({"signal": format!("{o} === 'left' || {o} === 'top' ? ({a}) : ({b})")})
    } else {
        match orient.as_str() {
            Some("left" | "top") => json!(a),
            _ => json!(b),
        }
    }
}

/// `ifX`: `a` for a horizontal axis, `b` for a vertical one.
fn if_x(orient: &Json, a: Json, b: Json) -> Json {
    if is_signal(orient) {
        if_enc(
            &format!("{o} === 'top' || {o} === 'bottom'", o = signal_of(orient)),
            a,
            b,
        )
    } else if is_x(orient) {
        a
    } else {
        b
    }
}

/// `ifY`: `a` for a vertical axis, `b` for a horizontal one.
fn if_y(orient: &Json, a: Json, b: Json) -> Json {
    if is_signal(orient) {
        if_enc(
            &format!("{o} !== 'top' && {o} !== 'bottom'", o = signal_of(orient)),
            a,
            b,
        )
    } else if is_x(orient) {
        b
    } else {
        a
    }
}

fn if_enc(test: &str, a: Json, b: Json) -> Json {
    let a = if a.is_null() { a } else { encoder(a) };
    let b = if b.is_null() { b } else { encoder(b) };
    let simple = |e: &Json| e.is_null() || e.as_object().is_some_and(|o| o.len() == 1);
    if simple(&a) && simple(&b) {
        let code = |e: &Json| match e {
            Json::Null => "null".to_string(),
            e if is_signal(e) => signal_of(e),
            e => string_value(&e["value"]),
        };
        json!({"signal": format!("{test} ? ({}) : ({})", code(&a), code(&b))})
    } else {
        let mut first = a.as_object().cloned().unwrap_or_default();
        let mut rule = Map::new();
        rule.insert("test".into(), json!(test));
        rule.append(&mut first);
        let mut rules = vec![Json::Object(rule)];
        if !b.is_null() {
            rules.push(b);
        }
        Json::Array(rules)
    }
}

/// `ifTop`: `{value: a}` on top, else `{value: b}`.
fn if_top(orient: &Json, a: &str, b: &str) -> Json {
    if is_signal(orient) {
        json!({"signal": format!("{} === 'top' ? ({}) : ({})", signal_of(orient), string_value(&json!(a)), string_value(&json!(b)))})
    } else if orient.as_str() == Some("top") {
        json!({"value": a})
    } else {
        json!({"value": b})
    }
}

/// `ifRight`: `{value: a}` on the right, else `{value: b}`.
fn if_right(orient: &Json, a: &str, b: &str) -> Json {
    if is_signal(orient) {
        json!({"signal": format!("{} === 'right' ? ({}) : ({})", signal_of(orient), string_value(&json!(a)), string_value(&json!(b)))})
    } else if orient.as_str() == Some("right") {
        json!({"value": a})
    } else {
        json!({"value": b})
    }
}

/// `mult(sign, value)`.
fn mult(sign: &Json, value: f64) -> Json {
    if value == 0.0 {
        json!(0)
    } else if is_signal(sign) {
        json!({"signal": format!("({}) * {}", signal_of(sign), crate::spec::string_value(&json!(value)))})
    } else {
        json!({"value": sign.as_f64().unwrap_or(1.0) * value})
    }
}

/// `patch(value, base)`: a signal ending in `(null)` falls through to
/// `base`.
fn patch(value: Json, base: &Json) -> Json {
    if is_signal(&value) {
        let s = signal_of(&value);
        if let Some(head) = s.strip_suffix("(null)") {
            return json!({"signal": format!("{head}{}", signal_of(base))});
        }
    }
    value
}

/// `addEncode`: a value reference to `update`, a plain value to `set`.
fn add_encode(encode: &mut Map<String, Json>, name: &str, value: Json, set: &str) {
    if value.is_null() {
        return;
    }
    let is_encoder = match &value {
        Json::Object(_) => true,
        Json::Array(items) => items.first().is_some_and(Json::is_object),
        _ => false,
    };
    let (block, value) = match is_encoder {
        true => ("update", value),
        false => (set, json!({"value": value})),
    };
    encode
        .entry(block)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .expect("a block")
        .insert(name.into(), value);
}

fn add_encoders(encode: &mut Map<String, Json>, enter: &[(&str, Json)], update: &[(&str, Json)]) {
    for (name, value) in enter {
        add_encode(encode, name, value.clone(), "enter");
    }
    for (name, value) in update {
        add_encode(encode, name, value.clone(), "update");
    }
}

/// `extendEncode`: `extra`'s blocks over `encode`'s, but for `skip`'s keys.
fn extend_encode(encode: &mut Map<String, Json>, extra: Option<&Json>, skip: &[&str]) {
    let Some(Json::Object(extra)) = extra else {
        return;
    };
    for (name, block) in extra {
        if skip.contains(&name.as_str()) {
            continue;
        }
        let target = encode
            .entry(name.clone())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("a block");
        if let Json::Object(block) = block {
            for (key, value) in block {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

/// `guideMark`: a mark of the axis, the user's encoding of the part over
/// its own.
fn guide_mark(mut mark: Map<String, Json>, user: Option<&Json>) -> Json {
    match user {
        Some(Json::Object(extras)) => {
            if let Some(name) = extras.get("name") {
                mark.insert("name".into(), name.clone());
            }
            if let Some(style) = extras.get("style") {
                mark.insert("style".into(), style.clone());
            }
            mark.insert(
                "interactive".into(),
                json!(extras.get("interactive").is_some_and(truthy)),
            );
            let mut encode = mark
                .remove("encode")
                .and_then(|e| e.as_object().cloned())
                .unwrap_or_default();
            extend_encode(&mut encode, user, &["name", "style", "interactive"]);
            mark.insert("encode".into(), Json::Object(encode));
        }
        _ => {
            mark.insert("interactive".into(), json!(false));
        }
    }
    Json::Object(mark)
}

/// `tickBand`: where ticks sit in a band, and whether an extra tick pegs the
/// domain's start.
struct TickBand {
    band: Json,
    extra: Json,
    offset: Json,
}

fn tick_band(l: &Lookup) -> TickBand {
    let v = l.get("tickBand");
    let mut offset = l.get("tickOffset");
    if !truthy(&v) {
        TickBand {
            band: l.get("bandPosition"),
            extra: l.get("tickExtra"),
            offset,
        }
    } else if is_signal(&v) {
        let s = signal_of(&v);
        if !offset.is_object() {
            offset =
                json!({"signal": format!("({s}) === 'extent' ? 0 : {}", string_value(&offset))});
        }
        TickBand {
            band: json!({"signal": format!("({s}) === 'extent' ? 1 : 0.5")}),
            extra: json!({"signal": format!("({s}) === 'extent'")}),
            offset,
        }
    } else if v.as_str() == Some("extent") {
        TickBand {
            band: json!(1),
            extra: json!(true),
            offset: json!(0),
        }
    } else {
        TickBand {
            band: json!(0.5),
            extra: json!(false),
            offset,
        }
    }
}

/// `extendOffset`.
fn extend_offset(value: Json, offset: Json) -> Json {
    if !truthy(&offset) {
        value
    } else if !truthy(&value) {
        offset
    } else if !value.is_object() {
        json!({"value": value, "offset": offset})
    } else {
        let mut value = value;
        let inner = value["offset"].take();
        value["offset"] = extend_offset(inner, offset);
        value
    }
}

/// What a JSON property holds as a value reference's literal, `null` read
/// as absent.
fn some(value: Json) -> Option<Json> {
    (!value.is_null()).then_some(value)
}

fn insert_some(map: &mut Map<String, Json>, key: &str, value: Json) {
    if let Some(value) = some(value) {
        map.insert(key.into(), value);
    }
}

/// The axis group's datum: its scale, and which parts it draws.
fn parts(axis: &Axis, l: &Lookup) -> Json {
    json!({
        "scale": axis.scale,
        "ticks": truthy(&l.get("ticks")),
        "labels": truthy(&l.get("labels")),
        "grid": truthy(&l.get("grid")),
        "domain": truthy(&l.get("domain")),
        "title": axis.title.as_ref().is_some_and(|t| !t.is_null()),
    })
}

/// The datum of the group `axis` draws, `band` where its scale is a band
/// scale.
pub fn datum(axis: &Axis, config: &Config, band: bool) -> Json {
    let l = Lookup {
        axis,
        config: axis_config(config, &axis.orient, band),
        style: config,
    };
    parts(axis, &l)
}

/// The group mark spec `axis` draws, `band` where its scale is a band
/// scale, reading `sources` for its data.
pub fn mark(axis: &Axis, config: &Config, band: bool, sources: &Sources) -> Json {
    let orient = axis.orient.clone();
    let l = Lookup {
        axis,
        config: axis_config(config, &orient, band),
        style: config,
    };
    let encode = axis
        .encode
        .as_ref()
        .map(|e| serde_json::to_value(e).expect("JSON"));
    let part = |name: &str| encode.as_ref().and_then(|e| e.get(name)).cloned();
    let band = tick_band(&l);
    let datum = parts(axis, &l);
    let mut children = Vec::new();
    let mut size = Json::Null;
    if datum["grid"] == json!(true) {
        children.push(grid(axis, &l, part("grid").as_ref(), &sources.ticks, &band));
    }
    if datum["ticks"] == json!(true) {
        size = l.get("tickSize");
        children.push(ticks(
            axis,
            &l,
            part("ticks").as_ref(),
            &sources.ticks,
            &size,
            &band,
        ));
    }
    if datum["labels"] == json!(true) {
        let size = if datum["ticks"] == json!(true) {
            size.clone()
        } else {
            json!(0)
        };
        children.push(labels(
            axis,
            &l,
            part("labels").as_ref(),
            &sources.ticks,
            &size,
            &band,
        ));
    }
    if datum["domain"] == json!(true) {
        children.push(domain(axis, &l, part("domain").as_ref(), &sources.datum));
    }
    if datum["title"] == json!(true) {
        children.push(title(axis, &l, part("title").as_ref(), &sources.datum));
    }
    let mut group_encode = Map::new();
    let offset = match l.get("offset") {
        v if truthy(&v) => v,
        _ => json!(0),
    };
    add_encoders(
        &mut group_encode,
        &[
            ("orient", l.get("orient")),
            ("offset", offset),
            ("position", axis.position.clone().unwrap_or(json!(0))),
            ("titlePadding", l.get("titlePadding")),
            ("minExtent", l.get("minExtent")),
            ("maxExtent", l.get("maxExtent")),
            (
                "range",
                json!({"signal": format!("abs(span(range({})))", string_value(&json!(axis.scale)))}),
            ),
            ("translate", l.get("translate")),
            ("format", axis.format.clone().unwrap_or(Json::Null)),
            ("formatType", axis.format_type.clone().unwrap_or(Json::Null)),
        ],
        &[],
    );
    group_encode.entry("enter").or_insert_with(|| json!({}));
    group_encode.entry("update").or_insert_with(|| json!({}));
    let axis_encode = part("axis");
    extend_encode(
        &mut group_encode,
        axis_encode.as_ref(),
        &["name", "style", "interactive"],
    );
    let mut mark = Map::new();
    mark.insert("type".into(), json!("group"));
    mark.insert("role".into(), json!("axis"));
    mark.insert("from".into(), json!({"data": sources.datum}));
    mark.insert("encode".into(), Json::Object(group_encode));
    mark.insert("marks".into(), Json::Array(children));
    insert_some(&mut mark, "aria", l.get("aria"));
    insert_some(&mut mark, "description", l.get("description"));
    insert_some(&mut mark, "zindex", l.get("zindex"));
    let extras = axis_encode.as_ref().and_then(Json::as_object);
    if let Some(extras) = extras {
        for key in ["name", "style"] {
            if let Some(value) = extras.get(key) {
                mark.insert(key.into(), value.clone());
            }
        }
    }
    mark.insert(
        "interactive".into(),
        json!(
            extras
                .and_then(|e| e.get("interactive"))
                .is_some_and(truthy)
        ),
    );
    Json::Object(mark)
}

fn tick_pos(axis: &Axis, l: &Lookup, band: &TickBand) -> Json {
    let mut pos = Map::new();
    pos.insert("scale".into(), json!(axis.scale));
    pos.insert("field".into(), json!("value"));
    insert_some(&mut pos, "band", band.band.clone());
    insert_some(&mut pos, "extra", band.extra.clone());
    insert_some(&mut pos, "offset", band.offset.clone());
    insert_some(&mut pos, "round", l.get("tickRound"));
    Json::Object(pos)
}

fn opacity_sets() -> Map<String, Json> {
    let mut encode = Map::new();
    encode.insert("enter".into(), json!({"opacity": {"value": 0}}));
    encode.insert("update".into(), json!({"opacity": {"value": 1}}));
    encode.insert("exit".into(), json!({"opacity": {"value": 0}}));
    encode
}

fn set(encode: &mut Map<String, Json>, block: &str, name: &str, value: Json) {
    if value.is_null() {
        return;
    }
    encode[block]
        .as_object_mut()
        .expect("a block")
        .insert(name.into(), value);
}

/// `offsetValue` in `axis-grid.js`.
fn offset_value(offset: Option<&Json>, sign: &Json) -> Json {
    let offset = offset.cloned().unwrap_or(Json::Null);
    if *sign == json!(1) {
        return offset;
    }
    if !offset.is_object() {
        let o = offset.as_f64().unwrap_or(0.0);
        return if is_signal(sign) {
            json!({"signal": format!("({}) * ({})", signal_of(sign), crate::spec::string_value(&json!(o)))})
        } else {
            json!(sign.as_f64().unwrap_or(1.0) * o)
        };
    }
    let mut offset = offset;
    let mut entry = &mut offset;
    loop {
        if entry.get("mult").is_some_and(|m| !m.is_null()) {
            if !entry["mult"].is_object() {
                let m = entry["mult"].clone();
                entry["mult"] = if is_signal(sign) {
                    json!({"signal": format!("({}) * ({})", string_value(&m), signal_of(sign))})
                } else {
                    json!(m.as_f64().unwrap_or(f64::NAN) * sign.as_f64().unwrap_or(1.0))
                };
                return offset;
            }
            entry = &mut entry["mult"];
        } else {
            entry["mult"] = sign.clone();
            return offset;
        }
    }
}

fn grid(axis: &Axis, l: &Lookup, user: Option<&Json>, ticks: &str, band: &TickBand) -> Json {
    let orient = &axis.orient;
    let vscale = axis.grid_scale.clone();
    let sign = sign(orient, 1, -1);
    let offset = offset_value(axis.offset.as_ref(), &sign);
    let mut encode = opacity_sets();
    add_encoders(
        &mut encode,
        &[
            ("stroke", l.get("gridColor")),
            ("strokeCap", l.get("gridCap")),
            ("strokeDash", l.get("gridDash")),
            ("strokeDashOffset", l.get("gridDashOffset")),
            ("strokeOpacity", l.get("gridOpacity")),
            ("strokeWidth", l.get("gridWidth")),
        ],
        &[],
    );
    let pos = tick_pos(axis, l, band);
    let size = if_x(
        orient,
        json!({"signal": "height"}),
        json!({"signal": "width"}),
    );
    let with = |mut base: Map<String, Json>| {
        base.insert("mult".into(), sign.clone());
        insert_some(&mut base, "offset", offset.clone());
        Json::Object(base)
    };
    let (start, end) = match &vscale {
        Some(vscale) => (
            with(
                json!({"scale": vscale, "range": 0})
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            with(
                json!({"scale": vscale, "range": 1})
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
        ),
        None => {
            let mut start = json!({"value": 0}).as_object().cloned().unwrap_or_default();
            insert_some(&mut start, "offset", offset.clone());
            (
                Json::Object(start),
                with(size.as_object().cloned().unwrap_or_default()),
            )
        }
    };
    for block in ["enter", "update"] {
        set(
            &mut encode,
            block,
            "x",
            if_x(orient, pos.clone(), start.clone()),
        );
        set(
            &mut encode,
            block,
            "y",
            if_y(orient, pos.clone(), start.clone()),
        );
        set(
            &mut encode,
            block,
            "x2",
            if_y(orient, end.clone(), Json::Null),
        );
        set(
            &mut encode,
            block,
            "y2",
            if_x(orient, end.clone(), Json::Null),
        );
    }
    set(
        &mut encode,
        "exit",
        "x",
        if_x(orient, pos.clone(), Json::Null),
    );
    set(&mut encode, "exit", "y", if_y(orient, pos, Json::Null));
    guide_mark(
        json!({"type": "rule", "role": "axis-grid", "key": "value", "from": {"data": ticks}, "encode": encode})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        user,
    )
}

fn ticks(
    axis: &Axis,
    l: &Lookup,
    user: Option<&Json>,
    data: &str,
    size: &Json,
    band: &TickBand,
) -> Json {
    let orient = &axis.orient;
    let sign = sign(orient, -1, 1);
    let mut encode = opacity_sets();
    add_encoders(
        &mut encode,
        &[
            ("stroke", l.get("tickColor")),
            ("strokeCap", l.get("tickCap")),
            ("strokeDash", l.get("tickDash")),
            ("strokeDashOffset", l.get("tickDashOffset")),
            ("strokeOpacity", l.get("tickOpacity")),
            ("strokeWidth", l.get("tickWidth")),
        ],
        &[],
    );
    let mut tick_size = encoder(size.clone());
    tick_size["mult"] = sign;
    let pos = tick_pos(axis, l, band);
    let zero = json!({"value": 0});
    for block in ["update", "enter"] {
        set(
            &mut encode,
            block,
            "y",
            if_x(orient, zero.clone(), pos.clone()),
        );
        set(
            &mut encode,
            block,
            "y2",
            if_x(orient, tick_size.clone(), Json::Null),
        );
    }
    set(
        &mut encode,
        "exit",
        "x",
        if_x(orient, pos.clone(), Json::Null),
    );
    for block in ["update", "enter"] {
        set(
            &mut encode,
            block,
            "x",
            if_y(orient, zero.clone(), pos.clone()),
        );
        set(
            &mut encode,
            block,
            "x2",
            if_y(orient, tick_size.clone(), Json::Null),
        );
    }
    set(&mut encode, "exit", "y", if_y(orient, pos, Json::Null));
    guide_mark(
        json!({"type": "rule", "role": "axis-tick", "key": "value", "from": {"data": data}, "encode": encode})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        user,
    )
}

fn flush_expr(scale: &str, threshold: &Json, a: &str, b: &str, c: &str) -> Json {
    let s = string_value(&json!(scale));
    json!({"signal": format!(
        "flush(range({s}), scale({s}, datum.value), {},{a},{b},{c})",
        string_value(threshold)
    )})
}

/// `deref`: a signal's expression, or the value.
fn deref(value: Json) -> Json {
    match value {
        v if is_signal(&v) => v["signal"].clone(),
        v => v,
    }
}

fn labels(
    axis: &Axis,
    l: &Lookup,
    user: Option<&Json>,
    data: &str,
    size: &Json,
    band: &TickBand,
) -> Json {
    let orient = &axis.orient;
    let scale = axis.scale.as_str();
    let sign = sign(orient, -1, 1);
    let flush = deref(l.get("labelFlush"));
    let flush_offset = deref(l.get("labelFlushOffset"));
    let label_align = l.get("labelAlign");
    let label_baseline = l.get("labelBaseline");
    let mut flush_on = flush == json!(0) || truthy(&flush);
    let mut tick_size = encoder(size.clone());
    tick_size["mult"] = sign.clone();
    let padding = match l.get("labelPadding") {
        v if truthy(&v) => v,
        _ => json!(0),
    };
    let mut padding = encoder(padding);
    padding["mult"] = sign;
    tick_size["offset"] = padding;
    let mut tick_pos = Map::new();
    tick_pos.insert("scale".into(), json!(scale));
    tick_pos.insert("field".into(), json!("value"));
    tick_pos.insert("band".into(), json!(0.5));
    insert_some(
        &mut tick_pos,
        "offset",
        extend_offset(band.offset.clone(), l.get("labelOffset")),
    );
    let tick_pos = Json::Object(tick_pos);
    let fo = string_value(&flush_offset);
    let align = if_x(
        orient,
        if flush_on {
            flush_expr(scale, &flush, "\"left\"", "\"right\"", "\"center\"")
        } else {
            json!({"value": "center"})
        },
        if_right(orient, "left", "right"),
    );
    let baseline = if_x(
        orient,
        if_top(orient, "bottom", "top"),
        if flush_on {
            flush_expr(scale, &flush, "\"top\"", "\"bottom\"", "\"middle\"")
        } else {
            json!({"value": "middle"})
        },
    );
    let offset_expr = flush_expr(scale, &flush, &format!("-({fo})"), &fo, "0");
    flush_on = flush_on && truthy(&flush_offset);
    let x = if_x(orient, tick_pos.clone(), tick_size.clone());
    let y = if_y(orient, tick_pos, tick_size);
    let mut encode = Map::new();
    let mut enter = Map::new();
    enter.insert("opacity".into(), json!({"value": 0}));
    insert_some(&mut enter, "x", x.clone());
    insert_some(&mut enter, "y", y.clone());
    encode.insert("enter".into(), Json::Object(enter));
    let mut update = Map::new();
    update.insert("opacity".into(), json!({"value": 1}));
    update.insert("text".into(), json!({"field": "label"}));
    insert_some(&mut update, "x", x.clone());
    insert_some(&mut update, "y", y.clone());
    update.insert("align".into(), align.clone());
    update.insert("baseline".into(), baseline.clone());
    encode.insert("update".into(), Json::Object(update));
    let mut exit = Map::new();
    exit.insert("opacity".into(), json!({"value": 0}));
    insert_some(&mut exit, "x", x);
    insert_some(&mut exit, "y", y);
    encode.insert("exit".into(), Json::Object(exit));
    add_encoders(
        &mut encode,
        &[
            (
                "dx",
                if label_align.is_null() && flush_on {
                    if_x(orient, offset_expr.clone(), Json::Null)
                } else {
                    Json::Null
                },
            ),
            (
                "dy",
                if label_baseline.is_null() && flush_on {
                    if_y(orient, offset_expr, Json::Null)
                } else {
                    Json::Null
                },
            ),
        ],
        &[],
    );
    add_encoders(
        &mut encode,
        &[
            ("angle", l.get("labelAngle")),
            ("fill", l.get("labelColor")),
            ("fillOpacity", l.get("labelOpacity")),
            ("font", l.get("labelFont")),
            ("fontSize", l.get("labelFontSize")),
            ("fontWeight", l.get("labelFontWeight")),
            ("fontStyle", l.get("labelFontStyle")),
            ("limit", l.get("labelLimit")),
            ("lineHeight", l.get("labelLineHeight")),
        ],
        &[("align", label_align), ("baseline", label_baseline)],
    );
    let bound = l.get("labelBound");
    let overlap = l.get("labelOverlap");
    let overlap = if truthy(&overlap) || truthy(&bound) {
        let mut o = Map::new();
        insert_some(&mut o, "separation", l.get("labelSeparation"));
        o.insert("method".into(), overlap);
        o.insert("order".into(), json!("datum.index"));
        o.insert(
            "bound".into(),
            if truthy(&bound) {
                json!({"scale": scale, "orient": orient, "tolerance": bound})
            } else {
                Json::Null
            },
        );
        Some(Json::Object(o))
    } else {
        None
    };
    for (key, base) in [("align", &align), ("baseline", &baseline)] {
        let current = encode["update"][key].clone();
        if current != *base {
            encode["update"][key] = patch(current, base);
        }
    }
    let mut mark = json!({
        "type": "text",
        "role": "axis-label",
        "style": "guide-label",
        "key": "value",
        "from": {"data": data},
        "encode": encode,
    })
    .as_object()
    .cloned()
    .unwrap_or_default();
    if let Some(overlap) = overlap {
        mark.insert("overlap".into(), overlap);
    }
    guide_mark(mark, user)
}

fn domain(axis: &Axis, l: &Lookup, user: Option<&Json>, data: &str) -> Json {
    let orient = &axis.orient;
    let mut encode = opacity_sets();
    add_encoders(
        &mut encode,
        &[
            ("stroke", l.get("domainColor")),
            ("strokeCap", l.get("domainCap")),
            ("strokeDash", l.get("domainDash")),
            ("strokeDashOffset", l.get("domainDashOffset")),
            ("strokeWidth", l.get("domainWidth")),
            ("strokeOpacity", l.get("domainOpacity")),
        ],
        &[],
    );
    let pos = |range: u8| json!({"scale": axis.scale, "range": range});
    let zero = json!({"value": 0});
    for block in ["enter", "update"] {
        set(&mut encode, block, "x", if_x(orient, pos(0), zero.clone()));
        set(&mut encode, block, "x2", if_x(orient, pos(1), Json::Null));
    }
    for block in ["enter", "update"] {
        set(&mut encode, block, "y", if_y(orient, pos(0), zero.clone()));
        set(&mut encode, block, "y2", if_y(orient, pos(1), Json::Null));
    }
    guide_mark(
        json!({"type": "rule", "role": "axis-domain", "from": {"data": data}, "encode": encode})
            .as_object()
            .cloned()
            .unwrap_or_default(),
        user,
    )
}

/// `anchorExpr(s, e, m)`.
fn anchor_expr(s: &str, e: &str, m: &str) -> String {
    format!("item.anchor === 'start' ? {s} : item.anchor === 'end' ? {e} : {m}")
}

fn title(axis: &Axis, l: &Lookup, user: Option<&Json>, data: &str) -> Json {
    let orient = &axis.orient;
    let sign = sign(orient, -1, 1);
    let align_expr = anchor_expr("\"left\"", "\"right\"", "\"center\"");
    let mut enter = Map::new();
    enter.insert("opacity".into(), json!({"value": 0}));
    enter.insert("anchor".into(), encoder(l.or("titleAnchor", Json::Null)));
    enter.insert("align".into(), json!({"signal": align_expr}));
    let mut update = enter.clone();
    update.insert("opacity".into(), json!({"value": 1}));
    update.insert(
        "text".into(),
        encoder(axis.title.clone().unwrap_or(Json::Null)),
    );
    let title_pos = json!({"signal": format!(
        "lerp(range({}), {})",
        string_value(&json!(axis.scale)),
        anchor_expr("0", "1", "0.5")
    )});
    insert_some(
        &mut update,
        "x",
        if_x(orient, title_pos.clone(), Json::Null),
    );
    insert_some(&mut update, "y", if_y(orient, title_pos, Json::Null));
    let angle = if_x(orient, json!({"value": 0}), mult(&sign, 90.0));
    let baseline = if_x(
        orient,
        if_top(orient, "bottom", "top"),
        json!({"value": "bottom"}),
    );
    enter.insert("angle".into(), angle.clone());
    enter.insert("baseline".into(), baseline.clone());
    update.insert("angle".into(), angle);
    update.insert("baseline".into(), baseline);
    let mut encode = Map::new();
    encode.insert("enter".into(), Json::Object(enter));
    encode.insert("update".into(), Json::Object(update));
    encode.insert("exit".into(), json!({"opacity": {"value": 0}}));
    add_encoders(
        &mut encode,
        &[
            ("fill", l.get("titleColor")),
            ("fillOpacity", l.get("titleOpacity")),
            ("font", l.get("titleFont")),
            ("fontSize", l.get("titleFontSize")),
            ("fontStyle", l.get("titleFontStyle")),
            ("fontWeight", l.get("titleFontWeight")),
            ("limit", l.get("titleLimit")),
            ("lineHeight", l.get("titleLineHeight")),
        ],
        &[
            ("align", l.get("titleAlign")),
            ("angle", l.get("titleAngle")),
            ("baseline", l.get("titleBaseline")),
        ],
    );
    // `autoLayout`.
    let user_has = |dim: &str| {
        user.is_some_and(|u| {
            ["enter", "update"]
                .iter()
                .any(|set| u.get(*set).and_then(|b| b.get(dim)).is_some())
        })
    };
    let auto = |value: Json, dim: &str, encode: &mut Map<String, Json>| -> bool {
        if !value.is_null() {
            let current = encode["update"][dim].clone();
            encode["update"][dim] = patch(encoder(value), &current);
            false
        } else {
            !user_has(dim)
        }
    };
    let auto_y = auto(l.get("titleX"), "x", &mut encode);
    let auto_x = auto(l.get("titleY"), "y", &mut encode);
    encode["enter"]["auto"] = if auto_x == auto_y {
        encoder(json!(auto_x))
    } else {
        if_x(orient, encoder(json!(auto_x)), encoder(json!(auto_y)))
    };
    let enter = encode["enter"].clone();
    for key in ["align", "angle", "baseline"] {
        let current = encode["update"][key].clone();
        encode["update"][key] = patch(current, &enter[key]);
    }
    guide_mark(
        json!({
            "type": "text",
            "role": "axis-title",
            "style": "guide-title",
            "from": {"data": data},
            "encode": encode,
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
        user,
    )
}
