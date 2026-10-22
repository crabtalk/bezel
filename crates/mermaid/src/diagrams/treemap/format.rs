//! Pele's `diagrams/treemap/format.ts`.

use js::number::Number;
use regex::Regex;
use std::sync::LazyLock;

/// The parts of d3-format's specifier that treemap values use: an optional
/// `$`, `,` for thousands separators, a precision, and a type of `f`, `%`,
/// `d`, `e` or none. Fill, alignment and width are read and ignored. Anything
/// else falls back to the default, `,`.
static RE_SPEC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:[^\n\r\u{2028}\u{2029}]?[<>=^])?([-+( ])?(\$)?0?[0-9]*(,)?(?:\.([0-9]+))?(~)?([a-zA-Z%])?$",
    )
    .expect("pattern")
});
static RE_ZEROS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.?0+$").expect("pattern"));

fn group(digits: &str) -> String {
    let mut out = String::new();
    let mut i = digits.len();
    while i > 0 {
        let from = i.saturating_sub(3);
        out = if out.is_empty() {
            digits[from..i].to_string()
        } else {
            format!("{},{out}", &digits[from..i])
        };
        i = from;
    }
    out
}

/// The type of a specifier: how the digits are written.
enum Notation {
    /// `f`: a fixed number of decimals.
    Fixed,
    /// `%`: times a hundred, with a percent sign.
    Percent,
    /// `d`: rounded to a whole number.
    Integer,
    /// `e`: in exponent notation.
    Exponent,
    /// Any other type, or none: significant digits.
    Significant,
}

pub struct Formatter {
    plus: bool,
    currency: bool,
    comma: bool,
    trim: bool,
    precision: Option<usize>,
    notation: Notation,
}

impl Formatter {
    pub fn new(spec: &str) -> Self {
        // Mermaid documents `$0,0` for a dollar amount with thousands
        // separators.
        let spec = if spec == "$0,0" { "$," } else { spec };
        let m = RE_SPEC
            .captures(spec)
            .or_else(|| RE_SPEC.captures(","))
            .expect("`,` is a specifier");
        let group = |i: usize| m.get(i).map(|g| g.as_str());
        Self {
            plus: group(1) == Some("+"),
            currency: group(2).is_some(),
            comma: group(3).is_some(),
            trim: group(5).is_some_and(|t| !t.is_empty()),
            precision: group(4).map(|p| Number::from(p).0.min(20.0) as usize),
            notation: match group(6).unwrap_or("") {
                "f" => Notation::Fixed,
                "%" => Notation::Percent,
                "d" => Notation::Integer,
                "e" => Notation::Exponent,
                _ => Notation::Significant,
            },
        }
    }

    pub fn format(&self, value: f64) -> String {
        let abs = value.abs();
        let mut suffix = "";
        let mut body = match self.notation {
            Notation::Fixed => Number(abs).to_fixed(self.precision.unwrap_or(6)),
            Notation::Percent => {
                suffix = "%";
                Number(abs * 100.0).to_fixed(self.precision.unwrap_or(6))
            }
            Notation::Integer => Number(abs.round()).to_fixed(0),
            Notation::Exponent => Number(abs).to_exponential(self.precision.unwrap_or(6)),
            Notation::Significant => Number(abs)
                .round_significant(self.precision.unwrap_or(12).max(1))
                .to_string(),
        };

        if self.trim && body.contains('.') && !body.contains('e') {
            body = RE_ZEROS.replace(&body, "").into_owned();
        }
        if self.comma && !body.contains('e') {
            body = match body.find('.') {
                None => group(&body),
                Some(dot) => group(&body[..dot]) + &body[dot..],
            };
        }
        let sign = if value < 0.0 {
            "-"
        } else if self.plus {
            "+"
        } else {
            ""
        };
        format!(
            "{sign}{}{body}{suffix}",
            if self.currency { "$" } else { "" }
        )
    }
}
