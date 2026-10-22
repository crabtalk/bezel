//! JavaScript's string operations where Rust's differ: `trim` with `\s`'s
//! whitespace, and lengths and order in UTF-16 units.

use crate::util::chars;

/// `text.trim()`.
pub fn trim(text: &str) -> &str {
    text.trim_matches(chars::is_space)
}

/// `text.trimStart()`.
pub fn trim_start(text: &str) -> &str {
    text.trim_start_matches(chars::is_space)
}

/// `text.trimEnd()`.
pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(chars::is_space)
}

/// `text.length`: UTF-16 units.
pub fn len(text: &str) -> usize {
    // One unit per character, two for the four-byte ones.
    text.bytes()
        .map(|b| usize::from(b & 0xc0 != 0x80) + usize::from(b >= 0xf0))
        .sum()
}

/// The byte offset in `text` after its first `units` UTF-16 units, at the
/// end of the character holding the last of them; the length of `text` when
/// it is shorter.
pub fn units_end(text: &str, units: usize) -> usize {
    let mut taken = 0;
    for (at, c) in text.char_indices() {
        if taken >= units {
            return at;
        }
        taken += c.len_utf16();
    }
    text.len()
}

/// Whether [`len`] of `text` is over `limit`, counted only when its bytes
/// are: no character has fewer UTF-8 bytes than UTF-16 units.
pub fn len_over(text: &str, limit: usize) -> bool {
    text.len() > limit && len(text) > limit
}

/// `list.sort()`: strings in order of their UTF-16 code units.
pub fn sort(list: &mut [String]) {
    list.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
}

/// The end of the run of whitespace at byte `at` of `text`.
pub fn space_end(text: &str, at: usize) -> usize {
    text.len() - trim_start(&text[at..]).len()
}
