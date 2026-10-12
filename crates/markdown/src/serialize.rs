//! [`Doc`] → markdown.
//!
//! The guarantee is a **fixed point**: `parse(serialize(parse(s)))` equals
//! `parse(s)` for any input. An edit/save cycle can therefore never drift,
//! which is the property an editor actually needs — stronger than pretty
//! output and weaker (honestly so) than byte-identical round tripping, which a
//! flat model cannot promise for arbitrarily nested CommonMark.
//!
//! Escaping is deliberately narrow. Over-escaping is its own bug: escaping `#`
//! everywhere turns a `#123` reference into `\#123`, which no reader matches.
//! So `#`, `>`, `-` and friends are escaped only at the start of a line, where
//! they would actually mean something.

use crate::{
    doc::{Align, Block, BlockKind, Doc, Mark, Part, Run, Text},
    marks::Marks,
    select::Cursor,
};

/// Four spaces per level: enough to sit inside any list marker's content
/// column (`- ` is 2, `10. ` is 4), and never enough to become an indented
/// code block, because a block at depth N+1 always follows its marker at N.
const INDENT: &str = "    ";

pub fn serialize(doc: &Doc) -> String {
    serialize_with(doc, &Marks::default())
}

impl From<&Doc> for String {
    fn from(doc: &Doc) -> Self {
        serialize(doc)
    }
}

/// [`serialize`] with the app's own marks — see [`crate::Marks`].
pub fn serialize_with(doc: &Doc, marks: &Marks) -> String {
    let layout = Layout::of(&doc.blocks);
    write_range(&doc.blocks, &layout, 0..doc.blocks.len(), marks)
}

/// Where each block stands when written.
///
/// A blank paragraph is not written: markdown has no spelling for one, and
/// blank lines there separate blocks rather than being one. The empty body of
/// an alert is the exception. Every other block's indent is clamped to one
/// level under the written block before it, and its place in a blockquote is
/// read off the written blocks alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    /// One per block: `None` for a block that is not written.
    pub(crate) at: Vec<Option<Place>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    pub(crate) indent: u8,
    pub(crate) run: Option<Run>,
}

impl Layout {
    pub(crate) fn of(blocks: &[Block]) -> Self {
        let mut at = Vec::with_capacity(blocks.len());
        let mut previous: Option<u8> = None;
        let mut runs = crate::doc::Runs::default();
        for block in blocks {
            if block.blank() {
                at.push(None);
                continue;
            }
            let indent = previous.map_or(0, |previous| block.indent.min(previous + 1));
            previous = Some(indent);
            at.push(Some(Place {
                indent,
                run: runs.next(block.quote, indent),
            }));
        }
        Self { at }
    }

    /// The layout of `blocks`, which are the blocks this was laid out for
    /// with `removed` replaced by `added` blocks. Laid out again from the
    /// top-level block before the change to the first top-level block after it
    /// that stands as it did.
    pub(crate) fn respliced(
        &self,
        blocks: &[Block],
        removed: std::ops::Range<usize>,
        added: usize,
    ) -> Self {
        let start = (0..removed.start.min(self.at.len()))
            .rev()
            .find(|&ix| self.top(ix))
            .unwrap_or(0);
        let delta = added as isize - removed.len() as isize;
        let changed_end = removed.start + added;
        let mut at = self.at[..start].to_vec();
        let mut previous: Option<u8> = None;
        let mut runs = crate::doc::Runs::default();
        for (ix, block) in blocks.iter().enumerate().skip(start) {
            let place = (!block.blank()).then(|| {
                let indent = previous.map_or(0, |previous| block.indent.min(previous + 1));
                previous = Some(indent);
                Place {
                    indent,
                    run: runs.next(block.quote, indent),
                }
            });
            let was = (ix as isize - delta) as usize;
            let top = place
                .is_some_and(|place| place.indent == 0 && place.run.is_none_or(|run| run.first));
            if ix >= changed_end && top && self.at.get(was) == Some(&place) {
                at.extend_from_slice(&self.at[was..]);
                return Self { at };
            }
            at.push(place);
        }
        Self { at }
    }

    /// Whether block `ix` is written at the top level: unindented, and opening
    /// its blockquote if it is in one. Markdown read from such a block's start
    /// reads the same as it does in place.
    pub(crate) fn top(&self, ix: usize) -> bool {
        self.at[ix].is_some_and(|place| place.indent == 0 && place.run.is_none_or(|run| run.first))
    }
}

/// Blocks `range` of `blocks` as markdown, each followed by the gap to the
/// next written block. The gap after the range's last written block is the
/// one to the first written block past it, or the document's tail where
/// none is.
///
/// A block that is not written writes nothing and takes no gap.
pub(crate) fn write_range(
    blocks: &[Block],
    layout: &Layout,
    range: std::ops::Range<usize>,
    marks: &Marks,
) -> String {
    let mut out = String::new();
    for ix in range {
        write_one(&mut out, blocks, layout, ix, marks);
    }
    out
}

/// Block `ix` and the gap after it, as [`write_range`] writes them.
pub(crate) fn write_one(
    out: &mut String,
    blocks: &[Block],
    layout: &Layout,
    ix: usize,
    marks: &Marks,
) {
    write_one_as(out, &blocks[ix], blocks, layout, ix, marks);
}

/// [`write_one`] with `block`'s texts in place of block `ix`'s. Everything
/// else, the gap included, is decided by `blocks`.
pub(crate) fn write_one_as(
    out: &mut String,
    block: &Block,
    blocks: &[Block],
    layout: &Layout,
    ix: usize,
    marks: &Marks,
) {
    let Some(place) = layout.at[ix] else { return };
    write_text(out, block, place, marks);
    write_gap_after(out, blocks, layout, ix);
}

/// The gap [`write_one`] writes after block `ix`, alone.
pub(crate) fn write_gap_after(out: &mut String, blocks: &[Block], layout: &Layout, ix: usize) {
    let Some(place) = layout.at[ix] else { return };
    let next = (ix + 1..blocks.len()).find_map(|next| layout.at[next].map(|at| (next, at)));
    match next {
        Some((next, at)) => write_gap(out, &blocks[ix].kind, place, &blocks[next].kind, at),
        None => {
            // GFM reads `[ ]` as a task marker only when whitespace follows
            // it, and an empty task has no text to supply it. Every block but
            // the last is followed by the newline of the block after; the last
            // is followed by nothing.
            if matches!(&blocks[ix].kind, BlockKind::Task { text, .. } if text.is_empty()) {
                out.push(' ');
            }
        }
    }
}

/// A blockquote's `>` stands at its level, and the block is written after it
/// as if the level were the margin: the prefix, and the indent within it.
fn quote_prefix(place: Place) -> (String, u8) {
    match place.run {
        Some(run) => (
            format!("{}> ", INDENT.repeat(run.level as usize)),
            place.indent - run.level,
        ),
        None => (String::new(), place.indent),
    }
}

fn write_text(out: &mut String, block: &Block, place: Place, marks: &Marks) {
    let (prefix, inner) = quote_prefix(place);
    let mut written = String::new();
    write_block(&mut written, &block.kind, inner, marks);
    if let Some(alert) = block.quote.and_then(|quote| quote.alert)
        && place.run.is_some_and(|run| run.first)
    {
        // A marker line stands alone when nothing at all is under it.
        let body = std::mem::take(&mut written);
        written.push_str(alert.marker());
        if !body.is_empty() {
            written.push('\n');
            written.push_str(&body);
        }
    }
    for (ix, line) in written.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
        }
        if line.is_empty() {
            out.push_str(prefix.trim_end());
        } else {
            out.push_str(&prefix);
            out.push_str(line);
        }
    }
}

/// The newlines between a block written at `before` and the next, at `after`.
fn write_gap(
    out: &mut String,
    previous: &BlockKind,
    before: Place,
    next: &BlockKind,
    after: Place,
) {
    out.push('\n');
    // A quote's edge takes a blank line, which is what ends one blockquote
    // before the next block or the next blockquote. Only an empty marker
    // cannot have one after it.
    let edge =
        after.run.is_some_and(|run| run.first) || (after.run.is_none() && before.run.is_some());
    if !tight_after(previous, next, after.indent > before.indent)
        || (edge && !is_empty_marker(previous))
    {
        if after.run.is_some_and(|run| !run.first) {
            out.push_str(quote_prefix(after).0.trim_end());
        }
        out.push('\n');
    }
}

/// Which list a marker block belongs to. Two markers of different kinds are two
/// different lists even when they are adjacent.
fn marker_kind(kind: &BlockKind) -> Option<u8> {
    match kind {
        BlockKind::Bullet(_) => Some(0),
        BlockKind::Ordered { .. } => Some(1),
        BlockKind::Task { .. } => Some(2),
        _ => None,
    }
}

fn is_marker(kind: &BlockKind) -> bool {
    marker_kind(kind).is_some()
}

/// Whether a blank line between these two blocks would be wrong.
///
/// Items of one list stay tight: a blank line between them makes the list loose,
/// and `- a\n- b` should come back out as it went in. A blank line does go
/// between two *different* lists, which is legal — they were already separate —
/// and is the difference between a readable document and a wall.
///
/// An *empty* marker is a special case on both sides, and in opposite
/// directions. Nothing may separate it from what follows: CommonMark lets a
/// list item begin with at most one blank line, so a blank line there ends the
/// list and the item's child becomes a top-level indented code block. But a
/// blank line must come *before* it, because an empty list item cannot
/// interrupt a paragraph — written tight against the item above, it is read as
/// a lazy continuation of that item's text instead of as a list of its own.
///
/// Everywhere else the blank line is required too: without it a child block is
/// read as a lazy continuation of its item.
///
/// `nested` says the next block opens a level deeper, which makes it a *new*
/// list whatever its marker — a checklist under a bullet is as much its own list
/// as a bullet under a bullet, so all that is left to ask is whether it can
/// interrupt the paragraph above it, which it has to do to be seen at all. A
/// bullet may; an ordered list may only when it starts at 1. So a nested `2.`
/// written tight is read as more text in the item above, and needs the blank
/// line that ends that paragraph.
fn tight_after(previous: &BlockKind, next: &BlockKind, nested: bool) -> bool {
    if is_empty_marker(previous) {
        return true;
    }
    if is_empty_marker(next) {
        return false;
    }
    if nested {
        return is_marker(previous)
            && is_marker(next)
            && !matches!(next, BlockKind::Ordered { number, .. } if *number != 1);
    }
    marker_kind(previous).is_some() && marker_kind(previous) == marker_kind(next)
}

fn is_empty_marker(kind: &BlockKind) -> bool {
    is_marker(kind)
        && Block::new(kind.clone())
            .text_at(Part::Body)
            .is_some_and(Text::is_empty)
}

fn write_block(out: &mut String, kind: &BlockKind, indent: u8, marks: &Marks) {
    let pad = INDENT.repeat(indent as usize);

    match kind {
        BlockKind::Paragraph(text) => write_lines(out, &pad, &pad, &inline(text, marks)),
        BlockKind::Heading { level, text } => {
            let hashes = "#".repeat((*level).clamp(1, 6) as usize);
            let body = inline_in(text, marks, Lead::Heading);
            write_lines(out, &format!("{pad}{hashes} "), &pad, &body);
        }
        // A bullet with no text would be written as a line holding nothing but
        // a dash — and a line of dashes directly under a paragraph is a setext
        // heading underline, not a list item. `+` is the bullet marker that
        // cannot be read as one.
        BlockKind::Bullet(text) => {
            let marker = if text.is_empty() { "+ " } else { "- " };
            write_marked(out, &pad, marker, text, marks)
        }
        BlockKind::Ordered { number, text } => {
            write_marked(out, &pad, &format!("{number}. "), text, marks)
        }
        BlockKind::Task { checked, text } => {
            let marker = if *checked { "- [x] " } else { "- [ ] " };
            write_marked(out, &pad, marker, text, marks);
        }
        BlockKind::Code {
            language,
            code,
            height,
        } => {
            let fence = "`".repeat(fence_width(&code.text));
            out.push_str(&pad);
            out.push_str(&fence);
            out.push_str(language.as_deref().unwrap_or(""));
            if let Some(height) = height {
                out.push(' ');
                out.push_str(&height.to_string());
            }
            for line in code.text.split('\n') {
                out.push('\n');
                out.push_str(&pad);
                out.push_str(line);
            }
            out.push('\n');
            out.push_str(&pad);
            out.push_str(&fence);
        }
        BlockKind::Image { url, alt, width } => {
            out.push_str(&pad);
            out.push_str("![");
            escape_inline(out, &alt.text, marks, Lead::Line);
            // After the escaping, and bare: every `|` a caption holds is
            // written `\|` to keep two body lines from reconstituting into a
            // table, so an unescaped one is the delimiter and nothing else.
            if let Some(width) = width {
                out.push('|');
                out.push_str(&width.to_string());
            }
            out.push_str("](");
            write_destination(out, url);
            out.push(')');
        }
        // The angles are what makes a line with a link on it into a card, and
        // they are core CommonMark — every other reader still shows a link
        // here. The other two forms have no shorthand and say their name.
        BlockKind::Bookmark { url, form } => {
            out.push_str(&pad);
            match form.title() {
                None => {
                    out.push('<');
                    out.push_str(url);
                    out.push('>');
                }
                Some(title) => {
                    out.push('[');
                    out.push_str(url);
                    out.push_str("](");
                    write_destination(out, url);
                    out.push_str(&format!(" \"{title}\")"));
                }
            }
        }
        BlockKind::Table {
            align,
            header,
            rows,
        } => write_table(out, &pad, align, header, rows, marks),
        BlockKind::Rule => {
            out.push_str(&pad);
            out.push_str("---");
        }
    }
}

/// A list item: the marker on the first line, its content column on the rest.
fn write_marked(out: &mut String, pad: &str, marker: &str, text: &Text, marks: &Marks) {
    // An empty item has nothing for the marker's space to hold apart from it,
    // so the space is trailing whitespace no one typed.
    let opener = if text.is_empty() {
        marker.trim_end()
    } else {
        marker
    };
    let first = format!("{pad}{opener}");
    let rest = format!("{pad}{}", " ".repeat(marker.chars().count()));
    write_lines(out, &first, &rest, &inline(text, marks));
}

fn write_lines(out: &mut String, first: &str, rest: &str, body: &str) {
    for (ix, line) in body.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
        }
        out.push_str(if ix == 0 { first } else { rest });
        out.push_str(line);
    }
}

/// Long enough to survive any run of backticks the code itself contains.
fn fence_width(code: &str) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for c in code.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    (longest + 1).max(3)
}

fn write_table(
    out: &mut String,
    pad: &str,
    align: &[Align],
    header: &[Text],
    rows: &[Vec<Text>],
    marks: &Marks,
) {
    let columns = align.len().max(header.len());
    let row_of = |cells: &[Text]| {
        let mut line = String::from("|");
        for ix in 0..columns {
            line.push(' ');
            if let Some(cell) = cells.get(ix) {
                // `escape_span` already escapes the pipes.
                line.push_str(&inline(cell, marks));
            }
            line.push_str(" |");
        }
        line
    };

    out.push_str(pad);
    out.push_str(&row_of(header));
    out.push('\n');
    out.push_str(pad);
    out.push('|');
    for ix in 0..columns {
        out.push_str(match align.get(ix).copied().unwrap_or_default() {
            Align::Left => " --- |",
            Align::Center => " :-: |",
            Align::Right => " ---: |",
        });
    }
    for row in rows {
        out.push('\n');
        out.push_str(pad);
        out.push_str(&row_of(row));
    }
}

/// Render inline content with its marks. Marks are stored outermost first, so
/// opening them in order and closing them in reverse reproduces the nesting —
/// which is what keeps `**_x_**` and `_**x**_` distinct.
fn inline(text: &Text, marks: &Marks) -> String {
    inline_in(text, marks, Lead::Line)
}

/// What the first line of inline text follows.
#[derive(Clone, Copy)]
enum Lead {
    /// The start of a line, or a container marker that reads block syntax
    /// after it.
    Line,
    /// An ATX heading's `# `, after which only a run of hashes means anything.
    Heading,
}

fn inline_in(text: &Text, marks: &Marks, lead: Lead) -> String {
    let mut out = String::new();
    let mut open: Vec<usize> = Vec::new();
    let mut started = vec![false; text.marks.len()];
    // The delimiter a span opened with, so it closes with the same one.
    let mut delimiters = vec!['_'; text.marks.len()];
    let mut cursor = 0usize;

    let mut boundaries: Vec<usize> = text
        .marks
        .iter()
        .flat_map(|m| [m.range.start, m.range.end])
        .chain([0, text.text.len()])
        .collect();
    boundaries.sort_unstable();
    boundaries.dedup();

    for point in boundaries {
        if point < cursor {
            continue;
        }
        escape_inline(&mut out, &text.text[cursor..point], marks, lead);
        cursor = point;

        while let Some(&top) = open.last() {
            if text.marks[top].range.end <= point {
                close_mark(&mut out, &text.marks[top].mark, delimiters[top], marks);
                open.pop();
            } else {
                break;
            }
        }

        for (ix, span) in text.marks.iter().enumerate() {
            if started[ix] || span.range.start != point {
                continue;
            }
            started[ix] = true;
            // Code spans are literal to their closing backtick: nothing inside
            // is markup, so they are emitted whole rather than opened.
            if span.mark == Mark::Code {
                let body = &text.text[span.range.clone()];
                let ticks = "`".repeat(fence_width_inline(body));
                out.push_str(&ticks);
                out.push_str(body);
                out.push_str(&ticks);
                cursor = cursor.max(span.range.end);
                continue;
            }
            // The shorthand is its angles, emitted whole for the same reason a
            // code span is: the text between them *is* the URL, so there is
            // nothing inside for another mark to open against. Anything the
            // angles cannot hold falls through to the explicit spelling, which
            // is why a mention never has to stop being one.
            if let Mark::Mention { url, .. } = &span.mark
                && crate::parse::is_shorthand(text, ix)
            {
                out.push('<');
                out.push_str(url);
                out.push('>');
                cursor = cursor.max(span.range.end);
                continue;
            }
            // A link whose text is the URL it points at is written bare, which
            // is what the linkifier reads back — so a URL in a sentence
            // survives byte for byte instead of growing brackets it never had.
            // Only when no other mark touches it: like a code span this is
            // emitted whole, and a boundary inside it would have nowhere to
            // land.
            if let Mark::Link(url) = &span.mark
                && text.text.get(span.range.clone()) == Some(url.as_str())
                && crate::parse::is_url(url)
                && text.alone(ix)
            {
                out.push_str(url);
                cursor = cursor.max(span.range.end);
                continue;
            }
            let italic = italic_delimiter(&out, text, &span.range);
            delimiters[ix] = italic;
            open_mark(&mut out, &span.mark, italic, marks);
            // A mark over nothing — an image with no alt text — closes here.
            // Leaving it on the stack would stretch it to the next boundary.
            if span.range.is_empty() {
                close_mark(&mut out, &span.mark, italic, marks);
            } else {
                open.push(ix);
            }
        }
    }

    escape_inline(
        &mut out,
        &text.text[cursor.min(text.text.len())..],
        marks,
        lead,
    );
    while let Some(ix) = open.pop() {
        close_mark(&mut out, &text.marks[ix].mark, delimiters[ix], marks);
    }
    out
}

fn fence_width_inline(body: &str) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for c in body.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    longest + 1
}

/// Which delimiter spells italic for this span.
///
/// `_` is preferred because it nests unambiguously inside `**` — `***x***` is
/// read as emphasis wrapping strong, so writing bold-outside-italic with stars
/// would come back inside out. But `_` cannot open or close against a letter,
/// so an emphasis that starts or ends mid-word has to use `*` instead.
///
/// What counts as "against a letter" is the *output*, not the source text: a
/// mark opening right after a code span is preceded by a backtick, which is
/// punctuation, even though the character before it in the text is a letter.
/// Deciding from `written` is what keeps `` `a`**_x_** `` from being spelled
/// `***`, which reads back inside out.
fn italic_delimiter(written: &str, text: &Text, range: &std::ops::Range<usize>) -> char {
    let intraword = written
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric)
        || text.text[range.end..]
            .chars()
            .next()
            .is_some_and(char::is_alphanumeric);
    if intraword { '*' } else { '_' }
}

fn open_mark(out: &mut String, mark: &Mark, italic: char, marks: &Marks) {
    match mark {
        Mark::Bold => out.push_str("**"),
        Mark::Italic => out.push(italic),
        Mark::Strike => out.push_str("~~"),
        Mark::Link(_) | Mark::Mention { .. } => out.push('['),
        Mark::Image(_) => out.push_str("!["),
        // A name no registry spells writes nothing and reads back as the text
        // it wrapped, which is the only degradation that cannot corrupt a file.
        Mark::Custom(name) => out.push_str(marks.delimiter(name).unwrap_or("")),
        Mark::Code => {}
    }
}

fn close_mark(out: &mut String, mark: &Mark, italic: char, marks: &Marks) {
    match mark {
        Mark::Bold => out.push_str("**"),
        Mark::Italic => out.push(italic),
        Mark::Strike => out.push_str("~~"),
        Mark::Link(url) | Mark::Image(url) => {
            out.push_str("](");
            write_destination(out, url);
            out.push(')');
        }
        // The title names the form. It is the only slot CommonMark leaves for
        // it, and the shorthand having been ruled out is what got us here.
        Mark::Mention { url, form } => {
            out.push_str("](");
            write_destination(out, url);
            out.push_str(" \"");
            out.push_str(&form.title().unwrap_or_else(|| "chip".to_owned()));
            out.push_str("\")");
        }
        Mark::Custom(name) => out.push_str(marks.delimiter(name).unwrap_or("")),
        Mark::Code => {}
    }
}

/// A link destination, in whichever of CommonMark's two spellings reads back
/// as the URL it was handed. Bare wherever that works, because every reader
/// shows it and it is what a URL was written as; in angles for a destination
/// bare would swallow or cut short — the space in `/My Notes/a.png` ends a
/// bare destination, and the rest of it becomes text.
fn write_destination(out: &mut String, url: &str) {
    if bare_destination(url) {
        return out.push_str(url);
    }
    out.push('<');
    for c in url.chars() {
        match c {
            '<' | '>' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // The one thing neither spelling can hold. Percent-encoding is
            // what a URL says instead, and leaving it raw would end the
            // destination the same way the space did.
            c if c.is_ascii_control() => out.push_str(&format!("%{:02X}", c as u8)),
            c => out.push(c),
        }
    }
    out.push('>');
}

/// Whether `url` survives being written without its angles: no whitespace, no
/// backslash to be read as an escape, and parentheses balanced — an unmatched
/// `)` is where the destination ends.
fn bare_destination(url: &str) -> bool {
    if url.starts_with('<') {
        return false;
    }
    let mut depth = 0i32;
    for c in url.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth == 0 => return false,
            ')' => depth -= 1,
            '\\' => return false,
            c if c.is_whitespace() || c.is_ascii_control() => return false,
            _ => {}
        }
    }
    depth == 0
}

/// Escape only what would otherwise re-parse as syntax.
///
/// Called with slices between mark boundaries, so "line start" means the start
/// of a line in the *output*, not in the slice.
fn escape_inline(out: &mut String, s: &str, marks: &Marks, lead: Lead) {
    let mut line_start = out.is_empty() || out.ends_with('\n');
    let mut lead = if out.is_empty() { lead } else { Lead::Line };
    for (ix, line) in s.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
            line_start = true;
            lead = Lead::Line;
        }
        let body = if line_start {
            escape_block_marker(out, line, lead)
        } else {
            line
        };
        escape_span(out, body, marks);
        line_start = false;
    }
}

/// Escape a leading run that would open a block, returning what is left of the
/// line. Only ever fires at a line start — mid-line these characters are
/// ordinary text, and escaping them there is what turns `#123` into `\#123`.
fn escape_block_marker<'a>(out: &mut String, line: &'a str, lead: Lead) -> &'a str {
    // A position's sentinel is gone once the text is written, so what follows
    // it is what starts the line.
    let placed = line.len()
        - line
            .trim_start_matches(|c| sentinel_index(c).is_some())
            .len();
    if placed > 0 {
        out.push_str(&line[..placed]);
        return escape_block_marker(out, &line[placed..], lead);
    }
    let after_space = |rest: &str| rest.starts_with([' ', '\t']) || rest.is_empty();

    let hashes = line.len() - line.trim_start_matches('#').len();
    if hashes > 0 && after_space(&line[hashes..]) {
        out.push('\\');
        out.push_str(&line[..hashes]);
        return &line[hashes..];
    }
    if let Lead::Heading = lead {
        return line;
    }

    if let Some(rest) = line.strip_prefix('>') {
        out.push_str("\\>");
        return rest;
    }

    // `*` is escaped by `escape_span` wherever it appears, so only `-` and `+`
    // need catching here.
    if (line.starts_with('-') || line.starts_with('+')) && after_space(&line[1..]) {
        out.push('\\');
        out.push_str(&line[..1]);
        return &line[1..];
    }

    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        let after = &line[digits..];
        if (after.starts_with('.') || after.starts_with(')')) && after_space(&after[1..]) {
            out.push_str(&line[..digits]);
            out.push('\\');
            out.push_str(&after[..1]);
            return &after[1..];
        }
    }

    // A run of `-` or `=` alone is a thematic break or a setext underline.
    let trimmed = line.trim_end();
    let mut bare = trimmed
        .chars()
        .filter(|c| sentinel_index(*c).is_none())
        .peekable();
    if bare.peek().is_some() && bare.all(|c| c == '=' || c == '-') {
        out.push('\\');
        out.push_str(&line[..1]);
        return &line[1..];
    }

    line
}

/// Per-character escaping within one line.
fn escape_span(out: &mut String, s: &str, marks: &Marks) {
    let mut skip = 0usize;
    for (ix, c) in s.char_indices() {
        if ix < skip {
            continue;
        }
        let rest = &s[ix + c.len_utf8()..];
        // A registered delimiter standing in the text is text, and has to come
        // back as text: every character of it takes a backslash, or the next
        // read finds a mark nobody wrote. Longest first, so `===` is not
        // escaped as `==` and a stray `=`.
        if let Some(entry) = marks
            .sorted()
            .into_iter()
            .find(|entry| s[ix..].starts_with(entry.delimiter.as_ref()))
        {
            for c in entry.delimiter.chars() {
                out.push('\\');
                out.push(c);
            }
            skip = ix + entry.delimiter.len();
            continue;
        }
        match c {
            // Every tilde, not just a doubled one: GFM strikes on `~x~` as
            // well, so escaping only the first of a pair leaves the survivors
            // to find each other. Pipes are here because two consecutive body
            // lines that happen to look like a header and a delimiter row will
            // otherwise reconstitute themselves into a table.
            '\\' | '*' | '`' | '[' | ']' | '~' | '|' => {
                out.push('\\');
                out.push(c);
            }
            // Intraword underscores are not emphasis in CommonMark, and
            // escaping them would mangle every snake_case identifier.
            '_' => {
                let before = s[..ix].chars().next_back();
                let inside_word = before.is_some_and(char::is_alphanumeric)
                    && rest.chars().next().is_some_and(char::is_alphanumeric);
                if !inside_word {
                    out.push('\\');
                }
                out.push('_');
            }
            // `<` matters for autolinks and raw tags, not for `1 < 2`.
            '<' if rest
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '/' | '!' | '?')) =>
            {
                out.push_str("\\<")
            }
            '&' if rest
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric() || c == '#') =>
            {
                out.push_str("\\&")
            }
            _ => out.push(c),
        }
    }
}

/// The private-use codepoints the position mapping rides on, one per
/// position: plane 15, clear of the ones [`crate::parse`] lifts marks to.
const SENTINELS: std::ops::RangeInclusive<u32> = 0xF0000..=0xFFFFD;

/// The sentinel standing for position `ix`, or `None` past the last one.
pub(crate) fn sentinel(ix: usize) -> Option<char> {
    let code = SENTINELS.start().checked_add(u32::try_from(ix).ok()?)?;
    SENTINELS
        .contains(&code)
        .then(|| char::from_u32(code))
        .flatten()
}

/// Which position a sentinel stands for.
pub(crate) fn sentinel_index(c: char) -> Option<usize> {
    SENTINELS
        .contains(&(c as u32))
        .then(|| (c as u32 - SENTINELS.start()) as usize)
}

/// Whether any text in `doc` already holds a sentinel.
fn carries_sentinel(doc: &Doc) -> bool {
    doc.blocks.iter().any(|block| {
        block.parts().into_iter().any(|part| {
            block
                .text_at(part)
                .is_some_and(|text| text.text.chars().any(|c| sentinel_index(c).is_some()))
        })
    })
}

/// The document as markdown, and where `at` landed in it.
///
/// Exact through markers, escapes and marks because it *is* the serializer: a
/// sentinel goes in at the caret, the document is written, and where the
/// sentinel came out is the answer. The string comes back without it.
///
/// The offset is the end of the output for a caret this cannot place — a
/// document already carrying a sentinel, or a part that no longer exists.
pub fn serialize_at(doc: &Doc, at: Cursor, marks: &Marks) -> (String, usize) {
    let (source, offsets) = serialize_at_many(doc, &[at], marks);
    let end = source.len();
    (source, offsets[0].unwrap_or(end))
}

/// [`serialize_at`] for many positions in one pass: where each of `at`
/// landed, in the same order, `None` for one this cannot place.
pub fn serialize_at_many(doc: &Doc, at: &[Cursor], marks: &Marks) -> (String, Vec<Option<usize>>) {
    let mut doc = doc.clone();
    let mut placed = vec![false; at.len()];
    if !carries_sentinel(&doc) {
        // Back to front, so each insert leaves the offsets still to come where
        // they were.
        let mut order: Vec<usize> = (0..at.len()).collect();
        order.sort_by_key(|&ix| std::cmp::Reverse(at[ix]));
        for ix in order {
            let Some(mark) = sentinel(ix) else { continue };
            let Some(text) = doc
                .blocks
                .get_mut(at[ix].block)
                .and_then(|block| block.text_at_mut(at[ix].part))
            else {
                continue;
            };
            let offset = at[ix].offset.min(text.text.len());
            if text.text.is_char_boundary(offset) {
                text.insert(offset, mark.encode_utf8(&mut [0; 4]));
                placed[ix] = true;
            }
        }
    }
    // Normalized *after* the sentinels go in, so the string this returns is
    // the one the offsets index into — a trailing space is only trailing
    // while nothing sits after it.
    doc.normalize_with(marks);
    let written = serialize_with(&doc, marks);
    if !placed.contains(&true) {
        return (written, vec![None; at.len()]);
    }
    let mut source = String::with_capacity(written.len());
    let mut offsets = vec![None; at.len()];
    for c in written.chars() {
        match sentinel_index(c) {
            Some(ix) => {
                if placed.get(ix) == Some(&true) {
                    offsets[ix] = Some(source.len());
                }
            }
            None => source.push(c),
        }
    }
    (source, offsets)
}
