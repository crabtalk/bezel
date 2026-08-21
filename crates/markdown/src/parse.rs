//! Markdown → [`Doc`].
//!
//! CommonMark nests; a [`Doc`] does not. Indent counts **list nesting only**:
//!
//! - a list item's first paragraph becomes its marker block (bullet, ordered,
//!   task) at one level shallower than the open list count, and anything else
//!   in that item becomes a child at the list count itself;
//! - a blockquote's paragraphs each become a [`BlockKind::Quote`]. Being inside
//!   a quote decides a block's *kind*, never its depth — an indent a blockquote
//!   contributed could not be reproduced in the output, and the document would
//!   move every time it was read;
//! - everything else keeps its kind at the open list count.
//!
//! Mixed containers therefore flatten: `> - a` yields a bullet and loses the
//! quote. That is the cost of the flat model, and the fixed-point test in
//! [`crate::serialize`] is what keeps it from mattering — whatever the first
//! parse decides is stable from then on.
//!
//! The parse also normalizes what markdown itself would not preserve: leading
//! and trailing whitespace per line, blank lines at a block's edges, headings
//! and table cells flattened to one line, and ordered runs renumbered
//! consecutively. Each of those is a place where writing the document back out
//! and reading it again would otherwise land somewhere new.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, LinkType, Options, Parser, Tag, TagEnd};
use std::ops::Range;

use crate::doc::{Align, Block, BlockKind, Doc, Form, Mark, MarkSpan, Text};

/// Parse a markdown document.
pub fn parse(source: &str) -> Doc {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut state = ParseState::default();
    for event in Parser::new_ext(source, options) {
        state.event(event);
    }
    state.doc.renumber();
    state.doc
}

/// Accumulates one run of inline content and the marks over it.
#[derive(Default)]
struct TextBuilder {
    text: String,
    marks: Vec<MarkSpan>,
    /// Indices into `marks` for the marks still open, innermost last.
    open: Vec<usize>,
}

impl TextBuilder {
    /// Open a mark at the cursor. Marks land in the list in the order they
    /// open, which is outermost first — the ordering [`crate::serialize`] reads
    /// back to reproduce the nesting.
    fn open(&mut self, mark: Mark) {
        let ix = self.marks.len();
        let at = self.text.len();
        self.marks.push(MarkSpan {
            range: at..at,
            mark,
        });
        self.open.push(ix);
    }

    /// Whether anything at all has accumulated — an image with no alt text is
    /// a mark and no text, and still has to close as a block.
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.marks.is_empty()
    }

    fn close(&mut self) {
        if let Some(ix) = self.open.pop() {
            self.marks[ix].range.end = self.text.len();
        }
    }

    /// A mark that opens and closes around `s` in one event (inline code).
    fn wrap(&mut self, mark: Mark, s: &str) {
        let start = self.text.len();
        self.text.push_str(s);
        self.marks.push(MarkSpan {
            range: start..self.text.len(),
            mark,
        });
    }

    fn take(&mut self) -> Text {
        self.open.clear();
        let mut text = normalize(
            &std::mem::take(&mut self.text),
            &std::mem::take(&mut self.marks),
        );
        settle_mentions(&mut text);
        linkify(&mut text);
        text
    }
}

/// A mention the shorthand cannot spell says its name instead.
///
/// [`Form::Auto`] records that `<url>` was written. Where the angles cannot be
/// written back — a `mailto:`, a boundary inside a span emitted whole — the
/// form settles here, so the document already holds what the next parse would
/// produce. A mention alone in its paragraph passes and stays `Auto`, which is
/// what leaves it to become a card.
fn settle_mentions(text: &mut Text) {
    let settled: Vec<usize> = (0..text.marks.len())
        .filter(|ix| {
            matches!(
                text.marks[*ix].mark,
                Mark::Mention {
                    form: Form::Auto,
                    ..
                }
            ) && !is_shorthand(text, *ix)
        })
        .collect();
    for ix in settled {
        if let Mark::Mention { form, .. } = &mut text.marks[ix].mark {
            *form = Form::Chip;
        }
    }
}

/// Whether the mark at `ix` can be written with the `<url>` shorthand.
///
/// The angles hold a bare URL and nothing else, so a mention has to *be* its
/// URL: `<mailto:x>` is an autolink this cannot spell that way, and
/// `**<https://x>**` has a boundary inside a span that is written whole and so
/// has nowhere to put it. Everything that fails here still has the explicit
/// spelling to fall back on, which is why nothing ever has to stop being a
/// mention.
pub(crate) fn is_shorthand(text: &Text, ix: usize) -> bool {
    let span = &text.marks[ix];
    let Mark::Mention { url, form } = &span.mark else {
        return false;
    };
    *form == Form::Auto
        && text.text.get(span.range.clone()) == Some(url.as_str())
        && is_url(url)
        && text.alone(ix)
}

/// The schemes a bare URL may carry. Narrow on purpose: a scheme and no
/// whitespace. Anything cleverer starts linking text that merely contains a dot.
const SCHEMES: [&str; 2] = ["https://", "http://"];

/// Every bare URL in `text`, as byte ranges.
///
/// One scan answers two questions that have to agree: what [`linkify`] marks,
/// and what [`crate::serialize`] may write without brackets. Split them and the
/// round trip drifts the first time the two disagree about a trailing bracket.
pub(crate) fn urls(text: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let Some((start, scheme)) = SCHEMES
            .iter()
            .filter_map(|scheme| text[at..].find(scheme).map(|ix| (at + ix, *scheme)))
            .min_by_key(|(ix, _)| *ix)
        else {
            break;
        };
        let stop = text[start..]
            .find(char::is_whitespace)
            .map_or(text.len(), |ix| start + ix);
        let end = start + trim_url(&text[start..stop]);
        // A scheme mid-word belongs to the word, and a scheme with no host
        // behind it is not a URL.
        let opens = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if opens && end > start + scheme.len() {
            found.push(start..end);
        }
        at = stop.max(start + 1);
    }
    found
}

/// Whether `source` is exactly one bare URL, and nothing else.
///
/// The question an editor asks of a paste, and the one [`crate::serialize`]
/// asks before writing a link bare — the same question, so it is one function.
pub fn is_url(source: &str) -> bool {
    matches!(urls(source).as_slice(), [only] if *only == (0..source.len()))
}

/// How much of a run the URL is. Closing punctuation belongs to the sentence,
/// and a bracket only belongs to the URL when the URL opened it.
fn trim_url(run: &str) -> usize {
    let mut end = run.len();
    while let Some(last) = run[..end].chars().next_back() {
        let keep = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '\'' | '"' => false,
            ')' => run[..end].matches('(').count() >= run[..end].matches(')').count(),
            ']' => run[..end].matches('[').count() >= run[..end].matches(']').count(),
            _ => true,
        };
        if keep {
            break;
        }
        end -= last.len_utf8();
    }
    end
}

/// Mark the bare URLs in a run.
///
/// CommonMark links `<http://x>` and nothing else, so a URL typed on its own
/// arrives as text. Marking it here is what lets a reader click it, what lets
/// [`crate::serialize`] write it back without brackets, and what makes a URL
/// alone in a block a [`BlockKind::Bookmark`].
fn linkify(text: &mut Text) {
    let fresh: Vec<Range<usize>> = urls(&text.text)
        .into_iter()
        .filter(|range| {
            // A URL already inside a link, an image target or a code span is
            // spelled by that mark, not by this one.
            !text.marks.iter().any(|span| {
                matches!(
                    span.mark,
                    Mark::Link(_) | Mark::Mention { .. } | Mark::Image(_) | Mark::Code
                ) && span.range.start < range.end
                    && range.start < span.range.end
            })
        })
        .collect();
    for range in fresh {
        let url = text.text[range.clone()].to_string();
        text.marks.push(MarkSpan {
            range,
            mark: Mark::Link(url),
        });
    }
}

/// Drop the whitespace markdown itself drops, and move the marks with it.
///
/// Leading and trailing spaces on a line are not content — one trailing space
/// is insignificant, two are a hard break, and a continuation line's indent
/// belongs to block structure. Keeping them would mean writing out whitespace
/// that the next parse discards, so the document would change every time it was
/// saved. Blank lines at either end of a block go the same way.
pub(crate) fn normalize(text: &str, marks: &[MarkSpan]) -> Text {
    let bytes = text.as_bytes();
    let mut keep = vec![true; text.len()];

    let mut line_begin = 0;
    for offset in memchr_newlines(text).chain([text.len()]) {
        let line = &text[line_begin..offset];
        let lead = line.len() - line.trim_start_matches([' ', '\t']).len();
        let trail = line.len() - line.trim_end_matches([' ', '\t']).len();
        keep[line_begin..line_begin + lead].fill(false);
        keep[offset - trail..offset].fill(false);
        line_begin = offset + 1;
    }

    let mut head = 0;
    while head < text.len() && (!keep[head] || bytes[head] == b'\n') {
        keep[head] = false;
        head += 1;
    }
    let mut tail = text.len();
    while tail > 0 && (!keep[tail - 1] || bytes[tail - 1] == b'\n') {
        keep[tail - 1] = false;
        tail -= 1;
    }

    let mut out = String::with_capacity(text.len());
    let mut map = vec![0; text.len() + 1];
    for (offset, ch) in text.char_indices() {
        map[offset] = out.len();
        if keep[offset] {
            out.push(ch);
        }
    }
    map[text.len()] = out.len();

    let marks = marks
        .iter()
        .map(|span| MarkSpan {
            range: map[span.range.start]..map[span.range.end],
            mark: span.mark.clone(),
        })
        // A mark left covering nothing has no spelling that survives a
        // round trip — `****` is literal text, not empty bold. An image is the
        // exception: `![](url)` is exactly a mark over no alt text.
        .filter(|span| !span.range.is_empty() || matches!(span.mark, Mark::Image(_)))
        .collect();

    Text {
        text: out,
        marks: merge_same_mark(marks),
    }
}

/// Fuse spans of the same mark that overlap or nest.
///
/// Emphasis inside the same emphasis is redundant — `_a _b_ c_` is italic
/// either way — and two spans of one mark have no unambiguous spelling: written
/// back out, the delimiters pair up differently than they came in. Collapsing
/// them here means the parse produces the one form that survives being written
/// and read again.
fn merge_same_mark(mut marks: Vec<MarkSpan>) -> Vec<MarkSpan> {
    let mut ix = 0;
    while ix < marks.len() {
        let mut fused = None;
        for other in ix + 1..marks.len() {
            let (a, b) = (&marks[ix], &marks[other]);
            if a.mark == b.mark
                && !matches!(a.mark, Mark::Image(_) | Mark::Mention { .. })
                && a.range.start <= b.range.end
                && b.range.start <= a.range.end
            {
                fused = Some((
                    other,
                    a.range.start.min(b.range.start),
                    a.range.end.max(b.range.end),
                ));
                break;
            }
        }
        match fused {
            Some((other, start, end)) => {
                marks[ix].range = start..end;
                marks.remove(other);
            }
            None => ix += 1,
        }
    }
    marks
}

/// Flatten a block whose serialized form is one line.
///
/// A setext heading (`Title\n=====`) and a table cell can both hold a line
/// break that has nowhere to go in the output — an ATX `#` heading ends at its
/// newline, and a second line in a cell would end the row. Both are single-line
/// blocks in this model, and since a newline and a space are each one byte, the
/// marks over them do not move.
pub(crate) fn collapse_to_one_line(text: &mut Text) {
    if text.text.contains('\n') {
        text.text = text.text.replace('\n', " ");
    }
}

fn memchr_newlines(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.bytes()
        .enumerate()
        .filter_map(|(ix, b)| (b == b'\n').then_some(ix))
}

/// A list item's marker, held until the item's first paragraph arrives.
#[derive(Clone, Copy)]
enum Marker {
    Bullet,
    Ordered(u64),
    Task(bool),
}

impl Marker {
    fn into_kind(self, text: Text) -> BlockKind {
        match self {
            Self::Bullet => BlockKind::Bullet(text),
            Self::Ordered(number) => BlockKind::Ordered { number, text },
            Self::Task(checked) => BlockKind::Task { checked, text },
        }
    }
}

#[derive(Default)]
struct TableBuild {
    align: Vec<Align>,
    header: Vec<Text>,
    rows: Vec<Vec<Text>>,
    row: Vec<Text>,
    in_head: bool,
}

#[derive(Default)]
struct ParseState {
    doc: Doc,
    builder: TextBuilder,
    /// One entry per open list; `Some` counts an ordered list's next number.
    lists: Vec<Option<u64>>,
    quote_depth: u8,
    pending_marker: Option<Marker>,
    heading: Option<u8>,
    code: Option<(Option<String>, String)>,
    table: Option<TableBuild>,
}

impl ParseState {
    /// Indent level for a block that is not a list marker.
    ///
    /// Only list nesting counts. A blockquote decides a block's *kind*, not how
    /// deep it sits — so a code block inside a quote stays at the quote's own
    /// level rather than acquiring an indent that nothing in the serialized
    /// output could reproduce.
    fn indent(&self) -> u8 {
        self.lists.len() as u8
    }

    /// Append a block, clamping its indent so the document invariant holds
    /// (first block at 0, never more than one deeper than its predecessor).
    fn push(&mut self, kind: BlockKind, indent: u8) {
        let max = self.doc.blocks.last().map_or(0, |b| b.indent + 1);
        self.doc.blocks.push(Block {
            kind,
            indent: indent.min(max),
        });
    }

    /// Emit a pending marker as an empty block so a non-paragraph leaf (a code
    /// block, a table) nests *under* its bullet instead of replacing it.
    fn flush_marker(&mut self) {
        let Some(marker) = self.pending_marker.take() else {
            return;
        };
        let indent = self.indent().saturating_sub(1);
        self.push(marker.into_kind(Text::default()), indent);
    }

    /// Close any inline content still open as a block.
    ///
    /// A *tight* list item carries no `Paragraph` tags — pulldown-cmark emits
    /// its text directly between `Item` tags — so every block boundary has to
    /// close the run itself rather than waiting for an end tag that never
    /// comes. Table cells are exempt: their builder is per-cell, and closing it
    /// here would push a block out of the middle of a table.
    fn flush_inline(&mut self) {
        if self.table.is_none() && !self.builder.is_empty() {
            self.finish_paragraph();
        }
    }

    /// Close the current run of inline content as a block.
    fn finish_paragraph(&mut self) {
        let text = self.builder.take();

        // A paragraph that is nothing but one image is an image block — the
        // `![](media://…)`-on-its-own-line shape. Anything else keeps the image
        // inline, where it stays an image rather than decaying to a link.
        if let [
            MarkSpan {
                range,
                mark: Mark::Image(url),
            },
        ] = text.marks.as_slice()
            && range.start == 0
            && range.end == text.text.len()
        {
            let (url, alt) = (url.clone(), text.text);
            self.flush_marker();
            let indent = self.indent();
            self.push(BlockKind::Image { url, alt }, indent);
            return;
        }

        // A paragraph that is nothing but a mention is a bookmark — the same
        // `<https://x>` that paints as a chip inside a sentence, given a line
        // of its own. A bare URL is what someone types when they mean a link
        // and `[Title](url)` is what a sentence spells, so carding either would
        // leave no way to write a link that stays one — and it is the paste
        // menu's `Dismiss` that has to write that down.
        //
        // A chip promotes too: off the text flow it can be a real element, and
        // that is the only place a favicon has room to sit.
        //
        // The text has to *be* the URL. `[Example Site](url "chip")` alone on a
        // line keeps its title and stays a paragraph, because promoting it
        // would drop words someone wrote — a block shows only what the preview
        // gave it.
        if let [
            MarkSpan {
                range,
                mark: Mark::Mention { url, form },
            },
        ] = text.marks.as_slice()
            && range.start == 0
            && range.end == text.text.len()
            && text.text == *url
            && is_url(url)
        {
            let (url, form) = (url.clone(), *form);
            self.flush_marker();
            let indent = self.indent();
            self.push(BlockKind::Bookmark { url, form }, indent);
            return;
        }

        if self.quote_depth > 0 {
            // The bullet comes first so the quote reads as its child rather
            // than replacing it.
            self.flush_marker();
            let indent = self.indent();
            self.push(BlockKind::Quote(text), indent);
        } else if let Some(marker) = self.pending_marker.take() {
            let indent = self.indent().saturating_sub(1);
            self.push(marker.into_kind(text), indent);
        } else {
            let indent = self.indent();
            self.push(BlockKind::Paragraph(text), indent);
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),

            Event::Text(t) => match &mut self.code {
                Some((_, code)) => code.push_str(&t),
                None => self.builder.text.push_str(&t),
            },
            Event::Code(t) => self.builder.wrap(Mark::Code, &t),
            // Raw HTML is content, not structure: this model has no HTML node,
            // so it survives as the literal text the author typed.
            Event::Html(t) | Event::InlineHtml(t) => self.builder.text.push_str(&t),
            // Soft and hard breaks are both just a line break in a block —
            // the distinction has no meaning in this model, or in Notion.
            Event::SoftBreak | Event::HardBreak => match &mut self.code {
                Some((_, code)) => code.push('\n'),
                None => self.builder.text.push('\n'),
            },
            Event::Rule => {
                self.flush_inline();
                self.flush_marker();
                let indent = self.indent();
                self.push(BlockKind::Rule, indent);
            }
            Event::TaskListMarker(checked) => {
                self.pending_marker = Some(Marker::Task(checked));
            }
            Event::FootnoteReference(label) => {
                self.builder.text.push_str(&format!("[^{label}]"));
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_inline();
                self.heading = Some(level as u8);
            }
            Tag::BlockQuote(_) => {
                self.flush_inline();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush_inline();
                self.flush_marker();
                let language = match kind {
                    CodeBlockKind::Fenced(info) => {
                        let tag = info.split_whitespace().next().unwrap_or("");
                        (!tag.is_empty()).then(|| tag.to_string())
                    }
                    CodeBlockKind::Indented => None,
                };
                self.code = Some((language, String::new()));
            }
            Tag::List(start) => {
                self.flush_inline();
                // An item whose content is only a nested list still has to emit
                // its own marker first. `flush_inline` covers the item that had
                // text; this covers the empty one, whose pending marker the
                // nested `Start(Item)` would otherwise overwrite — losing a
                // level of nesting. It runs before the push so the marker is
                // numbered at the outer list's depth.
                self.flush_marker();
                self.lists.push(start);
            }
            Tag::Item => {
                self.flush_inline();
                self.pending_marker = Some(match self.lists.last_mut() {
                    Some(Some(number)) => {
                        let n = *number;
                        *number += 1;
                        Marker::Ordered(n)
                    }
                    _ => Marker::Bullet,
                });
            }
            Tag::Table(aligns) => {
                self.flush_inline();
                self.flush_marker();
                self.table = Some(TableBuild {
                    align: aligns.iter().map(align_of).collect(),
                    ..TableBuild::default()
                });
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                }
            }
            Tag::Emphasis => {
                self.builder.open(Mark::Italic);
            }
            Tag::Strong => {
                self.builder.open(Mark::Bold);
            }
            Tag::Strikethrough => {
                self.builder.open(Mark::Strike);
            }
            // A rich link is its own mark rather than a flag on a link: where
            // the spelling came from is what decides the painting, and a flag
            // beside the mark is a second place for that to be recorded.
            Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            } => {
                let url = dest_url.into_string();
                let form = match link_type {
                    LinkType::Autolink => Some(Form::Auto),
                    _ => Form::from_title(&title),
                };
                self.builder.open(match form {
                    Some(form) => Mark::Mention { url, form },
                    None => Mark::Link(url),
                });
            }
            Tag::Image { dest_url, .. } => {
                self.builder.open(Mark::Image(dest_url.into_string()));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => self.flush_inline(),
            TagEnd::Heading(_) => {
                self.flush_marker();
                let level = self.heading.take().unwrap_or(1);
                let mut text = self.builder.take();
                collapse_to_one_line(&mut text);
                let indent = self.indent();
                self.push(BlockKind::Heading { level, text }, indent);
            }
            // Flushed before the depth changes, so trailing text still lands
            // as a quote rather than as a paragraph after it.
            TagEnd::BlockQuote(_) => {
                self.flush_inline();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                if let Some((language, code)) = self.code.take() {
                    let indent = self.indent();
                    // The fence swallows the final newline; storing it would
                    // grow the block by one blank line on every round trip.
                    let code = code.strip_suffix('\n').map_or(code.clone(), str::to_string);
                    self.push(
                        BlockKind::Code {
                            language,
                            code: Text::plain(code),
                        },
                        indent,
                    );
                }
            }
            TagEnd::List(_) => {
                self.flush_inline();
                self.lists.pop();
            }
            // A tight item's text arrives with no `Paragraph` tag to close it,
            // so the item's end is what turns it into the marker block. Only an
            // item that produced nothing at all falls through to an empty one.
            TagEnd::Item => {
                self.flush_inline();
                self.flush_marker();
            }
            TagEnd::Table => {
                if let Some(table) = self.table.take() {
                    let indent = self.indent();
                    self.push(
                        BlockKind::Table {
                            align: table.align,
                            header: table.header,
                            rows: table.rows,
                        },
                        indent,
                    );
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = &mut self.table {
                    table.header = std::mem::take(&mut table.row);
                    table.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = &mut self.table {
                    let row = std::mem::take(&mut table.row);
                    table.rows.push(row);
                }
            }
            TagEnd::TableCell => {
                let mut cell = self.builder.take();
                collapse_to_one_line(&mut cell);
                if let Some(table) = &mut self.table {
                    table.row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.builder.close();
            }
            TagEnd::Image => self.builder.close(),
            _ => {}
        }
    }
}

fn align_of(alignment: &Alignment) -> Align {
    match alignment {
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
        Alignment::Left | Alignment::None => Align::Left,
    }
}
