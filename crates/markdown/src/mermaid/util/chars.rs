//! Tests on characters as the lexers read them with `charCodeAt`. Past the
//! end of the text the code is [`NONE`], which is none of these.

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

/// Bytes the character at `at` takes; 1 past the end.
pub fn width_at(src: &str, at: usize) -> usize {
    src.get(at..)
        .and_then(|rest| rest.chars().next())
        .map_or(1, char::len_utf8)
}

/// Whitespace as `\s` sees it.
pub fn is_space(c: u32) -> bool {
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

pub fn is_blank(c: u32) -> bool {
    c == 32 || c == 9
}

/// A line terminator: what `.` does not match.
pub fn is_line_end(c: u32) -> bool {
    c == 10 || c == 13 || c == 0x2028 || c == 0x2029
}

pub fn is_digit(c: u32) -> bool {
    (48..=57).contains(&c)
}

pub fn is_letter(c: u32) -> bool {
    (65..=90).contains(&c) || (97..=122).contains(&c)
}

pub fn is_hex(c: u32) -> bool {
    (48..=57).contains(&c) || (65..=70).contains(&c) || (97..=102).contains(&c)
}

/// A character of `\w`.
pub fn is_word(c: u32) -> bool {
    (48..=57).contains(&c) || (65..=90).contains(&c) || (97..=122).contains(&c) || c == 95
}
