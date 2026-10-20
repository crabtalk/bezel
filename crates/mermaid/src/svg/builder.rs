//! Writing SVG: escaping, coordinates, and a measured label as `<text>`.
//! Pele's `svg/builder.ts`.

use std::{
    borrow::Cow,
    fmt::{self, Write},
    ops::Range,
};

use crate::{
    geom::{Point, Size},
    js,
    text::{Label, Span, Style, decoded},
};

/// The attributes of a diagram's title.
pub const TITLE: &str = crate::class!("title", "font-weight=\"var(--_tw)\"");

const BOLD: &str = " font-weight=\"var(--_w)\"";
const ITALIC: &str = " font-style=\"italic\"";

fn needs_escape(c: char) -> bool {
    c.is_ascii() && needs_escape_byte(c as u8)
}

fn needs_escape_byte(b: u8) -> bool {
    (b < 0x20 && b != b'\t' && b != b'\n' && b != b'\r')
        | (b == b'&')
        | (b == b'<')
        | (b == b'>')
        | (b == b'"')
        | (b == 0x7f)
}

/// Escapes text for XML and drops the control characters XML does not
/// allow.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    push_esc(&mut out, text);
    out
}

/// [`esc`] appended to `out`.
pub fn push_esc(out: &mut String, text: &str) {
    let _ = write_esc(out, text);
}

fn write_esc(out: &mut impl Write, text: &str) -> fmt::Result {
    // Without an early exit, so the check runs over many bytes at a time.
    if !text
        .bytes()
        .fold(false, |found, b| found | needs_escape_byte(b))
    {
        return out.write_str(text);
    }
    let mut last = 0;
    for (at, c) in text.char_indices() {
        let entity = match c {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            c if needs_escape(c) => "",
            _ => continue,
        };
        out.write_str(&text[last..at])?;
        out.write_str(entity)?;
        last = at + c.len_utf8();
    }
    out.write_str(&text[last..])
}

/// [`esc`] as a value `write!` writes in place.
#[derive(Clone, Copy)]
pub struct Esc<'a>(pub &'a str);

impl fmt::Display for Esc<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_esc(f, self.0)
    }
}

/// Escapes text that is only ever read as text — a tooltip, an accessible
/// name, a `data-id` — with entity codes turned into their characters
/// first. A value checked before it is written (a style, a colour, a URL)
/// goes through [`Esc`] instead, so what was checked is what is written.
#[derive(Clone, Copy)]
pub struct EscText<'a>(pub &'a str);

impl fmt::Display for EscText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_esc(f, &decoded(self.0))
    }
}

/// A coordinate to two decimals, without trailing zeros.
pub fn num(value: f64) -> Num {
    Num(value)
}

/// [`num`] appended to `out`.
pub fn push_num(out: &mut String, value: f64) {
    let mut buf = [0u8; 16];
    match digits(value, &mut buf) {
        Some(text) => out.push_str(text),
        None => out.push_str(&js::num_str(js::round(value * 100.0) / 100.0)),
    }
}

/// `value` to two decimals in `buf`, or `None` past what `buf` holds.
fn digits(value: f64, buf: &mut [u8; 16]) -> Option<&str> {
    let n = js::round(value * 100.0);
    if n == 0.0 {
        return Some("0");
    }
    let (negative, m) = if n > 0.0 { (false, n) } else { (true, -n) };
    if m >= 1e11 {
        return None;
    }
    // Integer digits, then up to two decimals, written from the right.
    let mut at = buf.len();
    let mut q = (m / 100.0).floor() as u64;
    let mut r = (m as u64 - q * 100) as u8;
    if r != 0 {
        if r.is_multiple_of(10) {
            r /= 10;
        } else {
            at -= 1;
            buf[at] = b'0' + r % 10;
            r /= 10;
        }
        at -= 1;
        buf[at] = b'0' + r;
        at -= 1;
        buf[at] = b'.';
    }
    loop {
        at -= 1;
        buf[at] = b'0' + (q % 10) as u8;
        q /= 10;
        if q == 0 {
            break;
        }
    }
    if negative {
        at -= 1;
        buf[at] = b'-';
    }
    // Only ASCII digits, a point and a sign were written.
    std::str::from_utf8(&buf[at..]).ok()
}

/// A fragment written when it is formatted, so `write!` puts it straight
/// into the document. Stands in for `std::fmt::from_fn` (Rust 1.93); the
/// workspace's `rust-version` is 1.90.
pub struct Fragment<F>(pub F);

impl<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result> fmt::Display for Fragment<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}

/// A [`Fragment`] of `write`.
pub fn fragment<F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result>(write: F) -> Fragment<F> {
    Fragment(write)
}

/// `buf` cleared and holding what `args` writes: a scratch buffer reused
/// across a loop.
pub fn rewrite<'b>(buf: &'b mut String, args: fmt::Arguments<'_>) -> &'b str {
    buf.clear();
    let _ = buf.write_fmt(args);
    buf
}

/// `attrs` and then `style`, an attribute that is often `""`.
pub fn styled<'a>(attrs: &'a str, style: &str) -> Cow<'a, str> {
    if style.is_empty() {
        return Cow::Borrowed(attrs);
    }
    let mut out = String::with_capacity(attrs.len() + style.len());
    out.push_str(attrs);
    out.push_str(style);
    Cow::Owned(out)
}

/// `x,y`, each as [`num`] writes it.
impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", Num(self.x), Num(self.y))
    }
}

/// A coordinate as [`num`] writes it.
#[derive(Clone, Copy, Debug)]
pub struct Num(f64);

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 16];
        match digits(self.0, &mut buf) {
            Some(text) => f.write_str(text),
            None => f.write_str(&js::num_str(js::round(self.0 * 100.0) / 100.0)),
        }
    }
}

impl Label {
    /// The label as SVG text centred on `center`. `attrs` go on the `<text>`
    /// as written.
    pub fn svg<'a>(&'a self, center: Point, attrs: &'a str) -> LabelSvg<'a> {
        LabelSvg {
            label: self,
            center,
            attrs,
        }
    }

    /// The label with each line starting at `at.x`, centred on `at.y` as a
    /// whole: each line is drawn as a one-line label of its own width.
    pub fn left_svg<'a>(&'a self, at: Point, attrs: &'a str) -> LeftLabel<'a> {
        LeftLabel {
            label: self,
            at,
            attrs,
        }
    }

    /// Line `index` of the label, as a one-line label of its own width
    /// centred on `center`.
    pub fn line_svg<'a>(&'a self, index: usize, center: Point, attrs: &'a str) -> LineSvg<'a> {
        LineSvg {
            label: self,
            index,
            center,
            attrs,
        }
    }

    /// The emphasis every span of the label has. It is written once, on the
    /// text element, where a CSS rule for the label reaches it.
    pub fn shared_style(&self) -> u8 {
        shared_of(&self.lines)
    }
}

/// [`Label::svg`], written in place by `write!`.
#[derive(Clone, Copy)]
pub struct LabelSvg<'a> {
    label: &'a Label,
    center: Point,
    attrs: &'a str,
}

impl fmt::Display for LabelSvg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_label(f, self.label, self.center, self.attrs)
    }
}

/// [`Label::left_svg`], written in place by `write!`.
#[derive(Clone, Copy)]
pub struct LeftLabel<'a> {
    label: &'a Label,
    at: Point,
    attrs: &'a str,
}

impl fmt::Display for LeftLabel<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let LeftLabel { label, at, attrs } = *self;
        if label.lines.len() < 2 {
            return write_label(f, label, Point::new(at.x + label.width / 2.0, at.y), attrs);
        }
        for i in 0..label.lines.len() {
            let line_cy = at.y - label.height / 2.0 + (i as f64 + 0.5) * label.line_height;
            let cx = at.x + label.widths[i] / 2.0;
            write!(f, "{}", label.line_svg(i, Point::new(cx, line_cy), attrs))?;
        }
        Ok(())
    }
}

/// [`Label::line_svg`], written in place by `write!`.
#[derive(Clone, Copy)]
pub struct LineSvg<'a> {
    label: &'a Label,
    index: usize,
    center: Point,
    attrs: &'a str,
}

impl fmt::Display for LineSvg<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let LineSvg {
            label,
            index,
            center,
            attrs,
        } = *self;
        write_lines(f, label, index..index + 1, label.line_height, center, attrs)
    }
}

/// A square of `side` with its top left corner at `at`, holding icon
/// `name`, drawn by `markup` on a 24-unit grid.
pub fn icon_element<'a>(
    name: &'a str,
    at: Point,
    side: f64,
    markup: impl fmt::Display + 'a,
) -> impl fmt::Display + 'a {
    fragment(move |f| {
        write!(
            f,
            "<svg class=\"{p}-icon\" data-icon=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{markup}</svg>",
            EscText(name),
            num(at.x),
            num(at.y),
            num(side),
            num(side),
            p = crate::PREFIX,
        )
    })
}

fn write_label(out: &mut impl Write, label: &Label, center: Point, attrs: &str) -> fmt::Result {
    write_lines(
        out,
        label,
        0..label.lines.len(),
        label.height,
        center,
        attrs,
    )
}

/// `label`'s lines in `range` as a label `height` tall, centred on
/// `center`.
fn write_lines(
    out: &mut impl Write,
    label: &Label,
    range: Range<usize>,
    height: f64,
    center: Point,
    attrs: &str,
) -> fmt::Result {
    let Point { x: cx, y: cy } = center;
    let widths = &label.widths[range.clone()];
    let lines = &label.lines[range];
    if lines.is_empty() {
        return Ok(());
    }
    let lh = label.line_height;
    let shift = label.size * 0.35;
    let top = cy - height / 2.0 + lh / 2.0 + shift;

    if lines.len() == 1
        && lines[0].len() == 1
        && lines[0][0].icon.is_none()
        && lines[0][0].style == 0
    {
        return write!(
            out,
            "<text{attrs} x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            num(cx),
            num(top),
            Esc(&lines[0][0].text)
        );
    }

    let shared = shared_of(lines);
    write!(
        out,
        "<text{}{} text-anchor=\"middle\" xml:space=\"preserve\">",
        attrs,
        TextStyle(attrs, shared)
    )?;
    let mut y = top;
    for (i, line) in lines.iter().enumerate() {
        if line.iter().any(|span| span.icon.is_some()) {
            let mut x = cx - widths[i] / 2.0;
            for span in line {
                if span.icon.is_none() && !span.text.is_empty() {
                    write!(
                        out,
                        "<tspan x=\"{}\" y=\"{}\" text-anchor=\"start\"{}>{}</tspan>",
                        num(x),
                        num(y),
                        span_style(span.style & !shared),
                        Esc(&span.text)
                    )?;
                }
                x += span.width;
            }
        } else {
            for (j, span) in line.iter().enumerate() {
                if j == 0 {
                    write!(out, "<tspan x=\"{}\" y=\"{}\"", num(cx), num(y))?;
                } else {
                    out.write_str("<tspan")?;
                }
                write!(
                    out,
                    "{}>{}</tspan>",
                    span_style(span.style & !shared),
                    Esc(&span.text)
                )?;
            }
            if line.is_empty() {
                write!(out, "<tspan x=\"{}\" y=\"{}\"> </tspan>", num(cx), num(y))?;
            }
        }
        y += lh;
    }
    out.write_str("</text>")?;
    // Icons are drawn after the text, in the order they appear.
    let mut y = top;
    for (i, line) in lines.iter().enumerate() {
        let mut x = cx - widths[i] / 2.0;
        for span in line {
            if let Some(icon) = &span.icon {
                let side = label.size * 1.1;
                write!(
                    out,
                    "{}",
                    icon_element(
                        &icon.name,
                        Point::new(x, y - shift - side / 2.0),
                        side,
                        &icon.markup
                    )
                )?;
            }
            x += span.width;
        }
        y += lh;
    }
    Ok(())
}

pub fn span_style(style: u8) -> &'static str {
    match (style & Style::BOLD != 0, style & Style::ITALIC != 0) {
        (true, true) => " font-weight=\"var(--_w)\" font-style=\"italic\"",
        (true, false) => BOLD,
        (false, true) => ITALIC,
        (false, false) => "",
    }
}

fn shared_of(lines: &[Vec<Span>]) -> u8 {
    let mut shared = Style::BOLD | Style::ITALIC;
    let mut any = false;
    for line in lines {
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

/// Adds `shared`, a [`Label::shared_style`], to a text element's attributes,
/// unless they give a weight of their own.
pub fn text_style(attrs: &str, shared: u8) -> impl fmt::Display + '_ {
    fragment(move |f| write!(f, "{attrs}{}", TextStyle(attrs, shared)))
}

/// The emphasis [`text_style`] adds after `attrs`.
struct TextStyle<'a>(&'a str, u8);

impl fmt::Display for TextStyle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TextStyle(attrs, shared) = *self;
        if shared & Style::BOLD != 0 && !attrs.contains(" font-weight=\"") {
            f.write_str(BOLD)?;
        }
        if shared & Style::ITALIC != 0 && !attrs.contains(" font-style=\"") {
            f.write_str(ITALIC)?;
        }
        Ok(())
    }
}

/// A `<rect>` with its top left corner at `corner`, then `attrs` as written.
pub fn rect(corner: Point, size: Size, attrs: impl fmt::Display) -> impl fmt::Display {
    fragment(move |f| {
        write!(
            f,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{attrs}/>",
            num(corner.x),
            num(corner.y),
            num(size.w),
            num(size.h)
        )
    })
}
