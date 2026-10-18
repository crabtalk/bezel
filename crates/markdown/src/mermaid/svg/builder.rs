//! Writing SVG: escaping, coordinates, and a measured label as `<text>`.
//! Pele's `svg/builder.ts`.

use std::fmt::Write as _;

use crate::mermaid::{
    js,
    text::{Label, Style, decode_entities},
};

const BOLD: &str = " font-weight=\"var(--_w)\"";
const ITALIC: &str = " font-style=\"italic\"";

fn needs_escape(c: char) -> bool {
    matches!(c, '&' | '<' | '>' | '"' | '\u{0}'..='\u{8}' | '\u{b}' | '\u{c}' | '\u{e}'..='\u{1f}' | '\u{7f}')
}

/// Escapes text for XML and drops the control characters XML does not
/// allow.
pub fn esc(text: &str) -> String {
    if !text.chars().any(needs_escape) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if needs_escape(c) => {}
            c => out.push(c),
        }
    }
    out
}

/// Escapes text that is only ever read as text — a tooltip, an accessible
/// name, a `data-id` — with entity codes turned into their characters
/// first. A value checked before it is written (a style, a colour, a URL)
/// goes through [`esc`] instead, so what was checked is what is written.
pub fn esc_text(text: &str) -> String {
    esc(&decode_entities(text))
}

/// A coordinate to two decimals, without trailing zeros.
pub fn num(value: f64) -> String {
    let mut out = String::new();
    push_num(&mut out, value);
    out
}

/// [`num`] appended to `out`.
pub fn push_num(out: &mut String, value: f64) {
    let n = js::round(value * 100.0);
    if n == 0.0 {
        out.push('0');
        return;
    }
    let (negative, m) = if n > 0.0 { (false, n) } else { (true, -n) };
    if m < 1e11 {
        let q = (m / 100.0).floor();
        let r = (m - q * 100.0) as u32;
        if negative {
            out.push('-');
        }
        let _ = write!(out, "{}", q as u64);
        if r != 0 {
            if r.is_multiple_of(10) {
                let _ = write!(out, ".{}", r / 10);
            } else if r < 10 {
                let _ = write!(out, ".0{r}");
            } else {
                let _ = write!(out, ".{r}");
            }
        }
        return;
    }
    out.push_str(&js::num_str(n / 100.0));
}

/// A measured label as SVG text centred on (`cx`, `cy`). `attrs` go on the
/// `<text>` as written.
pub fn label_svg(label: &Label, cx: f64, cy: f64, attrs: &str) -> String {
    let lines = &label.lines;
    if lines.is_empty() {
        return String::new();
    }
    let lh = label.line_height;
    let shift = label.size * 0.35;
    let mut y = cy - label.height / 2.0 + lh / 2.0 + shift;

    if lines.len() == 1
        && lines[0].len() == 1
        && lines[0][0].icon.is_none()
        && lines[0][0].style == 0
    {
        return format!(
            "<text{attrs} x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(cx),
            num(y),
            esc(&lines[0][0].text)
        );
    }

    let shared = shared_style(label);
    let attrs = text_style(attrs, shared);
    let mut body = String::new();
    let mut extra = String::new();
    for (i, line) in lines.iter().enumerate() {
        let has_icon = line.iter().any(|span| span.icon.is_some());
        if has_icon {
            let mut x = cx - label.widths[i] / 2.0;
            for span in line {
                if let Some(icon) = &span.icon {
                    let side = label.size * 1.1;
                    let inner = crate::mermaid::text::icon_markup(icon);
                    let _ = write!(
                        extra,
                        "<svg class=\"{p}-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{inner}</svg>",
                        esc_text(icon),
                        num(x),
                        num(y - shift - side / 2.0),
                        num(side),
                        num(side),
                        p = crate::mermaid::PREFIX,
                    );
                } else if !span.text.is_empty() {
                    let _ = write!(
                        body,
                        "<tspan x=\"{}\" y=\"{}\" text-anchor=\"start\"{}>{}</tspan>",
                        num(x),
                        num(y),
                        span_style(span.style & !shared),
                        esc(&span.text)
                    );
                }
                x += span.width;
            }
        } else {
            for (j, span) in line.iter().enumerate() {
                let pos = if j == 0 {
                    format!(" x=\"{}\" y=\"{}\"", num(cx), num(y))
                } else {
                    String::new()
                };
                let _ = write!(
                    body,
                    "<tspan{pos}{}>{}</tspan>",
                    span_style(span.style & !shared),
                    esc(&span.text)
                );
            }
            if line.is_empty() {
                let _ = write!(body, "<tspan x=\"{}\" y=\"{}\"> </tspan>", num(cx), num(y));
            }
        }
        y += lh;
    }
    format!("<text{attrs} text-anchor=\"middle\" xml:space=\"preserve\">{body}</text>{extra}")
}

pub fn span_style(style: u8) -> &'static str {
    match (style & Style::BOLD != 0, style & Style::ITALIC != 0) {
        (true, true) => " font-weight=\"var(--_w)\" font-style=\"italic\"",
        (true, false) => BOLD,
        (false, true) => ITALIC,
        (false, false) => "",
    }
}

/// The emphasis every span of a label has. It is written once, on the text
/// element, where a CSS rule for the label reaches it.
pub fn shared_style(label: &Label) -> u8 {
    let mut shared = Style::BOLD | Style::ITALIC;
    let mut any = false;
    for line in &label.lines {
        for span in line {
            if span.icon.is_some() || span.text.is_empty() {
                continue;
            }
            shared &= span.style;
            any = true;
        }
    }
    if any { shared } else { 0 }
}

/// Adds that emphasis to a text element's attributes, unless they give a
/// weight of their own.
pub fn text_style(attrs: &str, shared: u8) -> String {
    let mut out = attrs.to_string();
    if shared & Style::BOLD != 0 && !attrs.contains(" font-weight=\"") {
        out.push_str(BOLD);
    }
    if shared & Style::ITALIC != 0 && !attrs.contains(" font-style=\"") {
        out.push_str(ITALIC);
    }
    out
}
