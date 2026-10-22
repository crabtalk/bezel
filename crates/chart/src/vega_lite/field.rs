//! A field definition's `aggregate`, `bin` and `timeUnit`: what Vega-Lite
//! derives the channel's field from, the name it gives the derived field,
//! and its default title.

use crate::{
    model::Kind,
    transform::{Bin, Op, Part, TimeUnit},
};
use gpui::SharedString;
use js::number::Number as JsNumber;
use serde_json::{Map, Value};

/// Vega-Lite's name for the count of a group's rows.
pub(crate) const COUNT: &str = "__count";
/// Vega-Lite's title for `count`.
const COUNT_TITLE: &str = "Count of Records";
/// Vega-Lite's unit names, single and combined; `utc` before any is its
/// twin on the UTC calendar.
const UNITS: [&str; 40] = [
    "year",
    "quarter",
    "month",
    "week",
    "day",
    "dayofyear",
    "date",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "yearquarter",
    "yearquartermonth",
    "yearmonth",
    "yearmonthdate",
    "yearmonthdatehours",
    "yearmonthdatehoursminutes",
    "yearmonthdatehoursminutesseconds",
    "yearweek",
    "yearweekday",
    "yearweekdayhours",
    "yearweekdayhoursminutes",
    "yearweekdayhoursminutesseconds",
    "yeardayofyear",
    "quartermonth",
    "monthdate",
    "monthdatehours",
    "monthdatehoursminutes",
    "monthdatehoursminutesseconds",
    "weekday",
    "weekdayhours",
    "weekdayhoursminutes",
    "weekdayhoursminutesseconds",
    "dayhours",
    "dayhoursminutes",
    "dayhoursminutesseconds",
    "hoursminutes",
    "hoursminutesseconds",
    "minutesseconds",
    "secondsmilliseconds",
];
/// The bin parameters Vega-Lite takes.
const BIN: [&str; 10] = [
    "anchor", "base", "binned", "divide", "extent", "maxbins", "minstep", "nice", "step", "steps",
];

/// What a field definition derives its channel's field from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Derive {
    /// The field as it is.
    None,
    /// One value per group of the encoding's other fields: `op` of
    /// `measured`, named `output`.
    Aggregate {
        op: Op,
        measured: Option<SharedString>,
        param: Option<f64>,
        output: SharedString,
    },
    /// The field binned; the channel reads where each bin starts.
    Bin(Bin),
    /// The field holds bins' starts already, `step` apart if given.
    Binned(Option<f64>),
    /// The field truncated to a time unit; the channel reads where each
    /// step starts.
    TimeUnit(TimeUnit),
    /// The field holds times truncated to `units` already.
    BinnedTimeUnit {
        units: Vec<Part>,
        step: f64,
        utc: bool,
    },
}

impl Derive {
    /// The `aggregate`, `bin` and `timeUnit` of `def` on `channel`. `None` for
    /// one this does not read, more than one of them, or an aggregate on a
    /// field that is binned or truncated.
    pub(crate) fn read(def: &Map<String, Value>, channel: &str) -> Option<Self> {
        let field = def.get("field").and_then(Value::as_str);
        let derived = [def.get("aggregate"), def.get("bin"), def.get("timeUnit")]
            .iter()
            .filter(|v| !matches!(v, None | Some(Value::Bool(false)) | Some(Value::Null)))
            .count();
        if derived > 1 {
            return None;
        }
        if let Some(aggregate) = def.get("aggregate") {
            return aggregate_def(aggregate, field);
        }
        if let Some(bin) = def.get("bin").filter(|bin| **bin != Value::Bool(false)) {
            return bin_def(bin, field?, channel);
        }
        if let Some(unit) = def.get("timeUnit") {
            return unit_def(unit, field?);
        }
        Some(Self::None)
    }

    /// The field the channel reads: Vega-Lite's name for what it derives.
    pub(crate) fn name(&self, field: Option<&SharedString>) -> Option<SharedString> {
        match self {
            Self::Aggregate {
                op: Op::Argmin | Op::Argmax,
                output,
                ..
            } => Some(format!("{output}.{}", field?).into()),
            Self::Aggregate { output, .. } => Some(output.clone()),
            Self::Bin(bin) => Some(bin.output[0].clone()),
            Self::TimeUnit(unit) => Some(unit.output[0].clone()),
            Self::None | Self::Binned(_) | Self::BinnedTimeUnit { .. } => field.cloned(),
        }
    }

    /// The parts a time unit truncates to, and how many of the least a step
    /// takes.
    pub(crate) fn units(&self) -> Option<(&[Part], f64)> {
        match self {
            Self::TimeUnit(unit) => Some((&unit.units, unit.step)),
            Self::BinnedTimeUnit { units, step, .. } => Some((units, *step)),
            _ => None,
        }
    }

    /// Vega-Lite's type for a field definition that states none.
    pub(crate) fn kind(&self) -> Option<Kind> {
        match self {
            Self::TimeUnit(_) | Self::BinnedTimeUnit { .. } => Some(Kind::Temporal),
            Self::Aggregate {
                op: Op::Argmin | Op::Argmax,
                ..
            } => None,
            Self::Aggregate { .. } | Self::Bin(_) | Self::Binned(_) => Some(Kind::Quantitative),
            Self::None => None,
        }
    }

    /// Whether the op counts, which makes its field quantitative whatever
    /// type it states.
    pub(crate) fn counts(&self) -> bool {
        matches!(
            self,
            Self::Aggregate {
                op: Op::Count | Op::Valid | Op::Missing | Op::Distinct,
                ..
            }
        )
    }

    /// Vega-Lite's default title.
    pub(crate) fn title(&self, field: Option<&SharedString>) -> Option<SharedString> {
        let field = field.map(SharedString::as_ref).unwrap_or("");
        Some(match self {
            Self::Aggregate { op: Op::Count, .. } => COUNT_TITLE.into(),
            Self::Aggregate {
                op: Op::Argmax,
                measured,
                ..
            } => format!("{field} for max {}", measured.as_ref()?).into(),
            Self::Aggregate {
                op: Op::Argmin,
                measured,
                ..
            } => format!("{field} for min {}", measured.as_ref()?).into(),
            Self::Aggregate { op, .. } => {
                let name = op.to_string();
                let (first, rest) = name.split_at(1);
                format!("{}{rest} of {field}", first.to_uppercase()).into()
            }
            Self::Bin(_) => format!("{field} (binned)").into(),
            Self::TimeUnit(unit) => {
                let parts: Vec<&str> = unit.units.iter().map(|part| part.name()).collect();
                format!("{field} ({})", parts.join("-")).into()
            }
            Self::None | Self::Binned(_) | Self::BinnedTimeUnit { .. } => field.to_string().into(),
        })
    }
}

/// Vega-Lite's name for `op` of `field`.
pub(crate) fn output(op: Op, field: &str) -> SharedString {
    format!("{op}_{field}").into()
}

/// `aggregate`: an op's name, or `{"argmin": field}`, `{"argmax": field}`,
/// `{"exponential": weight}` or `{"exponentialb": weight}`.
pub(crate) fn aggregate_def(aggregate: &Value, field: Option<&str>) -> Option<Derive> {
    let (op, measured, param) = match aggregate {
        Value::String(op) => (op.parse::<Op>().ok()?, field.map(SharedString::from), None),
        Value::Object(def) if def.len() == 1 => {
            let (name, value) = def.iter().next()?;
            let op: Op = name.parse().ok()?;
            match op {
                Op::Argmin | Op::Argmax => (op, Some(value.as_str()?.to_string().into()), None),
                Op::Exponential | Op::Exponentialb => {
                    (op, field.map(SharedString::from), Some(value.as_f64()?))
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let output = match op {
        Op::Count => COUNT.into(),
        _ => output(op, measured.as_ref()?),
    };
    if op != Op::Count && measured.is_none() {
        return None;
    }
    Some(Derive::Aggregate {
        op,
        measured: measured.filter(|_| op != Op::Count),
        param,
        output,
    })
}

/// `bin`: `true`, `"binned"`, or parameters. Unless `step` or `maxbins` is
/// given, Vega-Lite's `autoMaxBins`: at most 6 bins on `row`, `column`,
/// `size`, a colour, `strokeWidth`, an opacity and `shape`, 4 on
/// `strokeDash` and 10 elsewhere.
pub(crate) fn bin_def(bin: &Value, field: &str, channel: &str) -> Option<Derive> {
    let maxbins = match channel {
        "row" | "column" | "size" | "color" | "fill" | "stroke" | "strokeWidth" | "opacity"
        | "fillOpacity" | "strokeOpacity" | "shape" => 6,
        "strokeDash" => 4,
        _ => 10,
    };
    let mut params = match bin {
        Value::Bool(true) => Map::new(),
        Value::String(binned) if binned == "binned" => return Some(Derive::Binned(None)),
        Value::Object(params) => params.clone(),
        _ => return None,
    };
    if params.keys().any(|key| !BIN.contains(&key.as_str())) {
        return None;
    }
    if params.get("binned").and_then(Value::as_bool) == Some(true) {
        let step = match params.get("step") {
            Some(step) => Some(step.as_f64()?),
            None => None,
        };
        return Some(Derive::Binned(step));
    }
    if !params.contains_key("maxbins") && !params.contains_key("step") {
        params.insert("maxbins".into(), maxbins.into());
    }
    let mut out = Bin::new(field);
    let numbers = |value: &Value| -> Option<Vec<f64>> {
        value.as_array()?.iter().map(Value::as_f64).collect()
    };
    for (key, value) in &params {
        match key.as_str() {
            "anchor" => out.anchor = Some(value.as_f64()?),
            "base" => out.base = value.as_f64()?,
            "binned" => {
                value.as_bool()?;
            }
            "divide" => out.divide = numbers(value)?,
            "extent" => {
                let [low, high] = numbers(value)?[..] else {
                    return None;
                };
                out.extent = Some([low, high]);
            }
            "maxbins" => out.maxbins = value.as_f64()?,
            "minstep" => out.minstep = value.as_f64()?,
            "nice" => out.nice = value.as_bool()?,
            "step" => out.step = Some(value.as_f64()?),
            _ => out.steps = Some(numbers(value)?),
        }
    }
    let name: String = params
        .iter()
        .map(|(key, value)| var_name(&format!("_{key}_{}", string(value))))
        .collect();
    let start = format!("bin{name}_{field}");
    out.output = [start.clone().into(), format!("{start}_end").into()];
    Some(Derive::Bin(out))
}

/// `timeUnit`: a unit's name, or `{unit, step, utc, maxbins, binned}` with
/// a `unit`. A name starting `binned` is a unit the field holds already.
pub(crate) fn unit_def(unit: &Value, field: &str) -> Option<Derive> {
    let mut params = match unit {
        Value::String(name) => match name.strip_prefix("binned") {
            Some(name) => {
                let mut params = Map::new();
                params.insert("unit".into(), name.into());
                params.insert("binned".into(), true.into());
                params
            }
            None => {
                let mut params = Map::new();
                params.insert("unit".into(), name.clone().into());
                params
            }
        },
        Value::Object(params) => params.clone(),
        _ => return None,
    };
    if params
        .keys()
        .any(|key| !["unit", "step", "utc", "maxbins", "binned"].contains(&key.as_str()))
    {
        return None;
    }
    let mut utc = match params.remove("utc") {
        None => false,
        Some(utc) => utc.as_bool()?,
    };
    let mut name = params.get("unit")?.as_str()?.to_string();
    if let Some(local) = name.strip_prefix("utc") {
        utc = true;
        name = local.to_string();
    }
    if !UNITS.contains(&name.as_str()) {
        return None;
    }
    let parts: Vec<Part> = Part::NAMES
        .iter()
        .filter(|(_, part)| contains(&name, part))
        .map(|(part, _)| *part)
        .collect();
    params.insert("unit".into(), name.into());
    let step = match params.get("step") {
        None => 1.0,
        Some(step) => step.as_f64()?,
    };
    if params.get("binned").and_then(Value::as_bool) == Some(true) {
        return Some(Derive::BinnedTimeUnit {
            units: parts,
            step,
            utc,
        });
    }
    let named: String = params
        .iter()
        .map(|(key, value)| match key.as_str() {
            "unit" => var_name(&string(value)),
            key => var_name(&format!("_{key}_{}", string(value))),
        })
        .collect();
    let name = if utc { format!("utc{named}") } else { named };
    let mut out = TimeUnit::new(field, parts);
    out.step = step;
    out.utc = utc;
    let start = format!("{name}_{field}");
    out.output = [start.clone().into(), format!("{start}_end").into()];
    Some(Derive::TimeUnit(out))
}

/// The field of the place `p` of the way from each `start` to its end.
pub(crate) fn band(start: &str, p: f64) -> SharedString {
    format!("{start}_band_{}", JsNumber(p)).into()
}

/// Whether Vega-Lite's unit `full` holds `part`.
fn contains(full: &str, part: &str) -> bool {
    let Some(index) = full.find(part) else {
        return false;
    };
    let before = full[..index].chars().next_back();
    let after = full[index..].chars().nth(3);
    !(part == "seconds" && before == Some('i')
        || part == "day" && after == Some('o')
        || part == "year" && before == Some('f'))
}

/// Vega-Lite's `varName`: each character but a letter, digit or `_` as `_`,
/// and `_` before leading digits.
fn var_name(text: &str) -> String {
    let name: String = text
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() || c == '_' {
            true => c,
            false => '_',
        })
        .collect();
    match text.starts_with(|c: char| c.is_ascii_digit()) {
        true => format!("_{name}"),
        false => name,
    }
}

/// `${value}` in JavaScript.
fn string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(n) => JsNumber(n.as_f64().unwrap_or(f64::NAN)).to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => "null".into(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::Null => String::new(),
                item => string(item),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}
