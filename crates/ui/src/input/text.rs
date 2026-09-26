//! Pure offset math, kept free of gpui so it can be unit-tested.

use super::*;

/// UTF-16 offset (what the platform IME speaks) → byte offset.
pub fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

/// Byte offset → UTF-16 offset.
pub fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }
    utf16_offset
}

/// Platform range → byte range, clamped to character boundaries in `text`.
pub fn range_from_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range.start)..offset_from_utf16(text, range.end)
}

/// Byte range → platform range.
pub fn range_to_utf16(text: &str, range: Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

/// An IME selection is relative to the replacement text, not the document.
pub fn composition_selection(
    text: &str,
    start: usize,
    selection: Option<Range<usize>>,
) -> Range<usize> {
    let range = selection
        .map(|range| range_from_utf16(text, range))
        .unwrap_or(text.len()..text.len());
    start + range.start..start + range.end
}

/// Previous *grapheme* boundary, so arrow keys and backspace step over a flag
/// emoji or a combining mark as one unit instead of splitting it into pieces
/// that render as garbage.
pub fn previous_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(idx, _)| (idx < offset).then_some(idx))
        .unwrap_or(0)
}

/// Next grapheme boundary; clamps to the end of the text.
pub fn next_boundary(text: &str, offset: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(idx, _)| (idx > offset).then_some(idx))
        .unwrap_or(text.len())
}

/// Whether an edit continues the group the last one opened, rather than
/// starting an undo step of its own.
///
/// Two conditions, both structural: the same kind of edit, landing where the
/// last one left the caret. Deliberately not "within N milliseconds" — a time
/// threshold is a number nobody has measured, and adjacency is what actually
/// distinguishes a run of typing from a fresh thought somewhere else.
pub fn joins_group(last: Option<(EditKind, usize)>, kind: EditKind, at: usize) -> bool {
    last.is_some_and(|(last_kind, offset)| last_kind == kind && at == offset)
}

/// The line breaks a field of this shape is allowed to hold.
///
/// CRLF is folded to LF whatever the shape: `shape_text` splits on `\n` alone,
/// so a surviving `\r` shapes as a glyph and puts every offset after it out by
/// one. A single-line field then keeps the text but not the breaks — a pasted
/// newline becomes a space rather than silently truncating what was pasted.
///
/// The invariant this buys: a [`Shape::Line`] field's content never contains a
/// newline, so nothing downstream has to ask whether it might.
pub fn normalize(text: &str, shape: Shape) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    if shape.is_multiline() {
        text
    } else {
        text.replace('\n', " ")
    }
}

/// Start of the logical line holding `offset` — the byte after the previous
/// newline.
///
/// Logical, not visual: with soft wrapping these two readings diverge, and
/// `ctrl-a` here goes to the start of the whole paragraph rather than stopping
/// at the wrap. That is emacs' `C-a`, and a deliberate divergence from macOS
/// `NSTextView`, which stops at the visual row. On text with no newline — every
/// [`Shape::Line`] field — the two are identical.
pub fn line_start(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map_or(0, |at| at + 1)
}

/// End of the logical line holding `offset` — the byte before the next newline.
pub fn line_end(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map_or(text.len(), |at| offset + at)
}

/// A word-bound segment counts as a word if it has any alphanumeric content;
/// whitespace and punctuation runs are the things word motion skips over.
pub(super) fn is_word(segment: &str) -> bool {
    segment.chars().any(char::is_alphanumeric)
}

/// Start of the word at or before `offset` — option-left.
///
/// Word units are Unicode word bounds (UAX#29), not space-delimited runs. In
/// practice that means `foo.bar` and `foo_bar` are ONE word — a dot or
/// underscore between letters does not break — while `a-b`, `path/to/file` and
/// `foo, bar` do break. Good defaults for identifiers and paths, and verified
/// against the segmenter rather than assumed (see tests).
pub fn previous_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .filter(|(start, _)| *start < offset)
        .rfind(|(_, segment)| is_word(segment))
        .map(|(start, _)| start)
        .unwrap_or(0)
}

/// End of the word at or after `offset` — option-right.
pub fn next_word_boundary(text: &str, offset: usize) -> usize {
    text.split_word_bound_indices()
        .filter(|(start, segment)| start + segment.len() > offset)
        .find(|(_, segment)| is_word(segment))
        .map(|(start, segment)| start + segment.len())
        .unwrap_or(text.len())
}
