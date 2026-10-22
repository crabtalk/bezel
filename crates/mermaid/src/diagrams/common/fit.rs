//! Labels laid out as journey and timeline draw them, and cut to fit a box.
//! Pele's `diagrams/common/fit.ts`.

use crate::{
    string,
    text::{Host, Label},
    util::chars,
};
use regex::Regex;
use std::sync::LazyLock;

/// A line break, an encoded entity, or one character: the pieces a word can
/// be cut between.
static RE_UNIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i-u:<br)[{}]*/?>|ﬂ°°?[0-9A-Z_a-z]+¶ß|&#?[0-9A-Z_a-z]+;|(?s:.)",
        chars::SPACE
    ))
    .expect("pattern")
});

/// `text` with each `<` that does not open a `<br>` as `&lt;`:
/// `text.replace(/<(?!br\s*\/?>)/gi, "&lt;")`.
fn escape_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (at, _) in text.match_indices('<') {
        let rest = &text[at + 1..];
        let after_br = rest
            .get(..2)
            .filter(|br| br.eq_ignore_ascii_case("br"))
            .map(|_| string::trim_start(&rest[2..]));
        let is_break =
            after_br.is_some_and(|after| after.strip_prefix('/').unwrap_or(after).starts_with('>'));
        if !is_break {
            out.push_str(&text[last..at]);
            out.push_str("&lt;");
            last = at + 1;
        }
    }
    out + &text[last..]
}

impl Label {
    /// Text laid out as Mermaid's journey and timeline draw it: plain text in
    /// which only `<br>` is markup.
    pub fn plain(raw: Option<&str>, measure: &impl Host, size: f64, max: f64, base: u8) -> Label {
        let text = raw.map(|text| {
            let mut text = text.to_string();
            if text.contains('<') {
                text = escape_tags(&text);
            }
            if text.contains(":fa-") {
                text = text.replace(":fa-", "&colon;fa-");
            }
            text
        });
        Label::layout(text.as_deref(), false, measure, size, max, base)
    }

    /// [`Label::layout`] wraps at spaces only. For a box of fixed width, this
    /// also cuts any word too long to fit.
    pub fn fit(raw: Option<&str>, measure: &impl Host, size: f64, max: f64, base: u8) -> Label {
        let label = Label::plain(raw, measure, size, max, base);
        let Some(raw) = raw else {
            return label;
        };
        if label.width <= max {
            return label;
        }
        let mut out = String::new();
        let mut width = 0.0;
        for unit in RE_UNIT.find_iter(raw).map(|unit| unit.as_str()) {
            let units = string::len(unit);
            if string::trim(unit).is_empty() || (units > 2 && unit.starts_with('<')) {
                width = 0.0;
            } else {
                let w = measure.width(if units > 2 { "x" } else { unit }, size, base);
                if width > 0.0 && width + w > max {
                    out.push('\n');
                    width = 0.0;
                }
                width += w;
            }
            out.push_str(unit);
        }
        Label::plain(Some(&out), measure, size, max, base)
    }
}
