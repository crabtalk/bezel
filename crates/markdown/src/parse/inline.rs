//! Inline post-processing: mentions, links, marks and one-line collapsing.

use super::*;

/// A mention the shorthand cannot spell says its name instead.
///
/// [`Form::Auto`] records that `<url>` was written. Where the angles cannot be
/// written back — a `mailto:`, a boundary inside a span emitted whole — the
/// form settles here, so the document already holds what the next parse would
/// produce. A mention alone in its paragraph passes and stays `Auto`, which is
/// what leaves it to become a card.
pub(super) fn settle_mentions(text: &mut Text) {
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
pub(super) const SCHEMES: [&str; 2] = ["https://", "http://"];

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

/// Whether a URL or a path names a picture, by the only thing either says
/// about itself without being fetched — its extension, against what gpui can
/// decode.
///
/// What decides whether a paste or a drop is worth offering as an image. A
/// server is free to disagree; the answer is a guess about a name, and the
/// alternative is a menu row that paints a broken box.
pub fn is_image(source: &str) -> bool {
    let path = source.split(['?', '#']).next().unwrap_or(source);
    let Some((_, extension)) = path.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "avif"
    )
}

/// How much of a run the URL is. Closing punctuation belongs to the sentence,
/// and a bracket only belongs to the URL when the URL opened it.
pub(super) fn trim_url(run: &str) -> usize {
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
pub(super) fn linkify(text: &mut Text) {
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
pub(super) fn merge_same_mark(mut marks: Vec<MarkSpan>) -> Vec<MarkSpan> {
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

/// Split a trailing `|480` off an image's alt text, which is where a width is
/// written down.
///
/// Obsidian's spelling, and the only one the parser leaves intact: `{width=480}`
/// trails as literal text and breaks the paragraph out of being an image at all,
/// and `=480x` is not an image to begin with. The last `|` wins, so a caption
/// may hold its own — but one *ending* in `|123` gives that tail up, because the
/// escape that tells them apart on disk is gone by the time this reads it.
pub(super) fn split_width(alt: &str) -> (&str, Option<u32>) {
    let Some((caption, tail)) = alt.rsplit_once('|') else {
        return (alt, None);
    };
    // A zero would paint a picture no pixels wide, and nothing that writes one
    // can produce it — the drag floors at `MIN_IMAGE_WIDTH`.
    match tail.parse().ok().filter(|width| *width > 0) {
        Some(width) => (caption, Some(width)),
        None => (alt, None),
    }
}

pub(super) fn memchr_newlines(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.bytes()
        .enumerate()
        .filter_map(|(ix, b)| (b == b'\n').then_some(ix))
}
