//! [`Doc`] → HTML.
//!
//! The markup is semantic HTML; every element this crate has a word of its own
//! for carries a `md-` class, and the styling is the page's.
//!
//! A block stands where [`crate::serialize`] writes it: a blank paragraph is
//! not written, an indent is clamped the same way, and a blockquote is the
//! same run of blocks. A block nested under a list item goes inside its `<li>`;
//! one nested under anything else goes in a `<div class="md-indent">`.
//!
//! A bookmark and a mention are links with their form as a class. Their
//! previews come from the network in the app, and are not fetched here.
//!
//! A highlighted span of code is a `<span class="hl-…">` named by
//! [`theme::HighlightKind::name`], and [`Html::stylesheet`] colors them.

#[cfg(feature = "gpui")]
use crate::fence::highlight::Highlighter;
use crate::{
    doc::{Align, BlockKind, Doc, Form, Mark, Text},
    serialize::Layout,
};

#[cfg(feature = "gpui")]
mod style;

/// How a document is written as HTML: what its URLs become, and who renders a
/// fenced block.
#[derive(Default)]
pub struct Html {
    url: Option<UrlFn>,
    fence: Option<FenceFn>,
    #[cfg(feature = "gpui")]
    highlighter: Option<Highlighter>,
}

type UrlFn = Box<dyn Fn(&str) -> String>;
type FenceFn = Box<dyn Fn(&str, &str) -> Option<String>>;

pub fn html(doc: &Doc) -> String {
    Html::new().render(doc)
}

impl Html {
    pub fn new() -> Self {
        Self::default()
    }

    /// What every URL in the document — a link's, a mention's, a bookmark's, a
    /// picture's — is written as. Without one, each is written as it is.
    pub fn url(mut self, url: impl Fn(&str) -> String + 'static) -> Self {
        self.url = Some(Box::new(url));
        self
    }

    /// The markup for a fenced block, from its language and its code, or
    /// `None` to write it as `<pre><code>`. The markup is written unescaped.
    pub fn fence(mut self, fence: impl Fn(&str, &str) -> Option<String> + 'static) -> Self {
        self.fence = Some(Box::new(fence));
        self
    }

    /// What colors a fenced block [`Html::fence`] leaves as `<pre><code>`.
    #[cfg(feature = "gpui")]
    pub fn highlighter(mut self, highlighter: Highlighter) -> Self {
        self.highlighter = Some(highlighter);
        self
    }

    pub fn render(&self, doc: &Doc) -> String {
        let layout = Layout::of(&doc.blocks);
        let anchors = doc.anchors();
        let mut anchors = anchors.iter().peekable();
        let mut out = String::new();
        let mut open: Vec<Frame> = Vec::new();
        for (ix, (block, place)) in doc.blocks.iter().zip(&layout.at).enumerate() {
            let Some(place) = *place else { continue };
            let depth = place.indent;
            let list = List::of(&block.kind);

            if place.run.is_none_or(|run| run.first)
                && let Some(quote) = open.iter().rposition(|frame| frame.kind == Kind::Quote)
            {
                close(&mut out, &mut open, quote);
            }
            while let Some(top) = open.last() {
                let stays = match top.kind {
                    Kind::Quote => true,
                    Kind::Item | Kind::Indent => top.level < depth,
                    Kind::List(kind) => {
                        top.level < depth || (top.level == depth && list == Some(kind))
                    }
                };
                if stays {
                    break;
                }
                let last = open.len() - 1;
                close(&mut out, &mut open, last);
            }
            if let Some(run) = place.run
                && run.first
            {
                parent(&mut out, &mut open, depth);
                let alert = block.quote.and_then(|quote| quote.alert);
                match alert {
                    Some(alert) => {
                        let label = alert.label();
                        out.push_str(&format!(
                            "<blockquote class=\"md-alert md-alert-{}\"><p class=\"md-alert-title\">{label}</p>",
                            label.to_lowercase()
                        ));
                    }
                    None => out.push_str("<blockquote>"),
                }
                open.push(Frame {
                    kind: Kind::Quote,
                    level: depth,
                });
            }
            parent(&mut out, &mut open, depth);
            if let Some(kind) = list {
                if open
                    .last()
                    .is_none_or(|top| top.kind != Kind::List(kind) || top.level != depth)
                {
                    match (kind, &block.kind) {
                        (List::Ordered, BlockKind::Ordered { number, .. }) if *number != 1 => {
                            out.push_str(&format!("<ol start=\"{number}\">"))
                        }
                        (List::Ordered, _) => out.push_str("<ol>"),
                        (List::Bullet, _) => out.push_str("<ul>"),
                        (List::Task, _) => out.push_str("<ul class=\"md-tasks\">"),
                    }
                    open.push(Frame {
                        kind: Kind::List(kind),
                        level: depth,
                    });
                }
                open.push(Frame {
                    kind: Kind::Item,
                    level: depth,
                });
            }
            let anchor = anchors
                .next_if(|(_, block)| *block == ix)
                .map(|(anchor, _)| anchor.as_str());
            self.block(&mut out, &block.kind, anchor);
        }
        close(&mut out, &mut open, 0);
        out
    }

    fn block(&self, out: &mut String, kind: &BlockKind, anchor: Option<&str>) {
        match kind {
            BlockKind::Paragraph(text) => {
                out.push_str("<p>");
                self.inline(out, text);
                out.push_str("</p>");
            }
            BlockKind::Heading { level, text } => {
                let level = (*level).clamp(1, 6);
                out.push_str(&format!("<h{level}"));
                if let Some(anchor) = anchor {
                    out.push_str(" id=\"");
                    escape(out, anchor);
                    out.push('"');
                }
                out.push('>');
                self.inline(out, text);
                out.push_str(&format!("</h{level}>"));
            }
            BlockKind::Bullet(text) | BlockKind::Ordered { text, .. } => {
                out.push_str("<li>");
                self.inline(out, text);
            }
            BlockKind::Task { checked, text } => {
                out.push_str("<li class=\"md-task\"><input");
                if *checked {
                    out.push_str(" checked=\"\"");
                }
                out.push_str(" disabled=\"\" type=\"checkbox\"> ");
                self.inline(out, text);
            }
            BlockKind::Code {
                language,
                code,
                height,
            } => {
                let fenced = language.as_deref().and_then(|language| {
                    let fence = self.fence.as_ref()?(language, &code.text)?;
                    Some((language, fence))
                });
                match fenced {
                    Some((language, fence)) => {
                        out.push_str("<div class=\"md-fence\" data-language=\"");
                        escape(out, language);
                        out.push('"');
                        if let Some(height) = height {
                            out.push_str(&format!(" style=\"height:{height}px\""));
                        }
                        out.push('>');
                        out.push_str(&fence);
                        out.push_str("</div>");
                    }
                    None => {
                        out.push_str("<pre><code");
                        if let Some(language) = language {
                            out.push_str(" class=\"language-");
                            escape(out, language);
                            out.push('"');
                        }
                        out.push('>');
                        self.code(out, language.as_deref(), &code.text);
                        if !code.text.is_empty() {
                            out.push('\n');
                        }
                        out.push_str("</code></pre>");
                    }
                }
            }
            BlockKind::Image { url, alt, width } => {
                out.push_str("<figure class=\"md-image\"><img src=\"");
                self.write_url(out, url);
                out.push_str("\" alt=\"");
                escape(out, &alt.text);
                out.push('"');
                if let Some(width) = width {
                    out.push_str(&format!(" width=\"{width}\""));
                }
                out.push('>');
                if !alt.is_empty() {
                    out.push_str("<figcaption>");
                    escape(out, &alt.text);
                    out.push_str("</figcaption>");
                }
                out.push_str("</figure>");
            }
            BlockKind::Bookmark { url, form } => {
                let class = match form {
                    Form::Auto => "md-bookmark",
                    Form::Chip => "md-bookmark md-chip",
                    Form::Embed(_) => "md-bookmark md-embed",
                };
                out.push_str(&format!("<p><a class=\"{class}\" href=\""));
                self.write_url(out, url);
                out.push('"');
                if let Form::Embed(Some(height)) = form {
                    out.push_str(&format!(" style=\"height:{height}px\""));
                }
                out.push('>');
                escape(out, url);
                out.push_str("</a></p>");
            }
            BlockKind::Table {
                align,
                header,
                rows,
            } => {
                let columns = align.len().max(header.len());
                let row = |out: &mut String, cells: &[Text], tag: &str| {
                    out.push_str("<tr>");
                    for column in 0..columns {
                        out.push_str(&format!("<{tag}"));
                        match align.get(column).copied().unwrap_or_default() {
                            Align::Left => {}
                            Align::Center => out.push_str(" style=\"text-align:center\""),
                            Align::Right => out.push_str(" style=\"text-align:right\""),
                        }
                        out.push('>');
                        if let Some(cell) = cells.get(column) {
                            self.inline(out, cell);
                        }
                        out.push_str(&format!("</{tag}>"));
                    }
                    out.push_str("</tr>");
                };
                out.push_str("<table>");
                if !header.is_empty() {
                    out.push_str("<thead>");
                    row(out, header, "th");
                    out.push_str("</thead>");
                }
                if !rows.is_empty() {
                    out.push_str("<tbody>");
                    for cells in rows {
                        row(out, cells, "td");
                    }
                    out.push_str("</tbody>");
                }
                out.push_str("</table>");
            }
            BlockKind::Rule => out.push_str("<hr>"),
        }
    }

    /// `code`, its highlighted spans wrapped. A span overlapping the one
    /// before it is dropped.
    fn code(&self, out: &mut String, language: Option<&str>, code: &str) {
        #[cfg(feature = "gpui")]
        if let Some(spans) = self
            .highlighter
            .zip(language)
            .and_then(|(highlighter, language)| highlighter(language, code))
        {
            let mut cursor = 0;
            for (range, kind) in spans {
                let (Some(before), Some(inside)) =
                    (code.get(cursor..range.start), code.get(range.clone()))
                else {
                    continue;
                };
                escape(out, before);
                out.push_str("<span class=\"hl-");
                out.push_str(kind.name());
                out.push_str("\">");
                escape(out, inside);
                out.push_str("</span>");
                cursor = range.end;
            }
            escape(out, &code[cursor..]);
            return;
        }
        #[cfg(not(feature = "gpui"))]
        let _ = language;
        escape(out, code);
    }

    /// Marks open outermost first. Where two overlap without nesting, the
    /// inner one closes at the outer one's edge and opens again past it.
    /// A newline is a `<br>`.
    fn inline(&self, out: &mut String, text: &Text) {
        let mut boundaries: Vec<usize> = text
            .marks
            .iter()
            .flat_map(|span| [span.range.start, span.range.end])
            .chain([0, text.text.len()])
            .filter(|&at| at <= text.text.len())
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();

        let mut open: Vec<usize> = Vec::new();
        for pair in boundaries.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let active: Vec<usize> = (0..text.marks.len())
                .filter(|&ix| {
                    let range = &text.marks[ix].range;
                    range.start <= start && end <= range.end
                })
                .collect();
            let kept = open
                .iter()
                .zip(&active)
                .take_while(|(open, active)| open == active)
                .count();
            while open.len() > kept {
                let ix = open.pop().expect("a mark past the kept ones");
                close_mark(out, &text.marks[ix].mark);
            }
            for &ix in &active[kept..] {
                self.open_mark(out, &text.marks[ix].mark);
                open.push(ix);
            }
            // A picture's text is its alt, written at its start and nowhere
            // else.
            let mut inside_image = false;
            for span in &text.marks {
                let Mark::Image(url) = &span.mark else {
                    continue;
                };
                if span.range.start == start {
                    self.image(out, url, &text.text[span.range.clone()]);
                }
                inside_image |= span.range.start <= start && end <= span.range.end;
            }
            if inside_image {
                continue;
            }
            for (line, part) in text.text[start..end].split('\n').enumerate() {
                if line > 0 {
                    out.push_str("<br>");
                }
                escape(out, part);
            }
        }
        while let Some(ix) = open.pop() {
            close_mark(out, &text.marks[ix].mark);
        }
        // A picture over nothing has no segment to stand before.
        for span in &text.marks {
            if let Mark::Image(url) = &span.mark
                && span.range.is_empty()
                && span.range.start == text.text.len()
            {
                self.image(out, url, "");
            }
        }
    }

    fn image(&self, out: &mut String, url: &str, alt: &str) {
        out.push_str("<img src=\"");
        self.write_url(out, url);
        out.push_str("\" alt=\"");
        escape(out, alt);
        out.push_str("\">");
    }

    fn open_mark(&self, out: &mut String, mark: &Mark) {
        match mark {
            Mark::Bold => out.push_str("<strong>"),
            Mark::Italic => out.push_str("<em>"),
            Mark::Strike => out.push_str("<del>"),
            Mark::Code => out.push_str("<code>"),
            Mark::Link(url) => {
                out.push_str("<a href=\"");
                self.write_url(out, url);
                out.push_str("\">");
            }
            Mark::Mention { url, form } => {
                let class = match form {
                    Form::Auto => "md-mention",
                    Form::Chip => "md-mention md-chip",
                    Form::Embed(_) => "md-mention md-embed",
                };
                out.push_str(&format!("<a class=\"{class}\" href=\""));
                self.write_url(out, url);
                out.push_str("\">");
            }
            Mark::Custom(name) => {
                out.push_str("<span class=\"md-mark\" data-mark=\"");
                escape(out, name);
                out.push_str("\">");
            }
            Mark::Image(_) => {}
        }
    }

    /// `url` resolved, then percent-encoded where an `href` cannot hold it as
    /// written. A `javascript:`, `vbscript:` or `file:` URL, or a `data:` URL
    /// that is not an image, is written empty.
    fn write_url(&self, out: &mut String, url: &str) {
        let resolved = match &self.url {
            Some(resolve) => resolve(url),
            None => url.to_owned(),
        };
        if unsafe_url(&resolved) {
            return;
        }
        let mut encoded = String::with_capacity(resolved.len());
        for c in resolved.chars() {
            if c.is_ascii_alphanumeric() || HREF_SAFE.contains(c) {
                encoded.push(c);
            } else {
                for byte in c.encode_utf8(&mut [0; 4]).bytes() {
                    encoded.push_str(&format!("%{byte:02X}"));
                }
            }
        }
        escape(out, &encoded);
    }
}

/// What an `href` holds as written besides ASCII letters and digits: cmark's
/// set, which leaves an existing `%XX` alone.
const HREF_SAFE: &str = "-_.+!*'(),%#@?=;:/&$~";

fn unsafe_url(url: &str) -> bool {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.trim().to_ascii_lowercase());
    match scheme.as_deref() {
        Some("javascript" | "vbscript" | "file") => true,
        Some("data") => ![
            "data:image/png",
            "data:image/gif",
            "data:image/jpeg",
            "data:image/webp",
        ]
        .iter()
        .any(|image| url.trim_start().to_ascii_lowercase().starts_with(image)),
        _ => false,
    }
}

fn close_mark(out: &mut String, mark: &Mark) {
    out.push_str(match mark {
        Mark::Bold => "</strong>",
        Mark::Italic => "</em>",
        Mark::Strike => "</del>",
        Mark::Code => "</code>",
        Mark::Link(_) | Mark::Mention { .. } => "</a>",
        Mark::Custom(_) => "</span>",
        Mark::Image(_) => "",
    });
}

/// An element left open while the blocks after it may still go inside it.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Frame {
    kind: Kind,
    level: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Quote,
    List(List),
    Item,
    Indent,
}

/// Which list a marker block belongs to: adjacent markers of two kinds are two
/// lists.
#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Bullet,
    Ordered,
    Task,
}

impl List {
    fn of(kind: &BlockKind) -> Option<Self> {
        match kind {
            BlockKind::Bullet(_) => Some(Self::Bullet),
            BlockKind::Ordered { .. } => Some(Self::Ordered),
            BlockKind::Task { .. } => Some(Self::Task),
            _ => None,
        }
    }
}

/// Close `open[from..]`, innermost first.
fn close(out: &mut String, open: &mut Vec<Frame>, from: usize) {
    while open.len() > from {
        let frame = open.pop().expect("a frame past `from`");
        out.push_str(match frame.kind {
            Kind::Quote => "</blockquote>",
            Kind::List(List::Ordered) => "</ol>",
            Kind::List(_) => "</ul>",
            Kind::Item => "</li>",
            Kind::Indent => "</div>",
        });
    }
}

/// Open a `<div class="md-indent">` for a block at `depth` that has nothing
/// open one level up to stand in.
fn parent(out: &mut String, open: &mut Vec<Frame>, depth: u8) {
    let Some(up) = depth.checked_sub(1) else {
        return;
    };
    let held = open.last().is_some_and(|top| match top.kind {
        Kind::Item | Kind::Indent => top.level == up,
        Kind::Quote | Kind::List(_) => top.level == depth,
    });
    if !held {
        out.push_str("<div class=\"md-indent\">");
        open.push(Frame {
            kind: Kind::Indent,
            level: up,
        });
    }
}

fn escape(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}
