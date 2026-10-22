//! A view's `encoding`: each channel's field, datum or value, and what
//! Vega-Lite derives each field from.

use crate::{
    color,
    model::{
        Channel, Datum, Domain, Encode, Encoding, Kind, Mark, Nice, Palette, Position, Range,
        Scale, ScaleType, Scheme, Shape, Sort, Source, Stack,
    },
    scale, time,
    transform::{Bins, Op},
    vega_lite::{
        datetime,
        field::{self, Derive},
    },
};
use gpui::{Hsla, SharedString};
use js::{interpolate::Interpolate, number::Number as JsNumber, scheme as palette, time::Unit};
use serde_json::{Map, Value};

/// What a field or datum definition may carry.
const CHANNEL: [&str; 13] = [
    "field",
    "datum",
    "type",
    "title",
    "axis",
    "legend",
    "scale",
    "sort",
    "stack",
    "aggregate",
    "bin",
    "timeUnit",
    "bandPosition",
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
/// Channels a sort may name, by the encoding's channel it sorts by.
const SORT_CHANNELS: [&str; 12] = [
    "x",
    "y",
    "color",
    "fill",
    "stroke",
    "strokeWidth",
    "size",
    "shape",
    "fillOpacity",
    "strokeOpacity",
    "opacity",
    "text",
];

/// A field definition read: its channel, the field as written, its type, and
/// what Vega-Lite derives the channel's field from.
#[derive(Clone, Debug)]
pub(crate) struct Def {
    pub(crate) channel: String,
    pub(crate) field: Option<SharedString>,
    pub(crate) kind: Kind,
    pub(crate) derive: Derive,
    /// The order of a discrete field's values by an aggregate of each
    /// value's rows.
    pub(crate) sort: Option<SortBy>,
    /// Whether the channel reads a bin's edges as text, `<start>_range`.
    pub(crate) range: bool,
    /// Where in each bin or step a mark that does not span it stands, 0 at
    /// its start and 1 at its end: the channel reads `<start>_band_<p>`.
    pub(crate) band: Option<f64>,
    /// Where in each step a bar or rect centres: it spans
    /// `<start>_offsetted_rect_start` to `<start>_offsetted_rect_end`.
    pub(crate) rect_band: Option<f64>,
    /// Whether an aggregate groups by where each bin or step ends too.
    pub(crate) end: bool,
    /// Whether its aggregate takes each group's `min` and `max` of the
    /// field too, for an unaggregated domain.
    pub(crate) unaggregated: bool,
}

/// A sort by `op` of `field` over each value's rows, `None` for the
/// default op.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SortBy {
    pub(crate) op: Option<Op>,
    pub(crate) field: Option<SharedString>,
    pub(crate) descending: bool,
    /// The channel whose field and aggregate it sorts by, in place of
    /// `op` and `field`.
    pub(crate) channel: Option<String>,
}

impl Def {
    /// The fields of where each bin or step starts and ends.
    pub(crate) fn edges(&self) -> Option<[SharedString; 2]> {
        match &self.derive {
            Derive::Bin(bin) => Some(bin.output.clone()),
            Derive::TimeUnit(unit) => Some(unit.output.clone()),
            Derive::BinnedTimeUnit { .. } => {
                let field = self.field.clone()?;
                Some([field.clone(), format!("{field}_end").into()])
            }
            _ => None,
        }
    }

    /// The fields of the data it reads, each with the type to read it as:
    /// `None` reads values as they are.
    pub(crate) fn reads(&self) -> Vec<(SharedString, Option<Kind>)> {
        let mut reads = Vec::new();
        match &self.derive {
            Derive::Aggregate {
                op: Op::Argmin | Op::Argmax,
                measured,
                ..
            } => {
                reads.extend(measured.clone().map(|f| (f, None)));
                reads.extend(self.field.clone().map(|f| (f, Some(self.kind))));
            }
            // Vega-Lite parses a temporal field, and a quantitative one for
            // `min` and `max`; Vega reads the others' text as numbers.
            Derive::Aggregate { op, measured, .. } => {
                let kind = match (self.kind, op) {
                    (Kind::Temporal, _) | (Kind::Quantitative, Op::Min | Op::Max) => {
                        Some(self.kind)
                    }
                    _ => None,
                };
                reads.extend(measured.clone().map(|f| (f, kind)));
            }
            Derive::Bin(_) => reads.extend(self.field.clone().map(|f| (f, None))),
            Derive::TimeUnit(_) | Derive::BinnedTimeUnit { .. } => {
                reads.extend(self.field.clone().map(|f| (f, Some(Kind::Temporal))));
            }
            Derive::None | Derive::Binned(_) => {
                reads.extend(self.field.clone().map(|f| (f, Some(self.kind))));
            }
        }
        if let Some(field) = self.sort.as_ref().and_then(|sort| sort.field.clone()) {
            reads.push((field, None));
        }
        reads
    }
}

/// Reads a view's channels, keeping each field definition read.
struct Reader {
    mark: Option<Mark>,
    /// Whether `x2` and `y2` are encoded.
    paired: [bool; 2],
    defs: Vec<Def>,
}

/// The channels of `map`, and each field definition among them, for a
/// `mark` (`None` for an overlay's values). `None` for a channel or a
/// property of one this does not read.
pub(crate) fn read(map: &Map<String, Value>, mark: Option<Mark>) -> Option<(Encoding, Vec<Def>)> {
    let mut encoding = Encoding::default();
    let mut r = Reader {
        mark,
        paired: [map.contains_key("x2"), map.contains_key("y2")],
        defs: Vec::new(),
    };
    let e = &mut encoding;
    for (name, def) in map {
        if SECONDARY.iter().any(|(secondary, _)| secondary == name) {
            continue;
        }
        let name = name.as_str();
        match name {
            "x" => e.x = Some(r.encode(def, name, None, across)?),
            "y" => e.y = Some(r.encode(def, name, None, down)?),
            "xOffset" => e.x_offset = Some(r.encode(def, name, None, number)?),
            "yOffset" => e.y_offset = Some(r.encode(def, name, None, number)?),
            "theta" => e.theta = Some(r.encode(def, name, None, number)?),
            "radius" => e.radius = Some(r.encode(def, name, None, number)?),
            "color" => e.color = Some(r.encode(def, name, None, color)?),
            "fill" => e.fill = Some(r.encode(def, name, None, color)?),
            "stroke" => e.stroke = Some(r.encode(def, name, None, color)?),
            "opacity" => e.opacity = Some(r.encode(def, name, None, number)?),
            "fillOpacity" => e.fill_opacity = Some(r.encode(def, name, None, number)?),
            "strokeOpacity" => e.stroke_opacity = Some(r.encode(def, name, None, number)?),
            "size" => e.size = Some(r.encode(def, name, None, number)?),
            "strokeWidth" => e.stroke_width = Some(r.encode(def, name, None, number)?),
            "strokeDash" => e.stroke_dash = Some(r.encode(def, name, None, dash)?),
            "shape" => e.shape = Some(r.encode(def, name, None, shape)?),
            "angle" => e.angle = Some(r.encode(def, name, None, number)?),
            "text" => e.text = Some(r.encode(def, name, None, text)?),
            "href" => e.href = Some(r.encode(def, name, None, string)?),
            "url" => e.url = Some(r.encode(def, name, None, string)?),
            "detail" => e.detail = r.list(def, name)?,
            "order" => e.order = r.list(def, name)?,
            "key" => e.key = Some(r.channel(def.as_object()?, name, None)?),
            "tooltip" => r.tooltip(def),
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
            "x2" => e.x2 = Some(r.encode(def, secondary, kind, across)?),
            "y2" => e.y2 = Some(r.encode(def, secondary, kind, down)?),
            "theta2" => e.theta2 = Some(r.encode(def, secondary, kind, number)?),
            _ => e.radius2 = Some(r.encode(def, secondary, kind, number)?),
        }
    }
    // A bar or rect spans a bin or a time unit's step where nothing ends it.
    for def in &mut r.defs {
        let (primary, secondary) = match def.channel.as_str() {
            "x" => (&e.x, &mut e.x2),
            "y" => (&e.y, &mut e.y2),
            _ => continue,
        };
        if secondary.is_some() || !def.end || def.range || def.band.is_some() {
            continue;
        }
        let Some(Encode::Scaled(primary)) = primary else {
            continue;
        };
        let Some([start, end]) = def.edges() else {
            continue;
        };
        let end = match def.rect_band {
            Some(_) => format!("{start}_offsetted_rect_end").into(),
            None => end,
        };
        *secondary = Some(Encode::Scaled(Channel {
            source: Source::Field(end),
            title: None,
            sort: Sort::default(),
            ..primary.clone()
        }));
    }
    // A sort by another channel sorts by that channel's field and aggregate.
    let by: Vec<(String, Option<Op>, Option<SharedString>)> = r
        .defs
        .iter()
        .map(|def| match &def.derive {
            Derive::Aggregate {
                op: Op::Argmin | Op::Argmax,
                ..
            } => (
                def.channel.clone(),
                None,
                def.derive.name(def.field.as_ref()),
            ),
            Derive::Aggregate { op, measured, .. } => {
                (def.channel.clone(), Some(*op), measured.clone())
            }
            _ => (def.channel.clone(), None, def.field.clone()),
        })
        .collect();
    for def in &mut r.defs {
        let Some(sort) = &mut def.sort else {
            continue;
        };
        if let Some(channel) = sort.channel.take() {
            let (_, op, field) = by.iter().find(|(name, ..)| *name == channel)?;
            sort.op = *op;
            sort.field = field.clone();
        }
    }
    Some((encoding, r.defs))
}

/// The channel named `name` that reads a field or datum.
pub(crate) fn scaled_mut<'a>(e: &'a mut Encoding, name: &str) -> Option<&'a mut Channel> {
    fn scaled<V>(encode: &mut Option<Encode<V>>) -> Option<&mut Channel> {
        match encode {
            Some(Encode::Scaled(channel)) => Some(channel),
            _ => None,
        }
    }
    match name {
        "x" => scaled(&mut e.x),
        "y" => scaled(&mut e.y),
        "x2" => scaled(&mut e.x2),
        "y2" => scaled(&mut e.y2),
        "xOffset" => scaled(&mut e.x_offset),
        "yOffset" => scaled(&mut e.y_offset),
        "theta" => scaled(&mut e.theta),
        "theta2" => scaled(&mut e.theta2),
        "radius" => scaled(&mut e.radius),
        "radius2" => scaled(&mut e.radius2),
        "color" => scaled(&mut e.color),
        "fill" => scaled(&mut e.fill),
        "stroke" => scaled(&mut e.stroke),
        "opacity" => scaled(&mut e.opacity),
        "fillOpacity" => scaled(&mut e.fill_opacity),
        "strokeOpacity" => scaled(&mut e.stroke_opacity),
        "size" => scaled(&mut e.size),
        "strokeWidth" => scaled(&mut e.stroke_width),
        "strokeDash" => scaled(&mut e.stroke_dash),
        "shape" => scaled(&mut e.shape),
        "angle" => scaled(&mut e.angle),
        "text" => scaled(&mut e.text),
        "href" => scaled(&mut e.href),
        "url" => scaled(&mut e.url),
        "key" => e.key.as_mut(),
        _ => None,
    }
}

impl Reader {
    /// A value definition read by `value`, or a field or datum definition. A
    /// secondary channel takes `kind` from its primary and states no type.
    fn encode<V>(
        &mut self,
        def: &Value,
        name: &str,
        kind: Option<Kind>,
        value: fn(&Value) -> Option<V>,
    ) -> Option<Encode<V>> {
        let def = def.as_object()?;
        match def.get("value") {
            Some(v) if def.len() == 1 => Some(Encode::Value(value(v)?)),
            Some(_) => None,
            None => Some(Encode::Scaled(self.channel(def, name, kind)?)),
        }
    }

    /// One field definition, or a list of them.
    fn list(&mut self, def: &Value, name: &str) -> Option<Vec<Channel>> {
        match def {
            Value::Array(defs) => defs
                .iter()
                .map(|def| self.channel(def.as_object()?, name, None))
                .collect(),
            def => Some(vec![self.channel(def.as_object()?, name, None)?]),
        }
    }

    /// The field definitions of `tooltip`, which draws nothing but groups an
    /// aggregate as any other field does. A definition this does not read
    /// is passed over.
    fn tooltip(&mut self, def: &Value) {
        let defs = match def {
            Value::Array(defs) => defs.iter().collect(),
            def => vec![def],
        };
        for def in defs.into_iter().filter_map(Value::as_object) {
            if !def.contains_key("field") && !def.contains_key("aggregate") {
                continue;
            }
            let read: Map<String, Value> = def
                .iter()
                .filter(|(key, _)| {
                    ["field", "type", "aggregate", "bin", "timeUnit"].contains(&key.as_str())
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let _ = self.channel(&read, "tooltip", None);
        }
    }

    fn channel(
        &mut self,
        def: &Map<String, Value>,
        name: &str,
        inherited: Option<Kind>,
    ) -> Option<Channel> {
        if def.keys().any(|key| !CHANNEL.contains(&key.as_str())) {
            return None;
        }
        let derive = Derive::read(def, name)?;
        let source = match (def.get("field"), def.get("datum"), &derive) {
            (Some(Value::String(field)), None, _) => Source::Field(field.clone().into()),
            (None, None, Derive::Aggregate { op: Op::Count, .. }) => {
                Source::Field(field::COUNT.into())
            }
            (None, Some(Value::Number(n)), Derive::None) => {
                Source::Datum(Datum::Number(n.as_f64()?))
            }
            (None, Some(Value::String(s)), Derive::None) => {
                Source::Datum(Datum::Text(s.clone().into()))
            }
            (None, Some(Value::Object(time)), Derive::None) => {
                Source::Datum(Datum::Number(datetime::time(time)?))
            }
            _ => return None,
        };
        let written = match (&source, def.get("field")) {
            (Source::Field(field), Some(_)) => Some(field.clone()),
            _ => None,
        };
        let scale_type = def
            .get("scale")
            .and_then(|scale| scale.get("type"))
            .and_then(Value::as_str);
        let kind = match (def.get("type"), inherited) {
            (Some(_), Some(_)) => return None,
            (None, Some(kind)) => kind,
            (Some(_), None) if derive.counts() => Kind::Quantitative,
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
                _ if derive.kind().is_some() => derive.kind()?,
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
        let mut record = Def {
            channel: name.to_string(),
            field: written.clone(),
            kind,
            derive,
            sort: None,
            range: false,
            band: None,
            rect_band: None,
            end: false,
            unaggregated: false,
        };
        let positional = matches!(name, "x" | "y");
        let rect = matches!(
            self.mark,
            Some(Mark::Bar | Mark::Rect | Mark::Image | Mark::Arc | Mark::Tick)
        );
        let paired = match name {
            "x" => self.paired[0],
            "y" => self.paired[1],
            _ => false,
        };
        let band_position = match def.get("bandPosition") {
            Some(position) => Some(position.as_f64()?),
            None => None,
        };
        let mut source = match &source {
            Source::Field(_) => Source::Field(record.derive.name(written.as_ref())?),
            Source::Datum(_) => source,
        };
        match &record.derive {
            Derive::Bin(bin) => {
                record.end = true;
                if kind.is_discrete() || matches!(name, "text" | "tooltip") {
                    record.range = true;
                    source = Source::Field(format!("{}_range", bin.output[0]).into());
                } else if positional && !rect {
                    let p = band_position.unwrap_or(0.5);
                    record.band = Some(p);
                    source = Source::Field(field::band(&bin.output[0], p));
                }
            }
            Derive::TimeUnit(_) | Derive::BinnedTimeUnit { .. } => {
                // Bars and rects take Vega-Lite's `timeUnitBandPosition` of
                // 0.5 where nothing ends them.
                let spans = matches!(self.mark, Some(Mark::Bar | Mark::Rect)) && !paired;
                let position = band_position.or(spans.then_some(0.5));
                record.end = position.is_some() && kind == Kind::Temporal;
                if let (Some(p), true, Source::Field(start)) =
                    (position, record.end && positional, &source)
                {
                    if rect && p != 0.5 {
                        record.rect_band = Some(p);
                        source = Source::Field(format!("{start}_offsetted_rect_start").into());
                    } else if !rect {
                        record.band = Some(p);
                        source = Source::Field(field::band(start, p));
                    }
                }
            }
            _ => {}
        }
        let mut out = Channel {
            source,
            kind,
            title: None,
            scale: Box::default(),
            stack: Stack::Auto,
            sort: Sort::default(),
        };

        let guide = ["axis", "legend"]
            .into_iter()
            .filter_map(|key| def.get(key)?.as_object()?.get("title"))
            .next();
        out.title = match guide.or(def.get("title")) {
            None if out.field().is_some() => record.derive.title(written.as_ref()),
            None => None,
            Some(written) => title(written),
        };

        match def.get("scale") {
            None => {}
            Some(Value::Object(written)) => out.scale = Box::new(scale(written, kind, name)?),
            // No scale: each value is what the channel takes.
            Some(Value::Null) => out.scale.kind = Some(ScaleType::Identity),
            Some(_) => return None,
        }
        let unaggregated = def
            .get("scale")
            .and_then(|scale| scale.get("domain"))
            .and_then(Value::as_str)
            == Some("unaggregated");
        // Vega-Lite takes an unaggregated domain from each group's least and
        // greatest value where the aggregate shares the field's domain, and
        // drops it elsewhere.
        if let (true, Derive::Aggregate { op, .. }, Some(field)) =
            (unaggregated, &record.derive, &written)
            && matches!(
                op,
                Op::Mean
                    | Op::Average
                    | Op::Median
                    | Op::Q1
                    | Op::Q3
                    | Op::Min
                    | Op::Max
                    | Op::Argmin
                    | Op::Argmax
            )
            && !(kind == Kind::Quantitative && out.scale.kind == Some(ScaleType::Log))
        {
            record.unaggregated = true;
            out.scale.domain = Some(Domain::Unaggregated(
                [Op::Min, Op::Max].map(|op| field::output(op, field)),
            ));
        }
        if let Derive::Binned(step) = record.derive {
            out.scale.zero = Some(false);
            out.scale.nice = Some(Nice::Off);
            out.scale.bins = step.map(|step| Bins {
                start: f64::NAN,
                stop: f64::NAN,
                step,
            });
        }

        let sorted = match name {
            "order" => Some(Sort::Ascending),
            _ if kind.is_discrete() => Some(Sort::Ascending),
            _ => None,
        };
        // A continuous scale sorts descending by running backwards.
        if sorted.is_none()
            && def.get("sort").and_then(Value::as_str) == Some("descending")
            && out.scale.reverse.is_none()
        {
            out.scale.reverse = Some(true);
        }
        if let Some(default) = sorted {
            // A binned field's text sorts by where each bin starts.
            let by_start = |descending: bool| match &record.derive {
                Derive::Bin(bin) if record.range => Some(SortBy {
                    op: Some(Op::Min),
                    field: Some(bin.output[0].clone()),
                    descending,
                    channel: None,
                }),
                _ => None,
            };
            out.sort = match def.get("sort") {
                None => {
                    record.sort = by_start(false);
                    default
                }
                Some(Value::Null) if name != "order" => Sort::Data,
                Some(Value::String(order)) if order == "ascending" => {
                    record.sort = by_start(false);
                    Sort::Ascending
                }
                Some(Value::String(order)) if order == "descending" => {
                    record.sort = by_start(true);
                    Sort::Descending
                }
                Some(Value::String(channel)) if name != "order" => {
                    let (descending, channel) = match channel.strip_prefix('-') {
                        Some(channel) => (true, channel),
                        None => (false, channel.as_str()),
                    };
                    if !SORT_CHANNELS.contains(&channel) {
                        return None;
                    }
                    record.sort = Some(SortBy {
                        op: None,
                        field: None,
                        descending,
                        channel: Some(channel.to_string()),
                    });
                    Sort::Ascending
                }
                Some(Value::Object(by)) if name != "order" => {
                    record.sort = Some(sort_by(by)?);
                    Sort::Ascending
                }
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

        out.stack = match def.get("stack") {
            None => Stack::Auto,
            Some(Value::Bool(true)) => Stack::Zero,
            Some(Value::Null | Value::Bool(false)) => Stack::Off,
            Some(Value::String(offset)) => match offset.as_str() {
                "zero" => Stack::Zero,
                "normalize" => Stack::Normalize,
                "center" => Stack::Center,
                _ => return None,
            },
            Some(_) => return None,
        };
        self.defs.push(record);
        Some(out)
    }
}

/// A sort object: `{"op", "field", "order"}`, or `{"encoding", "order"}`.
fn sort_by(by: &Map<String, Value>) -> Option<SortBy> {
    if by
        .keys()
        .any(|key| !["op", "field", "order", "encoding"].contains(&key.as_str()))
    {
        return None;
    }
    let descending = match by.get("order") {
        None | Some(Value::Null) => false,
        Some(Value::String(order)) if order == "ascending" => false,
        Some(Value::String(order)) if order == "descending" => true,
        Some(_) => return None,
    };
    if let Some(channel) = by.get("encoding") {
        let channel = channel.as_str()?;
        if by.contains_key("op") || by.contains_key("field") || !SORT_CHANNELS.contains(&channel) {
            return None;
        }
        return Some(SortBy {
            op: None,
            field: None,
            descending,
            channel: Some(channel.to_string()),
        });
    }
    let op = match by.get("op") {
        None => None,
        Some(op) => Some(op.as_str()?.parse::<Op>().ok()?),
    };
    let field = match by.get("field") {
        None => None,
        Some(field) => Some(SharedString::from(field.as_str()?.to_string())),
    };
    Some(SortBy {
        op,
        field,
        descending,
        channel: None,
    })
}

/// A field's or datum's `scale` on channel `name`.
fn scale(written: &Map<String, Value>, kind: Kind, name: &str) -> Option<Scale> {
    let mut scale = Scale::default();
    let number = |value: &Value| value.as_f64();
    for (key, value) in written {
        match (key.as_str(), value) {
            ("type", Value::String(kind)) => scale.kind = Some(kind.parse().ok()?),
            ("zero", Value::Bool(zero)) => scale.zero = Some(*zero),
            ("nice", Value::Bool(nice)) => {
                scale.nice = Some(match nice {
                    true => Nice::Ticks(10.0),
                    false => Nice::Off,
                })
            }
            ("nice", Value::Number(count)) => scale.nice = Some(Nice::Ticks(count.as_f64()?)),
            ("nice", Value::String(unit)) => {
                scale.nice = Some(Nice::Interval {
                    unit: interval(unit)?,
                    step: 1.0,
                })
            }
            ("nice", Value::Object(every)) => {
                if every.keys().any(|key| key != "interval" && key != "step") {
                    return None;
                }
                scale.nice = Some(Nice::Interval {
                    unit: interval(every.get("interval")?.as_str()?)?,
                    step: every.get("step").map_or(Some(1.0), Value::as_f64)?,
                })
            }
            // Read with the field's aggregate.
            ("domain", Value::String(unaggregated)) if unaggregated == "unaggregated" => {}
            ("domain", Value::Array(values)) => scale.domain = Some(domain(values, kind)?),
            ("domainMid", value) => scale.domain_mid = Some(number(value)?),
            ("domainMin", value) => scale.domain_min = Some(end(value, kind)?),
            ("domainMax", value) => scale.domain_max = Some(end(value, kind)?),
            ("rangeMin", value) => scale.range_min = Some(number(value)?),
            ("rangeMax", value) => scale.range_max = Some(number(value)?),
            ("range", value) => scale.range = Some(range(value, name)?),
            ("scheme", value) => scale.scheme = Some(scheme(value)?),
            ("reverse", Value::Bool(reverse)) => scale.reverse = Some(*reverse),
            ("clamp", Value::Bool(clamp)) => scale.clamp = *clamp,
            ("round", Value::Bool(round)) => scale.round = *round,
            ("padding", value) => scale.padding = Some(number(value)?),
            ("paddingInner", value) => scale.padding_inner = Some(number(value)?),
            ("paddingOuter", value) => scale.padding_outer = Some(number(value)?),
            ("interpolate", Value::String(name)) => scale.interpolate = Some(name.parse().ok()?),
            ("interpolate", Value::Object(params)) => {
                let interpolate: Interpolate = params.get("type")?.as_str()?.parse().ok()?;
                scale.interpolate = Some(match params.get("gamma") {
                    Some(gamma) => interpolate.with_gamma(number(gamma)?),
                    None => interpolate,
                });
            }
            ("exponent", value) => scale.exponent = Some(number(value)?),
            ("base", value) => scale.base = Some(number(value)?),
            ("constant", value) => scale.constant = Some(number(value)?),
            _ => return None,
        }
    }
    Some(scale)
}

/// A written domain: a continuous field's numbers or times, a discrete
/// field's values as its column reads them.
fn domain(values: &[Value], kind: Kind) -> Option<Domain> {
    if kind.is_discrete() {
        return Some(Domain::Values(
            values
                .iter()
                .map(|value| match value {
                    Value::String(s) => Some(s.clone().into()),
                    Value::Number(n) => Some(scale::value(n.as_f64()?).into()),
                    Value::Bool(b) => Some(b.to_string().into()),
                    Value::Null => Some(SharedString::default()),
                    Value::Object(time) => Some(scale::value(datetime::time(time)?).into()),
                    Value::Array(_) => None,
                })
                .collect::<Option<_>>()?,
        ));
    }
    Some(Domain::Numbers(
        values
            .iter()
            .map(|value| end(value, kind))
            .collect::<Option<_>>()?,
    ))
}

/// A continuous domain's value: a number, a date-time, or a temporal
/// field's string.
fn end(value: &Value, kind: Kind) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) if kind == Kind::Temporal => time::parse(s),
        Value::Object(time) => datetime::time(time),
        _ => None,
    }
}

/// A d3-time interval by Vega's name.
fn interval(name: &str) -> Option<Unit> {
    Some(match name {
        "millisecond" => Unit::Millisecond,
        "second" => Unit::Second,
        "minute" => Unit::Minute,
        "hour" => Unit::Hour,
        "day" => Unit::Day,
        "week" => Unit::Week,
        "month" => Unit::Month,
        "year" => Unit::Year,
        _ => return None,
    })
}

/// A written range on channel `name`: a named range, or a list of what the
/// channel takes.
fn range(value: &Value, name: &str) -> Option<Range> {
    let values = match value {
        Value::String(named) => return Some(Range::Named(named.parse().ok()?)),
        Value::Array(values) => values,
        Value::Object(field) if field.len() == 1 => {
            return Some(Range::Field(
                field.get("field")?.as_str()?.to_string().into(),
            ));
        }
        _ => return None,
    };
    Some(match name {
        "x" => Range::Positions(values.iter().map(across).collect::<Option<_>>()?),
        "y" => Range::Positions(values.iter().map(down).collect::<Option<_>>()?),
        "color" | "fill" | "stroke" => Range::Colors(
            values
                .iter()
                .map(|value| color(value).filter(|_| value.is_string()))
                .collect::<Option<_>>()?,
        ),
        "shape" => Range::Shapes(values.iter().map(shape).collect::<Option<_>>()?),
        "strokeDash" => Range::Dashes(values.iter().map(dash).collect::<Option<_>>()?),
        _ => Range::Numbers(values.iter().map(Value::as_f64).collect::<Option<_>>()?),
    })
}

/// A scheme by name, a list of colours, or `{name, extent, count}`, its
/// name either of those.
fn scheme(value: &Value) -> Option<Scheme> {
    match value {
        Value::String(name) => Some(name.parse::<palette::Scheme>().ok()?.into()),
        Value::Array(colors) => Some(Scheme {
            palette: Palette::Colors(
                colors
                    .iter()
                    .map(|value| color(value).filter(|_| value.is_string()))
                    .collect::<Option<_>>()?,
            ),
            extent: None,
            count: None,
        }),
        Value::Object(params) => {
            let mut scheme = match params.get("name")? {
                Value::Object(_) => return None,
                name => scheme(name)?,
            };
            for (key, value) in params {
                match key.as_str() {
                    "name" => {}
                    "extent" => {
                        let [lo, hi] = value.as_array()?.as_slice() else {
                            return None;
                        };
                        scheme.extent = Some([lo.as_f64()?, hi.as_f64()?]);
                    }
                    "count" => scheme.count = Some(value.as_u64()? as usize),
                    _ => return None,
                }
            }
            Some(scheme)
        }
        _ => None,
    }
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
    match value {
        Value::Null => Some(Hsla::transparent_black()),
        value => Some(color::hsla(value.as_str()?.parse().ok()?)),
    }
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
