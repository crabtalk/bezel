//! Labels that keep every character, and labels broken between characters.
//! Pele's `diagrams/common/literal-label.ts`.

use crate::geom::Point;
use crate::{
    js,
    svg::builder::{Esc, fragment, num, span_style, text_style},
    text::{Host, Label, Span},
    util::chars::is_space,
};
use std::fmt;

impl Label {
    /// Literal labels, filenames and protocol fields among them, keep every
    /// character but the spaces a line breaks at. A long word breaks between
    /// code points.
    pub fn literal(text: &str, measure: &impl Host, size: f64, room: f64, style: u8) -> Label {
        let mut lines: Vec<String> = Vec::new();
        for raw in text.split('\n') {
            let chars: Vec<char> = raw.chars().collect();
            if chars.is_empty() {
                lines.push(String::new());
            }
            let mut from = 0;
            while from < chars.len() {
                let fits = |length: usize| -> bool {
                    let piece: String = chars[from..(from + length).min(chars.len())]
                        .iter()
                        .collect();
                    measure.width(&piece, size, style) <= room
                };
                let mut hi = 1;
                while hi < chars.len() - from && fits(hi) {
                    hi = (chars.len() - from).min(hi * 2);
                }
                let mut lo = 1;
                while lo < hi {
                    let mid = (lo + hi).div_ceil(2);
                    if fits(mid) {
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                let mut take = lo;
                let wraps = from + take < chars.len();
                if wraps && !is_space(chars[from + take]) {
                    for i in (1..take).rev() {
                        if is_space(chars[from + i]) {
                            take = i + 1;
                            break;
                        }
                    }
                }
                let line: String = chars[from..from + take].iter().collect();
                lines.push(if wraps {
                    let trimmed = js::trim_end(&line);
                    if trimmed.is_empty() {
                        line.clone()
                    } else {
                        trimmed.to_string()
                    }
                } else {
                    line
                });
                from += take;
                while wraps && from < chars.len() && is_space(chars[from]) {
                    from += 1;
                }
            }
        }
        if text.is_empty() {
            lines.clear();
        }
        let widths: Vec<f64> = lines
            .iter()
            .map(|line| measure.width(line, size, style))
            .collect();
        let width = widths.iter().copied().fold(0.0, f64::max);
        let line_height = js::round(size * 1.5);
        let height = lines.len() as f64 * line_height;
        Label {
            lines: lines
                .into_iter()
                .zip(&widths)
                .map(|(text, &width)| {
                    vec![Span {
                        text,
                        style,
                        icon: None,
                        width,
                    }]
                })
                .collect(),
            widths,
            width,
            height,
            size,
            line_height,
        }
    }

    /// This label with every line wider than `room` broken between characters.
    pub fn broken(&self, measure: &impl Host, room: f64) -> Label {
        if self.width <= room {
            return self.clone();
        }
        let mut lines: Vec<Vec<Span>> = Vec::new();
        let mut widths: Vec<f64> = Vec::new();
        for (i, source) in self.lines.iter().enumerate() {
            if self.widths[i] <= room {
                lines.push(source.clone());
                widths.push(self.widths[i]);
                continue;
            }
            let mut current: Vec<Span> = Vec::new();
            let mut width = 0.0;
            for span in source {
                if span.icon.is_some() {
                    if !current.is_empty() && width + span.width > room {
                        lines.push(std::mem::take(&mut current));
                        widths.push(width);
                        width = 0.0;
                    }
                    current.push(span.clone());
                    width += span.width;
                    continue;
                }
                for ch in span.text.chars() {
                    let joined = current
                        .last()
                        .is_some_and(|last| last.icon.is_none() && last.style == span.style);
                    let next_width = if joined {
                        let last = current.last().map_or("", |last| last.text.as_str());
                        measure.width(&format!("{last}{ch}"), self.size, span.style)
                    } else {
                        measure.width(&ch.to_string(), self.size, span.style)
                    };
                    let mut added = next_width
                        - if joined {
                            current.last().map_or(0.0, |last| last.width)
                        } else {
                            0.0
                        };
                    let mut can_join = joined;
                    if !current.is_empty() && width + added > room {
                        lines.push(std::mem::take(&mut current));
                        widths.push(width);
                        width = 0.0;
                        can_join = false;
                        added = measure.width(&ch.to_string(), self.size, span.style);
                    }
                    match current.last_mut() {
                        Some(last)
                            if can_join && last.icon.is_none() && last.style == span.style =>
                        {
                            last.text.push(ch);
                            last.width += added;
                        }
                        _ => current.push(Span {
                            text: ch.to_string(),
                            style: span.style,
                            icon: None,
                            width: added,
                        }),
                    }
                    width += added;
                }
            }
            if !current.is_empty() || source.is_empty() {
                lines.push(current);
                widths.push(width);
            }
        }
        let width = widths.iter().copied().fold(0.0, f64::max);
        let height = lines.len() as f64 * self.line_height;
        Label {
            lines,
            widths,
            width,
            height,
            ..self.clone()
        }
    }

    /// This label with every line starting at `at.x`, the first line's top at
    /// `at.y`. A line is anchored at its start and keeps its spaces, so where
    /// it begins does not depend on how its text was measured.
    pub fn start_lines<'a>(&'a self, at: Point, attrs: &'a str) -> StartLines<'a> {
        StartLines {
            label: self,
            at,
            attrs,
        }
    }
}

/// [`Label::start_lines`].
pub struct StartLines<'a> {
    label: &'a Label,
    at: Point,
    attrs: &'a str,
}

impl fmt::Display for StartLines<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let StartLines { label, at, attrs } = *self;
        let (x, top) = (at.x, at.y);
        let shared = label.shared_style();
        let open = fragment(|f| write!(f, "<text{}", text_style(attrs, shared)));
        for (i, spans) in label.lines.iter().enumerate() {
            let cy = top + (i as f64 + 0.5) * label.line_height;
            if spans.iter().any(|span| span.icon.is_some()) {
                let width = label.widths[i];
                write!(
                    f,
                    "{}",
                    label.line_svg(i, Point::new(x + width / 2.0, cy), attrs)
                )?;
                continue;
            }
            let shown =
                |span: &Span| !span.text.is_empty() || !span_style(span.style & !shared).is_empty();
            if !spans.iter().any(shown) {
                continue;
            }
            write!(
                f,
                "{open} x=\"{}\" y=\"{}\" text-anchor=\"start\" xml:space=\"preserve\">",
                num(x),
                num(cy + label.size * 0.35)
            )?;
            for span in spans {
                let own = span_style(span.style & !shared);
                if own.is_empty() {
                    write!(f, "{}", Esc(&span.text))?;
                } else {
                    write!(f, "<tspan{own}>{}</tspan>", Esc(&span.text))?;
                }
            }
            f.write_str("</text>")?;
        }
        Ok(())
    }
}
