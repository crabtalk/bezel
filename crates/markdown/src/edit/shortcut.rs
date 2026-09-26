//! Typed markdown prefixes and inline rules.

use super::*;

/// A markdown prefix typed at the start of a block, and what it turns it into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Heading(u8),
    Bullet,
    Ordered,
    Task(bool),
    Quote,
    Code,
    Rule,
}

impl Shortcut {
    /// The block this shortcut makes, carrying whatever text was left over.
    pub fn apply(self, text: Text) -> BlockKind {
        match self {
            Self::Heading(level) => BlockKind::Heading { level, text },
            Self::Bullet => BlockKind::Bullet(text),
            Self::Ordered => BlockKind::Ordered { number: 1, text },
            Self::Task(checked) => BlockKind::Task { checked, text },
            Self::Quote => BlockKind::Quote { kind: None, text },
            // Code is literal, so whatever marks the text carried have no
            // meaning inside the fence.
            Self::Code => BlockKind::Code {
                language: None,
                code: Text::plain(text.text),
            },
            Self::Rule => BlockKind::Rule,
        }
    }
}

/// Match a markdown prefix at the start of a block, returning it and how many
/// bytes it occupied.
///
/// This is the input side of the same vocabulary [`crate::parse`] reads: typing
/// `## ` makes a heading because pasting `## ` would have. Order matters — a
/// task marker is a bullet with more on the end.
pub fn shortcut(text: &str) -> Option<(Shortcut, usize)> {
    const PREFIXES: &[(&str, Shortcut)] = &[
        ("- [ ] ", Shortcut::Task(false)),
        ("- [x] ", Shortcut::Task(true)),
        ("###### ", Shortcut::Heading(6)),
        ("##### ", Shortcut::Heading(5)),
        ("#### ", Shortcut::Heading(4)),
        ("### ", Shortcut::Heading(3)),
        ("## ", Shortcut::Heading(2)),
        ("# ", Shortcut::Heading(1)),
        ("- ", Shortcut::Bullet),
        ("* ", Shortcut::Bullet),
        ("+ ", Shortcut::Bullet),
        ("1. ", Shortcut::Ordered),
        ("> ", Shortcut::Quote),
        ("```", Shortcut::Code),
        ("---", Shortcut::Rule),
    ];
    PREFIXES
        .iter()
        .find(|(prefix, _)| text.starts_with(prefix))
        .map(|(prefix, shortcut)| (*shortcut, prefix.len()))
}

/// A closing inline delimiter just typed, and the run it closes.
///
/// The inline half of the same vocabulary [`shortcut`] covers: typing the last
/// `*` of `**bold**` makes it bold because pasting `**bold**` would have.
/// Returns the opening delimiter's range and the text between it and the caret;
/// the closing delimiter is `inner.end..caret`.
pub fn inline_rule(text: &str, caret: usize) -> Option<(Range<usize>, Range<usize>, Mark)> {
    let head = text.get(..caret)?;
    // Longest first — `**` is bold, and only what is left of it is italic.
    for (delimiter, mark) in [
        ("**", Mark::Bold),
        ("__", Mark::Bold),
        ("~~", Mark::Strike),
        ("`", Mark::Code),
        ("_", Mark::Italic),
        ("*", Mark::Italic),
    ] {
        let Some(closes) = head.strip_suffix(delimiter) else {
            continue;
        };
        let Some(open) = closes.rfind(delimiter) else {
            continue;
        };
        // `**bold*` is one keystroke from closing. Reading its second opening
        // star as italic's spends it, and the star still to come then finds no
        // `**` to close. The same holds for `__bold_`.
        if matches!(delimiter, "*" | "_") && text[..open].ends_with(delimiter) {
            continue;
        }
        let inner = open + delimiter.len()..closes.len();
        let Some(body) = text.get(inner.clone()).filter(|body| !body.is_empty()) else {
            continue;
        };
        // Emphasis cannot open or close against whitespace, so a mark reaching
        // over one has no spelling and [`Text::normalize_marks`] would shrink
        // it straight back off. A rule that fires and vanishes is worse than
        // one that does not fire.
        if body.starts_with(char::is_whitespace) || body.ends_with(char::is_whitespace) {
            continue;
        }
        // An underscore inside a word is not emphasis in CommonMark, which is
        // the only reason `snake_case_names` survive being typed.
        if delimiter.starts_with('_')
            && text[..open]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        return Some((open..open + delimiter.len(), inner, mark));
    }
    None
}
