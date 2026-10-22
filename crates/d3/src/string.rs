//! JavaScript strings: whitespace and word characters as its regular
//! expressions class them, lengths and order in UTF-16 units, and
//! `replace`.

use std::{cmp::Ordering, ops::Range};

/// What `\s` matches, as the inside of a `regex` class.
pub const SPACE: &str = "\t\n\x0B\x0C\r \u{a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

/// Whitespace as `\s` sees it, and what `trim` strips.
pub fn is_space(c: impl Into<u32>) -> bool {
    let c = c.into();
    if c < 128 {
        return c == 32 || (9..=13).contains(&c);
    }
    c == 0xa0
        || c == 0x1680
        || (0x2000..=0x200a).contains(&c)
        || c == 0x2028
        || c == 0x2029
        || c == 0x202f
        || c == 0x205f
        || c == 0x3000
        || c == 0xfeff
}

/// A line terminator: what `.` does not match.
pub fn is_line_end(c: impl Into<u32>) -> bool {
    let c = c.into();
    c == 10 || c == 13 || c == 0x2028 || c == 0x2029
}

/// A character of `\w`.
pub fn is_word(c: impl Into<u32>) -> bool {
    let c = c.into();
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}

/// `text.trim()`.
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// `text.trimStart()`.
pub fn trim_start(text: &str) -> &str {
    text.trim_start_matches(is_space)
}

/// `text.trimEnd()`.
pub fn trim_end(text: &str) -> &str {
    text.trim_end_matches(is_space)
}

/// The end of the run of whitespace at byte `at` of `text`.
pub fn space_end(text: &str, at: usize) -> usize {
    text.len() - trim_start(&text[at..]).len()
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

/// `a < b` on strings: by their UTF-16 code units.
pub fn compare(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `list.sort()`: strings in order of their UTF-16 code units.
pub fn sort(list: &mut [String]) {
    list.sort_by(|a, b| compare(a, b));
}

/// `text.replace(pattern, replacement)` with a string `pattern`: its first
/// occurrence, `replacement` read for `$` patterns.
pub fn replace(text: &str, pattern: &str, replacement: &str) -> String {
    let Some(at) = text.find(pattern) else {
        return text.to_string();
    };
    let end = at + pattern.len();
    let with = substitute(replacement, text, at..end, &[], &[]);
    format!("{}{with}{}", &text[..at], &text[end..])
}

/// `GetSubstitution`: `replacement` with `$$`, `$&`, `` $` ``, `$'`, `$n`,
/// `$nn` and, when `named` is not empty, `$<name>` expanded, for the match
/// `matched` of `text` with these groups.
pub fn substitute(
    replacement: &str,
    text: &str,
    matched: Range<usize>,
    groups: &[Option<&str>],
    named: &[(&str, Option<&str>)],
) -> String {
    let mut out = String::with_capacity(replacement.len());
    let mut rest = replacement;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let bytes = tail.as_bytes();
        let group = |n: usize| (1..=groups.len()).contains(&n).then(|| groups[n - 1]);
        let used = match bytes.first() {
            Some(b'$') => {
                out.push('$');
                1
            }
            Some(b'&') => {
                out.push_str(&text[matched.clone()]);
                1
            }
            Some(b'`') => {
                out.push_str(&text[..matched.start]);
                1
            }
            Some(b'\'') => {
                out.push_str(&text[matched.end..]);
                1
            }
            Some(d) if d.is_ascii_digit() => {
                let one = usize::from(d - b'0');
                let two = bytes
                    .get(1)
                    .filter(|d| d.is_ascii_digit())
                    .map(|d2| one * 10 + usize::from(d2 - b'0'));
                match (two.and_then(group), group(one)) {
                    (Some(text), _) => {
                        out.push_str(text.unwrap_or(""));
                        2
                    }
                    (None, Some(text)) => {
                        out.push_str(text.unwrap_or(""));
                        1
                    }
                    (None, None) => {
                        out.push('$');
                        0
                    }
                }
            }
            Some(b'<') if !named.is_empty() => match tail.find('>') {
                Some(end) => {
                    let name = &tail[1..end];
                    let group = named.iter().find(|(n, _)| *n == name);
                    out.push_str(group.and_then(|(_, text)| *text).unwrap_or(""));
                    end + 1
                }
                None => {
                    out.push('$');
                    0
                }
            },
            _ => {
                out.push('$');
                0
            }
        };
        rest = &tail[used..];
    }
    out.push_str(rest);
    out
}
