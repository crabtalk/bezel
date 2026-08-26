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

use crate::doc::{Align, Block, BlockKind, Doc, Mark, Part, Text};

/// Four spaces per level: enough to sit inside any list marker's content
/// column (`- ` is 2, `10. ` is 4), and never enough to become an indented
/// code block, because a block at depth N+1 always follows its marker at N.
const INDENT: &str = "    ";

pub fn serialize(doc: &Doc) -> String {
    let mut out = String::new();
    let mut previous: Option<(&BlockKind, u8)> = None;

    for block in &doc.blocks {
        let indent = match previous {
            Some((_, prev)) => block.indent.min(prev + 1),
            None => 0,
        };

        if let Some((prev_kind, prev_indent)) = previous {
            out.push('\n');
            if !tight_after(prev_kind, &block.kind, indent > prev_indent) {
                out.push('\n');
            }
        }

        write_block(&mut out, &block.kind, indent);
        previous = Some((&block.kind, indent));
    }

    // GFM reads `[ ]` as a task marker only when whitespace follows it, and an
    // empty task has no text to supply it. Every block but the last is followed
    // by the newline of the block after; the last is followed by nothing.
    if doc
        .blocks
        .last()
        .is_some_and(|block| matches!(&block.kind, BlockKind::Task { text, .. } if text.is_empty()))
    {
        out.push(' ');
    }

    out
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

fn write_block(out: &mut String, kind: &BlockKind, indent: u8) {
    let pad = INDENT.repeat(indent as usize);

    match kind {
        BlockKind::Paragraph(text) => write_lines(out, &pad, &pad, &inline(text)),
        BlockKind::Heading { level, text } => {
            let hashes = "#".repeat((*level).clamp(1, 6) as usize);
            write_lines(out, &format!("{pad}{hashes} "), &pad, &inline(text));
        }
        // A bullet with no text would be written as a line holding nothing but
        // a dash — and a line of dashes directly under a paragraph is a setext
        // heading underline, not a list item. `+` is the bullet marker that
        // cannot be read as one.
        BlockKind::Bullet(text) => {
            let marker = if text.is_empty() { "+ " } else { "- " };
            write_marked(out, &pad, marker, text)
        }
        BlockKind::Ordered { number, text } => {
            write_marked(out, &pad, &format!("{number}. "), text)
        }
        BlockKind::Task { checked, text } => {
            let marker = if *checked { "- [x] " } else { "- [ ] " };
            write_marked(out, &pad, marker, text);
        }
        BlockKind::Quote(text) => {
            let prefix = format!("{pad}> ");
            write_lines(out, &prefix, &prefix, &inline(text));
        }
        BlockKind::Code { language, code } => {
            let fence = "`".repeat(fence_width(&code.text));
            out.push_str(&pad);
            out.push_str(&fence);
            out.push_str(language.as_deref().unwrap_or(""));
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
            escape_inline(out, &alt.text);
            // After the escaping, and bare: every `|` a caption holds is
            // written `\|` to keep two body lines from reconstituting into a
            // table, so an unescaped one is the delimiter and nothing else.
            if let Some(width) = width {
                out.push('|');
                out.push_str(&width.to_string());
            }
            out.push_str("](");
            out.push_str(url);
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
                    out.push_str(url);
                    out.push_str(&format!(" \"{title}\")"));
                }
            }
        }
        BlockKind::Table {
            align,
            header,
            rows,
        } => write_table(out, &pad, align, header, rows),
        BlockKind::Rule => {
            out.push_str(&pad);
            out.push_str("---");
        }
    }
}

/// A list item: the marker on the first line, its content column on the rest.
fn write_marked(out: &mut String, pad: &str, marker: &str, text: &Text) {
    // An empty item has nothing for the marker's space to hold apart from it,
    // so the space is trailing whitespace no one typed.
    let opener = if text.is_empty() {
        marker.trim_end()
    } else {
        marker
    };
    let first = format!("{pad}{opener}");
    let rest = format!("{pad}{}", " ".repeat(marker.chars().count()));
    write_lines(out, &first, &rest, &inline(text));
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

fn write_table(out: &mut String, pad: &str, align: &[Align], header: &[Text], rows: &[Vec<Text>]) {
    let columns = align.len().max(header.len());
    let row_of = |cells: &[Text]| {
        let mut line = String::from("|");
        for ix in 0..columns {
            line.push(' ');
            if let Some(cell) = cells.get(ix) {
                // `escape_span` already escapes the pipes.
                line.push_str(&inline(cell));
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
fn inline(text: &Text) -> String {
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
        escape_inline(&mut out, &text.text[cursor..point]);
        cursor = point;

        while let Some(&top) = open.last() {
            if text.marks[top].range.end <= point {
                close_mark(&mut out, &text.marks[top].mark, delimiters[top]);
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
            open_mark(&mut out, &span.mark, italic);
            // A mark over nothing — an image with no alt text — closes here.
            // Leaving it on the stack would stretch it to the next boundary.
            if span.range.is_empty() {
                close_mark(&mut out, &span.mark, italic);
            } else {
                open.push(ix);
            }
        }
    }

    escape_inline(&mut out, &text.text[cursor.min(text.text.len())..]);
    while let Some(ix) = open.pop() {
        close_mark(&mut out, &text.marks[ix].mark, delimiters[ix]);
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

fn open_mark(out: &mut String, mark: &Mark, italic: char) {
    match mark {
        Mark::Bold => out.push_str("**"),
        Mark::Italic => out.push(italic),
        Mark::Strike => out.push_str("~~"),
        Mark::Link(_) | Mark::Mention { .. } => out.push('['),
        Mark::Image(_) => out.push_str("!["),
        Mark::Code => {}
    }
}

fn close_mark(out: &mut String, mark: &Mark, italic: char) {
    match mark {
        Mark::Bold => out.push_str("**"),
        Mark::Italic => out.push(italic),
        Mark::Strike => out.push_str("~~"),
        Mark::Link(url) | Mark::Image(url) => {
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        }
        // The title names the form. It is the only slot CommonMark leaves for
        // it, and the shorthand having been ruled out is what got us here.
        Mark::Mention { url, form } => {
            out.push_str("](");
            out.push_str(url);
            out.push_str(" \"");
            out.push_str(form.title().unwrap_or("chip"));
            out.push_str("\")");
        }
        Mark::Code => {}
    }
}

/// Escape only what would otherwise re-parse as syntax.
///
/// Called with slices between mark boundaries, so "line start" means the start
/// of a line in the *output*, not in the slice.
fn escape_inline(out: &mut String, s: &str) {
    let mut line_start = out.is_empty() || out.ends_with('\n');
    for (ix, line) in s.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
            line_start = true;
        }
        let body = if line_start {
            escape_block_marker(out, line)
        } else {
            line
        };
        escape_span(out, body);
        line_start = false;
    }
}

/// Escape a leading run that would open a block, returning what is left of the
/// line. Only ever fires at a line start — mid-line these characters are
/// ordinary text, and escaping them there is what turns `#123` into `\#123`.
fn escape_block_marker<'a>(out: &mut String, line: &'a str) -> &'a str {
    let after_space = |rest: &str| rest.starts_with([' ', '\t']) || rest.is_empty();

    let hashes = line.len() - line.trim_start_matches('#').len();
    if hashes > 0 && after_space(&line[hashes..]) {
        out.push('\\');
        out.push_str(&line[..hashes]);
        return &line[hashes..];
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
    if !trimmed.is_empty() && trimmed.chars().all(|c| c == '=' || c == '-') {
        out.push('\\');
        out.push_str(&line[..1]);
        return &line[1..];
    }

    line
}

/// Per-character escaping within one line.
fn escape_span(out: &mut String, s: &str) {
    for (ix, c) in s.char_indices() {
        let rest = &s[ix + c.len_utf8()..];
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
