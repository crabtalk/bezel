//! Labels laid out as journey and timeline draw them, and cut to fit a box.
//! Pele's `diagrams/common/fit.ts`.

use crate::{
    js::{self, Lazy},
    text::{Host, Label},
};

static RE_TAG_START: Lazy = Lazy::new(r"<(?!br\s*\/?>)", "gi");
static RE_ICON: Lazy = Lazy::new(r":fa-", "g");
/// A line break, an encoded entity, a surrogate pair, or one character: the
/// pieces a word can be cut between.
static RE_UNIT: Lazy = Lazy::new(r"<br\s*\/?>|ﬂ°°?\w+¶ß|&#?\w+;|[\s\S]", "gi");

impl Label {
    /// Text laid out as Mermaid's journey and timeline draw it: plain text in
    /// which only `<br>` is markup.
    pub fn plain(raw: Option<&str>, measure: &impl Host, size: f64, max: f64, base: u8) -> Label {
        let text = raw.map(|text| {
            let mut text = text.to_string();
            if text.contains('<') {
                text = RE_TAG_START.replace(&text, |_| "&lt;".into());
            }
            if text.contains(":fa-") {
                text = RE_ICON.replace(&text, |_| "&colon;fa-".into());
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
        for unit in RE_UNIT.find_all(raw) {
            let units = js::len(unit);
            if js::trim(unit).is_empty() || (units > 2 && unit.starts_with('<')) {
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
