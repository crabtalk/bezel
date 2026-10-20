//! Tests on characters, as `char`s or as the codes the lexers read with
//! `charCodeAt`, and text between delimiters. Past the end of the text the
//! code is [`NONE`], which is none of these.

/// What `charCodeAt` answers past the end.
pub const NONE: u32 = u32::MAX;

/// The character at byte `at` of `src`, as a code, or [`NONE`].
pub fn code_at(src: &str, at: usize) -> u32 {
    match src.as_bytes().get(at) {
        None => NONE,
        Some(&b) if b < 128 => u32::from(b),
        Some(_) => src
            .get(at..)
            .and_then(|rest| rest.chars().next())
            .map_or(NONE, u32::from),
    }
}

/// The character at byte `at` of `src`, or `None` past the end.
pub fn char_at(src: &str, at: usize) -> Option<char> {
    src.get(at..)?.chars().next()
}

/// Bytes the character at `at` takes; 1 past the end.
pub fn width_at(src: &str, at: usize) -> usize {
    src.get(at..)
        .and_then(|rest| rest.chars().next())
        .map_or(1, char::len_utf8)
}

/// Whitespace as `\s` sees it.
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

/// Whitespace as `\s` sees it, other than a line feed or carriage return.
pub fn is_inline_space(c: impl Into<u32>) -> bool {
    let c = c.into();
    is_space(c) && c != u32::from('\n') && c != u32::from('\r')
}

pub fn is_blank(c: impl Into<u32>) -> bool {
    let c = c.into();
    c == 32 || c == 9
}

/// A line terminator: what `.` does not match.
pub fn is_line_end(c: impl Into<u32>) -> bool {
    let c = c.into();
    c == 10 || c == 13 || c == 0x2028 || c == 0x2029
}

pub fn is_digit(c: impl Into<u32>) -> bool {
    let c = c.into();
    (48..=57).contains(&c)
}

pub fn is_letter(c: impl Into<u32>) -> bool {
    let c = c.into();
    (65..=90).contains(&c) || (97..=122).contains(&c)
}

pub fn is_hex(c: impl Into<u32>) -> bool {
    let c = c.into();
    (48..=57).contains(&c) || (65..=70).contains(&c) || (97..=102).contains(&c)
}

/// A character of `\w`.
pub fn is_word(c: impl Into<u32>) -> bool {
    let c = c.into();
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}

/// Whether `word`, written in lower case, is at byte `at` of `src`. Only
/// ASCII letters match in either case, as with the `i` flag without `u`.
pub fn starts_with_caseless(src: &str, at: usize, word: &str) -> bool {
    let bytes = src.as_bytes();
    word.bytes().enumerate().all(|(k, w)| {
        bytes
            .get(at + k)
            .is_some_and(|&c| c == w || (w.is_ascii_lowercase() && c == w - 32))
    })
}

/// The end of the run of ASCII digits at byte `from` of `src`.
pub fn digits_end(src: &str, from: usize) -> usize {
    from + src.get(from..).map_or(0, |rest| {
        rest.bytes().take_while(u8::is_ascii_digit).count()
    })
}

/// The UTF-16 unit after the character at byte `at`, as `charCodeAt(i + 1)`
/// reads it, and the byte where that unit's character starts. After an
/// astral character it is the second half of the pair, which is none of
/// these tests' characters.
pub fn code_after(src: &str, at: usize) -> (u32, usize) {
    let width = width_at(src, at);
    if width == 4 {
        (0xdc00, at + 4)
    } else {
        (code_at(src, at + width), at + width)
    }
}

/// `text.slice(1, -1)`: `text` without its first and last bytes, which are
/// ASCII delimiters.
pub fn inner(text: &str) -> &str {
    if text.len() < 2 {
        return "";
    }
    &text[1..text.len() - 1]
}

/// `text` without one pair of double quotes around it.
pub fn unquote(text: &str) -> &str {
    if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
        inner(text)
    } else {
        text
    }
}
