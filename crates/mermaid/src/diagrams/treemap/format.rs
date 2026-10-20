//! Pele's `diagrams/treemap/format.ts`.

use crate::js::{self, Lazy};

/// The parts of d3-format's specifier that treemap values use: an optional
/// `$`, `,` for thousands separators, a precision, and a type of `f`, `%`,
/// `d`, `e` or none. Fill, alignment and width are read and ignored. Anything
/// else falls back to the default, `,`.
static RE_SPEC: Lazy = Lazy::new(
    r"^(?:.?[<>=^])?([-+( ])?(\$)?0?\d*(,)?(?:\.(\d+))?(~)?([a-zA-Z%])?$",
    "",
);
static RE_ZEROS: Lazy = Lazy::new(r"\.?0+$", "");

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
            .exec(spec)
            .or_else(|| RE_SPEC.exec(","))
            .expect("`,` is a specifier");
        Self {
            plus: m.get(1) == Some("+"),
            currency: m.get(2).is_some(),
            comma: m.get(3).is_some(),
            trim: m.get(5).is_some_and(|t| !t.is_empty()),
            precision: m.get(4).map(|p| js::number(p).min(20.0) as usize),
            notation: match m.str(6) {
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
            Notation::Fixed => js::to_fixed(abs, self.precision.unwrap_or(6)),
            Notation::Percent => {
                suffix = "%";
                js::to_fixed(abs * 100.0, self.precision.unwrap_or(6))
            }
            Notation::Integer => js::to_fixed(abs.round(), 0),
            Notation::Exponent => js::to_exponential(abs, self.precision.unwrap_or(6)),
            Notation::Significant => js::num_str(js::to_precision_number(
                abs,
                self.precision.unwrap_or(12).max(1),
            )),
        };

        if self.trim && body.contains('.') && !body.contains('e') {
            body = RE_ZEROS.replace(&body, |_| String::new());
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
