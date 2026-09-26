//! Parsing with the caret carried through, and custom marks lifted to sentinels.

use super::*;

/// Parse markdown, and say where `offset` in it landed in the document.
///
/// The inverse of [`crate::serialize_at`] and the same trick: a sentinel goes
/// into the source at the offset, the source is parsed, and the text holding
/// the sentinel is the caret's. The document comes back without it.
///
/// The caret is the start of the document where the sentinel would have
/// changed what the source *means* — between a `#` and its space, inside a
/// fence's delimiter — because a caret in the right place is worth less than a
/// document that is still the one you were editing.
pub fn parse_at(source: &str, offset: usize, marks: &Marks) -> (Doc, Cursor) {
    let plain = parse_with(source, marks);
    let start = || (plain.clone(), Cursor::default().clamp(&plain));
    if source.contains(crate::serialize::SENTINEL) {
        return start();
    }
    let mut marked = String::with_capacity(source.len() + 3);
    let offset = offset.min(source.len());
    if !source.is_char_boundary(offset) {
        return start();
    }
    marked.push_str(&source[..offset]);
    marked.push(crate::serialize::SENTINEL);
    marked.push_str(&source[offset..]);

    let mut doc = parse_with(&marked, marks);
    let Some(at) = find(&doc) else { return start() };
    let Some(text) = doc
        .blocks
        .get_mut(at.block)
        .and_then(|block| block.text_at_mut(at.part))
    else {
        return start();
    };
    text.remove(at.offset..at.offset + crate::serialize::SENTINEL.len_utf8());
    // The sentinel is a character like any other to the parser, so a document
    // it changed the shape of is not the one the caller handed in.
    if doc != plain { start() } else { (doc, at) }
}

/// Where the sentinel sits, in document order.
pub(super) fn find(doc: &Doc) -> Option<Cursor> {
    doc.blocks.iter().enumerate().find_map(|(ix, block)| {
        block.parts().into_iter().find_map(|part| {
            let at = block.text_at(part)?.text.find(crate::serialize::SENTINEL)?;
            Some(Cursor::new(ix, part, at))
        })
    })
}

/// A registered mark, lifted out of the source and into two private-use
/// characters CommonMark carries through as ordinary text.
///
/// The pair rather than the delimiter itself, because the delimiter is what the
/// escape question is about: by the time pulldown has finished, `\=\=` and `==`
/// are the same two bytes, and only the source still knows which was written.
pub(super) const OPEN: char = '\u{E010}';
pub(super) const CLOSE: char = '\u{E011}';

/// Which registered mark an [`OPEN`] belongs to, as a character of its own so
/// the pair needs no length prefix.
pub(super) fn tag(ix: usize) -> Option<char> {
    char::from_u32(0xE020 + u32::try_from(ix).ok()?).filter(|_| ix < 0x100)
}

pub(super) fn tag_index(c: char) -> Option<usize> {
    (0xE020..0xE120)
        .contains(&(c as u32))
        .then(|| c as usize - 0xE020)
}

/// The source with every registered delimiter pair replaced by its sentinels.
pub(super) fn lift(source: &str, marks: &Marks) -> String {
    let skipped = literal(source);
    let entries = marks.sorted();
    let mut out = String::with_capacity(source.len());
    let mut open: Vec<(usize, &str)> = Vec::new();
    let mut at = 0usize;

    while at < source.len() {
        // Inside a fence, a code span or a link's destination the delimiter is
        // not markup and never was.
        if let Some(range) = skipped.iter().find(|range| range.contains(&at)) {
            out.push_str(&source[at..range.end]);
            at = range.end;
            continue;
        }
        let rest = &source[at..];
        // A backslash takes the next character with it, delimiter or not.
        if let Some(escaped) = rest.strip_prefix('\\') {
            let width = escaped.chars().next().map_or(1, |c| 1 + c.len_utf8());
            out.push_str(&rest[..width.min(rest.len())]);
            at += width.min(rest.len());
            continue;
        }
        let found = entries
            .iter()
            .find(|entry| rest.starts_with(entry.delimiter.as_ref()));
        if let Some(entry) = found {
            let delimiter: &str = entry.delimiter.as_ref();
            let closes = open.last().is_some_and(|(_, open)| *open == delimiter);
            if closes && !source[..at].ends_with(char::is_whitespace) {
                out.push(CLOSE);
                open.pop();
                at += delimiter.len();
                continue;
            }
            if !closes
                && let Some(ix) = marks.position(entry)
                && let Some(tag) = tag(ix)
                && closing(
                    source,
                    at + delimiter.len(),
                    delimiter,
                    &skipped,
                    line_end(source, at),
                )
            {
                out.push(OPEN);
                out.push(tag);
                open.push((ix, delimiter));
                at += delimiter.len();
                continue;
            }
        }
        let c = rest.chars().next().unwrap_or_default();
        out.push(c);
        at += c.len_utf8();
    }
    out
}

/// Whether a delimiter opened at `from` has a partner to close against: an
/// unescaped one, on the same line, outside everything literal, with something
/// between them that neither opens nor closes on a space — the rule emphasis
/// already follows.
///
/// The same line, and only ever the same line. Emphasis may reach across a soft
/// break; a mark this crate does not know the meaning of may not, because the
/// next line may belong to another block — a lazy continuation out of a quote,
/// a list item's second paragraph — and no mark can span two of those. An open
/// with no close on its own line stays the text it was written as.
pub(super) fn closing(
    source: &str,
    from: usize,
    delimiter: &str,
    skipped: &[Range<usize>],
    line_end: usize,
) -> bool {
    if source[from..].starts_with(char::is_whitespace) {
        return false;
    }
    let mut at = from;
    while let Some(found) = source[at..line_end.max(at)].find(delimiter) {
        let found = at + found;
        let escaped = source[..found].ends_with('\\');
        let literal = skipped.iter().any(|range| range.contains(&found));
        let spaced = source[..found].ends_with(char::is_whitespace);
        if !escaped && !literal && !spaced && found > from {
            return true;
        }
        at = found + delimiter.len();
    }
    false
}

/// Where the line `at` sits on ends.
pub(super) fn line_end(source: &str, at: usize) -> usize {
    source[at..].find('\n').map_or(source.len(), |ix| at + ix)
}

/// The source ranges a delimiter means nothing in: a fence, a code span, raw
/// HTML, and a link's destination.
pub(super) fn literal(source: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    for (event, range) in Parser::new_ext(source, OPTIONS).into_offset_iter() {
        match event {
            Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => out.push(range),
            Event::Start(Tag::CodeBlock(_)) => out.push(range),
            // A link's destination only: its label is prose, and a mark is
            // welcome in it. An autolink has no `](` and is a destination all
            // through.
            Event::Start(Tag::Link { .. }) => {
                let at = source[range.clone()]
                    .rfind("](")
                    .map_or(range.start, |ix| range.start + ix);
                out.push(at..range.end);
            }
            // A picture whole: its label is alt text, which markdown writes as
            // a plain string — a mark placed there would have nowhere to go on
            // the way out.
            Event::Start(Tag::Image { .. }) => out.push(range),
            _ => {}
        }
    }
    out
}

/// Take the sentinels back out of a parsed text, leaving the marks they stood
/// for — and move every mark the ordinary parse produced, whose offsets were
/// measured with the sentinels still in.
pub(super) fn settle(text: &mut Text, marks: &Marks) {
    if !text.text.contains(OPEN) {
        return;
    }
    let mut settled = String::with_capacity(text.text.len());
    // Where a sentinel was, and how many bytes it took with it.
    let mut cut: Vec<(usize, usize)> = Vec::new();
    // Each open takes a number, because a mark is closed inner first and the
    // list is read outermost first — `++==x==++` is underline over highlight,
    // and writing it the other way round is a different document.
    let mut open: Vec<(usize, usize, usize)> = Vec::new();
    let mut found: Vec<(usize, MarkSpan)> = Vec::new();
    let mut opened = 0usize;
    let mut chars = text.text.char_indices();

    while let Some((at, c)) = chars.next() {
        match c {
            OPEN => {
                let width = match chars.next() {
                    Some((_, tag)) => {
                        if let Some(ix) = tag_index(tag) {
                            open.push((ix, settled.len(), opened));
                            opened += 1;
                        }
                        OPEN.len_utf8() + tag.len_utf8()
                    }
                    None => OPEN.len_utf8(),
                };
                cut.push((at, width));
            }
            CLOSE => {
                if let Some((ix, from, seq)) = open.pop()
                    && let Some(entry) = marks.index(ix)
                {
                    found.push((
                        seq,
                        MarkSpan {
                            range: from..settled.len(),
                            mark: Mark::Custom(entry.name.to_string()),
                        },
                    ));
                }
                cut.push((at, CLOSE.len_utf8()));
            }
            _ => settled.push(c),
        }
    }

    let moved = |offset: usize| {
        offset
            - cut
                .iter()
                .filter(|(at, _)| *at < offset)
                .map(|(_, width)| width)
                .sum::<usize>()
    };
    for span in &mut text.marks {
        span.range = moved(span.range.start)..moved(span.range.end);
    }
    text.text = settled;
    found.sort_by_key(|(seq, _)| *seq);
    text.marks.extend(found.into_iter().map(|(_, span)| span));
    // Outermost first is what the serializer writes the nesting from. A stable
    // sort leaves the ordinary marks in the order the parse put them.
    text.marks
        .sort_by_key(|span| (span.range.start, std::cmp::Reverse(span.range.end)));
    text.marks.retain(|span| !span.range.is_empty());
}
