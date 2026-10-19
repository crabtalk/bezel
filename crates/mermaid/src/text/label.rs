//! A label's text broken into lines of styled spans and measured. Pele's
//! `text/label.ts`.

use super::{
    entities::decode_entities,
    icons::icon_markup,
    measure::{Host, Style},
};
use crate::js::{self, Lazy};

pub const LINE_HEIGHT: f64 = 1.5;

/// Chinese and Japanese break between any two characters; Hangul is left out,
/// as Korean puts spaces between words.
const CJK: &str = r"⺀-鿿豈-﫿＀-｠";
const CLOSE: &str = r"、。，．：；！？）］｝」』】〕〉》〙〗ー…‥・ゝゞぁぃぅぇぉっゃゅょゎァィゥェォッャュョヮ,.:;!?)\]}";
const OPEN: &str = "（［｛「『【〔〈《〘〖([{";

static RE_BREAK: Lazy = Lazy::new(r"<br\s*\/?>|\\n|\n", "gi");
static RE_TAG: Lazy = Lazy::new(r"<\/?([a-zA-Z][\w-]*)(?:\s[^<>]*)?\/?>", "y");
static RE_ICON: Lazy = Lazy::new(r"(fa[bklrs]?):fa-([\w-]+)", "g");
static RE_MARKUP: Lazy = Lazy::new(r"[<\\\n*_]|fa[bklrs]?:fa-", "");
static RE_WORD: Lazy = Lazy::new(r"\w", "");
static RE_CJK: std::sync::LazyLock<crate::js::Regex> =
    std::sync::LazyLock::new(|| crate::js::Regex::new(&format!("[{CJK}]"), ""));
static RE_PIECE: std::sync::LazyLock<crate::js::Regex> = std::sync::LazyLock::new(|| {
    crate::js::Regex::new(
        &format!(r"\s*(?:[{OPEN}]*[{CJK}][{CLOSE}]*|[^\s{CJK}]+[{CLOSE}]*)\s*|\s+"),
        "g",
    )
});

#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: u8,
    pub icon: Option<Icon>,
    pub width: f64,
}

/// An icon in a label: its name as written, and the host's markup for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Icon {
    pub name: String,
    pub markup: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Label {
    pub lines: Vec<Vec<Span>>,
    pub widths: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub size: f64,
    pub line_height: f64,
}

fn dedent(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut min = usize::MAX;
    for line in &lines {
        if js::trim(line).is_empty() {
            continue;
        }
        let indent = js::len(line) - js::len(js::trim_start(line));
        min = min.min(indent);
    }
    if min == usize::MAX || min == 0 {
        return text.to_string();
    }
    lines
        .iter()
        .map(|line| {
            let own = js::len(line) - js::len(js::trim_start(line));
            let cut = min.min(own);
            // The cut is all whitespace, one unit per character it takes.
            let at = line
                .char_indices()
                .nth(cut)
                .map_or(line.len(), |(at, _)| at);
            &line[at..]
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn push_text(line: &mut Vec<Span>, text: &str, style: u8) {
    if text.is_empty() {
        return;
    }
    let decoded = decode_entities(text);
    if let Some(last) = line.last_mut()
        && last.style == style
        && last.icon.is_none()
    {
        last.text.push_str(&decoded);
        return;
    }
    line.push(Span {
        text: decoded,
        style,
        icon: None,
        width: 0.0,
    });
}

fn push_icons(line: &mut Vec<Span>, text: &str, style: u8, host: &impl Host) {
    if !text.contains(":fa-") {
        push_text(line, text, style);
        return;
    }
    let mut last = 0;
    for caps in RE_ICON.captures_iter(text) {
        let icon = format!("{}:fa-{}", caps.str(1), caps.str(2));
        push_text(line, &text[last..caps.start()], style);
        last = caps.end();
        let markup = icon_markup(host, &icon);
        if !markup.is_empty() {
            line.push(Span {
                text: String::new(),
                style,
                icon: Some(Icon { name: icon, markup }),
                width: 0.0,
            });
        } else if text[last..].starts_with(' ') {
            // An icon the host does not have leaves no gap, nor the space
            // that set it off from the text.
            last += 1;
        } else if last == text.len()
            && let Some(before) = line.last_mut()
            && before.icon.is_none()
        {
            before.text = js::trim_end(&before.text).to_string();
        }
    }
    push_text(line, &text[last..], style);
}

fn is_blank(c: Option<char>) -> bool {
    c.is_none_or(|c| js::trim(c.encode_utf8(&mut [0; 4])).is_empty())
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| RE_WORD.test(c.encode_utf8(&mut [0; 4])))
}

/// Splits inline HTML and, for markdown labels, emphasis markers into styled
/// spans.
fn inline(line: &mut Vec<Span>, source: &str, markdown: bool, base: u8, host: &impl Host) {
    let mut style = base;
    let mut marked = 0u8;
    let mut text = String::new();
    let mut i = 0;
    let n = source.len();
    let char_at = |at: usize| source.get(at..).and_then(|rest| rest.chars().next());
    while i < n {
        let ch = char_at(i).unwrap_or(' ');
        if ch == '<' {
            if let Some(m) = RE_TAG.match_here(source, i) {
                let tag = m.str(1).to_lowercase();
                let closing = source[i + 1..].starts_with('/');
                let bit = match tag.as_str() {
                    "b" | "strong" => Style::BOLD,
                    "i" | "em" => Style::ITALIC,
                    _ => 0,
                };
                if bit != 0 {
                    push_icons(line, &text, style, host);
                    text.clear();
                    style = if closing {
                        (style & !bit) | (base & bit)
                    } else {
                        style | bit
                    };
                }
                i = m.end();
                continue;
            }
        } else if markdown && (ch == '*' || ch == '_') {
            let double = char_at(i + 1) == Some(ch);
            let bit = if double { Style::BOLD } else { Style::ITALIC };
            let len = if double { 2 } else { 1 };
            let open = marked & bit == 0;
            let prev = if i > 0 {
                source[..i].chars().next_back()
            } else {
                Some(' ')
            };
            let next = if i + len < n {
                char_at(i + len)
            } else {
                Some(' ')
            };
            let marker = if double {
                format!("{ch}{ch}")
            } else {
                ch.to_string()
            };
            let can_open = open && !is_blank(next) && source[i + len..].contains(&marker);
            let can_close = !open && !is_blank(prev);
            let wordy = ch == '_' && is_word(prev) && is_word(next);
            if (can_open || can_close) && !wordy {
                push_icons(line, &text, style, host);
                text.clear();
                marked = if can_open {
                    marked | bit
                } else {
                    marked & !bit
                };
                style = if can_open {
                    style | bit
                } else {
                    (style & !bit) | (base & bit)
                };
                i += len;
                continue;
            }
        }
        text.push(ch);
        i += ch.len_utf8();
    }
    push_icons(line, &text, style, host);
}

fn measure_spans(line: &mut [Span], measure: &impl Host, size: f64) -> f64 {
    let mut total = 0.0;
    for span in line {
        span.width = if span.icon.is_some() {
            size * 1.25
        } else {
            measure.width(&span.text, size, span.style)
        };
        total += span.width;
    }
    total
}

fn wrap(
    line: &[Span],
    measure: &impl Host,
    size: f64,
    max: f64,
    out: &mut Vec<Vec<Span>>,
    widths: &mut Vec<f64>,
) {
    let mut current: Vec<Span> = Vec::new();
    let mut width = 0.0;
    let commit = |current: &mut Vec<Span>,
                  width: &mut f64,
                  out: &mut Vec<Vec<Span>>,
                  widths: &mut Vec<f64>| {
        if let Some(last) = current.last_mut()
            && last.icon.is_none()
        {
            let trimmed = js::trim_end(&last.text).to_string();
            if trimmed != last.text {
                let w = measure.width(&trimmed, size, last.style);
                *width += w - last.width;
                last.text = trimmed;
                last.width = w;
            }
        }
        out.push(std::mem::take(current));
        widths.push(*width);
        *width = 0.0;
    };
    for span in line {
        if span.icon.is_some() {
            let w = size * 1.25;
            if width > 0.0 && width + w > max {
                commit(&mut current, &mut width, out, widths);
            }
            current.push(Span {
                width: w,
                ..span.clone()
            });
            width += w;
            continue;
        }
        for piece in RE_PIECE.find_all(&span.text) {
            let mut part = piece;
            if width > 0.0 && width + measure.width(js::trim_end(part), size, span.style) > max {
                commit(&mut current, &mut width, out, widths);
                part = js::trim_start(part);
            }
            if let Some(last) = current.last_mut()
                && last.style == span.style
                && last.icon.is_none()
            {
                last.text.push_str(part);
                last.width = measure.width(&last.text, size, last.style);
                width = current.iter().map(|s| s.width).sum();
            } else {
                let w = measure.width(part, size, span.style);
                current.push(Span {
                    text: part.to_string(),
                    style: span.style,
                    icon: None,
                    width: w,
                });
                width += w;
            }
        }
    }
    if !current.is_empty() || out.is_empty() {
        commit(&mut current, &mut width, out, widths);
    }
}

/// `raw` laid out at `size` px, wrapped at `max_width`: inline `<b>`/`<i>`,
/// `<br>` and, for a `markdown` label, `**` and `_` emphasis. `base` is the
/// [`Style`] the whole label starts in.
pub fn layout_label(
    raw: Option<&str>,
    markdown: bool,
    measure: &impl Host,
    size: f64,
    max_width: f64,
    base: u8,
) -> Label {
    let Some(raw) = raw.filter(|raw| !raw.is_empty()) else {
        return Label::default();
    };
    let line_height = js::round(size * LINE_HEIGHT);

    if !RE_MARKUP.test(raw) && !raw.contains('&') && !raw.contains('ﬂ') {
        let text = js::trim(raw);
        let width = measure.width(text, size, base);
        if width <= max_width || !(text.contains(' ') || RE_CJK.test(text)) {
            return Label {
                lines: vec![vec![Span {
                    text: text.to_string(),
                    style: base,
                    icon: None,
                    width,
                }]],
                widths: vec![width],
                width,
                height: line_height,
                size,
                line_height,
            };
        }
        let mut lines = Vec::new();
        let mut widths = Vec::new();
        wrap(
            &[Span {
                text: text.to_string(),
                style: base,
                icon: None,
                width,
            }],
            measure,
            size,
            max_width,
            &mut lines,
            &mut widths,
        );
        let widest = widths.iter().copied().fold(0.0, f64::max);
        let height = lines.len() as f64 * line_height;
        return Label {
            lines,
            widths,
            width: widest,
            height,
            size,
            line_height,
        };
    }

    let source = if markdown {
        js::trim(&dedent(&raw.replace("\r\n", "\n").replace('\r', "\n"))).to_string()
    } else {
        js::trim(raw).to_string()
    };
    let mut lines = Vec::new();
    let mut widths = Vec::new();
    for part in RE_BREAK.split_str(&source) {
        let mut line = Vec::new();
        inline(&mut line, js::trim(part), markdown, base, measure);
        let w = measure_spans(&mut line, measure, size);
        if w > max_width {
            wrap(&line, measure, size, max_width, &mut lines, &mut widths);
        } else {
            lines.push(line);
            widths.push(w);
        }
    }
    let width = widths
        .iter()
        .copied()
        .fold(0.0, |a, b| if b > a { b } else { a });
    let height = lines.len() as f64 * line_height;
    Label {
        lines,
        widths,
        width,
        height,
        size,
        line_height,
    }
}
