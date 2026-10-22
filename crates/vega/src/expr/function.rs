//! The built-in functions, by Vega's names.

use crate::{
    expr::{
        Context, Error, array,
        date::{self, Field},
        parse::Node,
        string as text,
    },
    value::{Value, regexp::RegExp},
};
use d3::{
    format::{InvalidSpecifier, NumberFormat, number},
    string,
    time::date::{Date, LocalZone},
};
use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    str::FromStr,
};

static UNDEFINED: Value = Value::Undefined;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Function {
    IsArray,
    IsBoolean,
    IsDate,
    IsDefined,
    IsNumber,
    IsObject,
    IsRegExp,
    IsString,
    IsValid,
    ToBoolean,
    ToDate,
    ToNumber,
    ToString,
    If,
    IsNaN,
    IsFinite,
    /// A `Math` function of one number.
    Math(fn(f64) -> f64),
    Atan2,
    Clamp,
    Hypot,
    Max,
    Min,
    Pow,
    Random,
    Now,
    Datetime,
    Utc,
    Get(Field),
    UtcGet(Field),
    TimezoneOffset,
    TimeOffset,
    UtcOffset,
    TimeSequence,
    UtcSequence,
    Extent,
    ClampRange,
    IndexOf,
    InRange,
    Join,
    LastIndexOf,
    Length,
    Lerp,
    Peek,
    Pluck,
    Reverse,
    Sequence,
    Slice,
    Sort,
    Span,
    Lower,
    Pad,
    ParseFloat,
    ParseInt,
    Replace,
    Split,
    Substring,
    Trim,
    Truncate,
    Upper,
    RegExp,
    Test,
    Format,
    TimeFormat,
    UtcFormat,
    TimeParse,
    UtcParse,
    Bandspace,
    Bandwidth,
    Domain,
    Range,
    Scale,
    /// The encoders' `_scale`, `_bandwidth` and `_range`.
    EncodeScale,
    EncodeBandwidth,
    EncodeRange,
    Flush,
}

impl FromStr for Function {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Ok(match name {
            "isArray" => Self::IsArray,
            "isBoolean" => Self::IsBoolean,
            "isDate" => Self::IsDate,
            "isDefined" => Self::IsDefined,
            "isNumber" => Self::IsNumber,
            "isObject" => Self::IsObject,
            "isRegExp" => Self::IsRegExp,
            "isString" => Self::IsString,
            "isValid" => Self::IsValid,
            "toBoolean" => Self::ToBoolean,
            "toDate" => Self::ToDate,
            "toNumber" => Self::ToNumber,
            "toString" => Self::ToString,
            "if" => Self::If,
            "isNaN" => Self::IsNaN,
            "isFinite" => Self::IsFinite,
            "abs" => Self::Math(f64::abs),
            "acos" => Self::Math(f64::acos),
            "asin" => Self::Math(f64::asin),
            "atan" => Self::Math(f64::atan),
            "ceil" => Self::Math(f64::ceil),
            "cos" => Self::Math(f64::cos),
            "exp" => Self::Math(f64::exp),
            "floor" => Self::Math(f64::floor),
            "log" => Self::Math(f64::ln),
            "round" => Self::Math(number::round),
            "sin" => Self::Math(f64::sin),
            "sqrt" => Self::Math(f64::sqrt),
            "tan" => Self::Math(f64::tan),
            "atan2" => Self::Atan2,
            "clamp" => Self::Clamp,
            "hypot" => Self::Hypot,
            "max" => Self::Max,
            "min" => Self::Min,
            "pow" => Self::Pow,
            "random" => Self::Random,
            "now" => Self::Now,
            "datetime" => Self::Datetime,
            "utc" => Self::Utc,
            "timezoneoffset" => Self::TimezoneOffset,
            "timeOffset" => Self::TimeOffset,
            "utcOffset" => Self::UtcOffset,
            "timeSequence" => Self::TimeSequence,
            "utcSequence" => Self::UtcSequence,
            "extent" => Self::Extent,
            "clampRange" => Self::ClampRange,
            "indexof" => Self::IndexOf,
            "inrange" => Self::InRange,
            "join" => Self::Join,
            "lastindexof" => Self::LastIndexOf,
            "length" => Self::Length,
            "lerp" => Self::Lerp,
            "peek" => Self::Peek,
            "pluck" => Self::Pluck,
            "reverse" => Self::Reverse,
            "sequence" => Self::Sequence,
            "slice" => Self::Slice,
            "sort" => Self::Sort,
            "span" => Self::Span,
            "lower" => Self::Lower,
            "pad" => Self::Pad,
            "parseFloat" => Self::ParseFloat,
            "parseInt" => Self::ParseInt,
            "replace" => Self::Replace,
            "split" => Self::Split,
            "substring" => Self::Substring,
            "trim" => Self::Trim,
            "truncate" => Self::Truncate,
            "upper" => Self::Upper,
            "regexp" => Self::RegExp,
            "test" => Self::Test,
            "format" => Self::Format,
            "timeFormat" => Self::TimeFormat,
            "utcFormat" => Self::UtcFormat,
            "timeParse" => Self::TimeParse,
            "utcParse" => Self::UtcParse,
            "bandspace" => Self::Bandspace,
            "bandwidth" => Self::Bandwidth,
            "domain" => Self::Domain,
            "range" => Self::Range,
            "scale" => Self::Scale,
            "_scale" => Self::EncodeScale,
            "_bandwidth" => Self::EncodeBandwidth,
            "_range" => Self::EncodeRange,
            "flush" => Self::Flush,
            _ => match name.strip_prefix("utc") {
                Some(field) => match field.parse()? {
                    Field::Time => return Err(()),
                    field => Self::UtcGet(field),
                },
                None => Self::Get(name.parse()?),
            },
        })
    }
}

impl Function {
    pub(crate) fn call(self, args: &[Node], row: &Context<'_, '_>) -> Result<Value, Error> {
        if let Self::If = self {
            let [test, then, otherwise] = args else {
                unreachable!("`if` parses with three arguments");
            };
            return if test.eval(row)?.truthy() {
                then.eval(row)
            } else {
                otherwise.eval(row)
            };
        }
        let args = args
            .iter()
            .map(|arg| arg.eval(row))
            .collect::<Result<Vec<_>, _>>()?;
        let arg = |i: usize| args.get(i).unwrap_or(&UNDEFINED);
        let scale = || (row.scale)(&arg(0).to_string());
        let numbers = |values: Vec<f64>| -> Value {
            values
                .into_iter()
                .map(Value::Number)
                .collect::<Vec<_>>()
                .into()
        };
        Ok(match self {
            Self::Bandspace => {
                let n = |i: usize| match arg(i).to_number() {
                    n if n.is_nan() => 0.0,
                    n => n,
                };
                Value::Number(d3::scale::Band::space(n(0) as usize, n(1), n(2)))
            }
            Self::Bandwidth | Self::EncodeBandwidth => {
                Value::Number(scale().map_or(0.0, |s| s.bandwidth()))
            }
            Self::Domain => scale().map_or_else(|| Vec::new().into(), |s| s.domain().into()),
            Self::Range | Self::EncodeRange => {
                scale().map_or_else(|| Vec::new().into(), |s| numbers(s.range()))
            }
            Self::Scale | Self::EncodeScale => {
                scale().map_or(Value::Undefined, |s| s.apply(arg(1)))
            }
            Self::Flush => {
                let threshold = arg(2);
                let zero = matches!(threshold, Value::Number(n) if *n == 0.0);
                let range = array::items(arg(0), "flush")?;
                match (range.first(), range.last()) {
                    _ if !threshold.truthy() && !zero => arg(5).clone(),
                    (Some(a), Some(b)) if !matches!(b, Value::Undefined) => {
                        let (mut a, mut b) = (a.to_number(), b.to_number());
                        if b < a {
                            (a, b) = (b, a);
                        }
                        let (t, value) = (threshold.to_number(), arg(1).to_number());
                        let (l, r) = ((value - a).abs(), (b - value).abs());
                        match () {
                            _ if l < r && l <= t => arg(3).clone(),
                            _ if r <= t => arg(4).clone(),
                            _ => arg(5).clone(),
                        }
                    }
                    _ => arg(5).clone(),
                }
            }
            _ => return self.apply(&args),
        })
    }

    fn apply(self, args: &[Value]) -> Result<Value, Error> {
        let arg = |i: usize| args.get(i).unwrap_or(&UNDEFINED);
        let number = |i: usize| arg(i).to_number();
        let (a, b, c) = (arg(0), arg(1), arg(2));
        let null_or_empty = a.is_nullish() || matches!(a, Value::String(s) if s.is_empty());
        Ok(match self {
            Self::IsArray => matches!(a, Value::Array(_)).into(),
            Self::IsBoolean => matches!(a, Value::Bool(_)).into(),
            Self::IsDate => false.into(),
            Self::IsDefined => (!matches!(a, Value::Undefined)).into(),
            Self::IsNumber => matches!(a, Value::Number(_)).into(),
            Self::IsObject => a.is_object().into(),
            Self::IsRegExp => matches!(a, Value::RegExp(_)).into(),
            Self::IsString => matches!(a, Value::String(_)).into(),
            Self::IsValid => {
                (!a.is_nullish() && !matches!(a, Value::Number(n) if n.is_nan())).into()
            }
            Self::ToBoolean if null_or_empty => Value::Null,
            Self::ToBoolean => {
                let text = matches!(a, Value::String(s) if &**s == "false" || &**s == "0");
                (a.truthy() && !text).into()
            }
            Self::ToDate if null_or_empty => Value::Null,
            Self::ToDate => match a {
                Value::Number(n) => Value::Number(*n),
                _ => Value::Number(LocalZone::default().parse(&a.to_string()).time()),
            },
            Self::ToNumber if null_or_empty => Value::Null,
            Self::ToNumber => Value::Number(a.to_number()),
            Self::ToString if null_or_empty => Value::Null,
            Self::ToString => a.to_string().into(),
            Self::If => unreachable!("`if` evaluates its own arguments"),
            Self::IsNaN => number(0).is_nan().into(),
            Self::IsFinite => number(0).is_finite().into(),
            Self::Math(f) => Value::Number(f(number(0))),
            Self::Atan2 => Value::Number(number(0).atan2(number(1))),
            Self::Clamp => Value::Number(number::max(number(1), number::min(number(2), number(0)))),
            Self::Hypot => Value::Number(number::hypot(
                &args.iter().map(Value::to_number).collect::<Vec<_>>(),
            )),
            Self::Max => Value::Number(
                args.iter()
                    .map(Value::to_number)
                    .fold(f64::NEG_INFINITY, number::max),
            ),
            Self::Min => Value::Number(
                args.iter()
                    .map(Value::to_number)
                    .fold(f64::INFINITY, number::min),
            ),
            Self::Pow => Value::Number(number::pow(number(0), number(1))),
            // Uniform in `[0, 1)`. Not for cryptography.
            Self::Random => {
                let bits = RandomState::new().build_hasher().finish();
                Value::Number((bits >> 11) as f64 / (1u64 << 53) as f64)
            }
            Self::Now => Value::Number(Date::now()),
            Self::Datetime => Value::Number(match args.len() {
                0 => Date::now(),
                1 => date::time(a),
                _ => {
                    let [y, m, d, h, min, s, ms] = date::parts(args);
                    LocalZone::default().local(y, m, d, h, min, s, ms).time()
                }
            }),
            Self::Utc => {
                let [y, m, d, h, min, s, ms] = date::parts(args);
                Value::Number(Date::utc(y, m, d, h, min, s, ms))
            }
            Self::Get(field) | Self::UtcGet(field) => {
                Value::Number(field.of(self.zone().date(date::time(a))))
            }
            Self::TimezoneOffset => {
                Value::Number(LocalZone::default().date(date::time(a)).timezone_offset())
            }
            Self::TimeOffset | Self::UtcOffset => {
                let zone = self.zone();
                let step = if c.is_nullish() {
                    1.0
                } else {
                    number(2).floor()
                };
                let mut at = zone.date(date::time(b));
                date::interval(&a.to_string(), &zone)?.offset(&mut at, step);
                Value::Number(at.time())
            }
            Self::TimeSequence | Self::UtcSequence => {
                let zone = self.zone();
                let step = if arg(3).is_nullish() { 1.0 } else { number(3) };
                date::interval(&a.to_string(), &zone)?
                    .range(date::time(b), date::time(c), step, &zone)
                    .into_iter()
                    .map(Value::Number)
                    .collect::<Vec<_>>()
                    .into()
            }
            Self::Extent => array::extent(a)?,
            Self::ClampRange => array::clamp_range(a, number(1), number(2))?,
            Self::IndexOf => array::index_of(a, b, false)?,
            Self::LastIndexOf => array::index_of(a, b, true)?,
            Self::InRange => array::in_range(a, b)?,
            Self::Join => array::join(array::items(a, "join")?, b),
            Self::Length => a.get(&"length".into())?,
            Self::Lerp => array::lerp(array::items(a, "lerp")?, b),
            Self::Peek => array::items(a, "peek")?.last().cloned().unwrap_or_default(),
            Self::Pluck => array::pluck(array::items(a, "pluck")?, &b.to_string())?,
            Self::Reverse => array::items(a, "reverse")?
                .iter()
                .rev()
                .cloned()
                .collect::<Vec<_>>()
                .into(),
            Self::Sequence => array::sequence(args),
            Self::Slice => array::slice(a, b, c)?,
            Self::Sort => array::sort(array::items(a, "sort")?),
            Self::Span => array::span(a)?,
            Self::Lower => a.to_string().to_lowercase().into(),
            Self::Upper => a.to_string().to_uppercase().into(),
            Self::Pad => text::pad(&a.to_string(), number(1), c, arg(3)).into(),
            Self::ParseFloat => Value::Number(number::parse_float(&a.to_string())),
            Self::ParseInt => Value::Number(number::parse_int(&a.to_string(), b.to_int32())),
            Self::Replace => text::replace(&a.to_string(), b, &c.to_string()).into(),
            Self::Split => text::split(&a.to_string(), b, c),
            Self::Substring => text::substring(&a.to_string(), b, c).into(),
            Self::Trim => string::trim(&a.to_string()).into(),
            Self::Truncate => text::truncate(&a.to_string(), number(1), c, arg(3)).into(),
            Self::RegExp => match a {
                Value::RegExp(regexp) if b.is_nullish() => Value::RegExp(regexp.clone()),
                _ => {
                    let source = if matches!(a, Value::Undefined) {
                        String::new()
                    } else {
                        a.to_string()
                    };
                    let flags = if matches!(b, Value::Undefined) {
                        String::new()
                    } else {
                        b.to_string()
                    };
                    RegExp::new(&source, &flags)?.into()
                }
            },
            Self::Test => {
                let regexp = match a {
                    Value::RegExp(regexp) => regexp.clone(),
                    _ => RegExp::new(&a.to_string(), "")?.into(),
                };
                regexp.test(&b.to_string()).into()
            }
            Self::Format => {
                let specifier = b.to_string();
                let format: NumberFormat = specifier
                    .parse()
                    .map_err(|e: InvalidSpecifier| Error::Runtime(e.to_string()))?;
                format.format(a.to_number()).into()
            }
            Self::TimeFormat | Self::UtcFormat => {
                let zone = self.zone();
                zone.date(date::time(a)).format(&b.to_string()).into()
            }
            Self::TimeParse | Self::UtcParse => {
                let zone = self.zone();
                match zone.time_parse(&b.to_string(), &a.to_string()) {
                    Some(date) => Value::Number(date.time()),
                    None => Value::Null,
                }
            }
            Self::Bandspace
            | Self::Bandwidth
            | Self::Domain
            | Self::Range
            | Self::Scale
            | Self::EncodeScale
            | Self::EncodeBandwidth
            | Self::EncodeRange
            | Self::Flush => unreachable!("read in `call`, with the context"),
        })
    }

    /// The zone a time function reads dates in: UTC for the `utc` ones, else
    /// the host's.
    fn zone(self) -> LocalZone {
        match self {
            Self::UtcGet(_)
            | Self::UtcOffset
            | Self::UtcSequence
            | Self::UtcFormat
            | Self::UtcParse => LocalZone::utc(),
            _ => LocalZone::default(),
        }
    }
}
