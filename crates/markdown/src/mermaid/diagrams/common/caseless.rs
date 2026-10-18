//! Pele's `diagrams/common/caseless.ts`.

/// Whether `word`, written in lower case, is at `at`. Only ASCII letters
/// match in either case, as with the `i` flag without `u`.
pub fn word_at(src: &str, at: usize, word: &str) -> bool {
    let bytes = src.as_bytes();
    word.bytes().enumerate().all(|(k, w)| {
        bytes
            .get(at + k)
            .is_some_and(|&c| c == w || (w.is_ascii_lowercase() && c == w - 32))
    })
}
