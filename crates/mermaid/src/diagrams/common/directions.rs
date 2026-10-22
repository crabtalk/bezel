//! Jison's `.*direction\s+TB[^\n]*` rules and their siblings, which a lexer
//! tries at every position of `INITIAL`.

use crate::util::chars::starts_with_caseless;
use d3::string;

/// The direction rules of one grammar, in its order, each a kind and the
/// direction it names after `direction`, and what earlier
/// positions found out about the text. Positions must be asked about in
/// increasing order.
pub(crate) struct Directions<K: 'static> {
    rules: &'static [(K, &'static str)],
    /// The next `direction` at or after the last position asked about.
    next: Option<usize>,
    /// The end of the line holding the last position asked about.
    line_end: Option<usize>,
    /// No rule matches before this.
    failed: usize,
    /// `direction` matches in any case of its ASCII letters.
    caseless: bool,
}

impl<K: Copy> Directions<K> {
    pub fn new(src: &str, rules: &'static [(K, &'static str)]) -> Self {
        Self::with_case(src, rules, false)
    }

    /// Rules for a grammar whose `direction` keyword is caseless.
    pub fn caseless(src: &str, rules: &'static [(K, &'static str)]) -> Self {
        Self::with_case(src, rules, true)
    }

    fn with_case(src: &str, rules: &'static [(K, &'static str)], caseless: bool) -> Self {
        let mut directions = Self {
            rules,
            next: None,
            line_end: None,
            failed: 0,
            caseless,
        };
        directions.next = directions.find(src, 0);
        directions
    }

    /// The next `direction` at or after `from`.
    fn find(&self, src: &str, from: usize) -> Option<usize> {
        if self.caseless {
            (from..src.len()).find(|&at| starts_with_caseless(src, at, "direction"))
        } else {
            src[from..].find("direction").map(|at| from + at)
        }
    }

    /// Whether `word` is at byte `at` of `src`, in its case or, when the
    /// grammar is caseless, in any.
    fn is(&self, src: &str, at: usize, word: &str) -> bool {
        let Some(here) = src.as_bytes().get(at..at + word.len()) else {
            return false;
        };
        if self.caseless {
            here.eq_ignore_ascii_case(word.as_bytes())
        } else {
            here == word.as_bytes()
        }
    }

    /// The end of `.*direction\s+<name>[^\n]*` at `p` when the last
    /// `direction` before `line_end` that `name` follows, past whitespace,
    /// starts at or after `from`.
    fn rule_end(&self, src: &str, from: usize, line_end: usize, name: &str) -> Option<usize> {
        let mut at = line_end;
        while at > from {
            at = self.find_back(src, from, at)?;
            let after = at + "direction".len();
            let named = string::space_end(src, after);
            if named > after && self.is(src, named, name) {
                let rest = named + name.len();
                return Some(
                    memchr::memchr(b'\n', &src.as_bytes()[rest..]).map_or(src.len(), |n| rest + n),
                );
            }
        }
        None
    }

    /// The last `direction` starting in `from..before`.
    fn find_back(&self, src: &str, from: usize, before: usize) -> Option<usize> {
        if self.caseless {
            (from..before)
                .rev()
                .find(|&at| self.is(src, at, "direction"))
        } else {
            src[from..before].rfind("direction").map(|at| from + at)
        }
    }

    /// The kind and end of the first rule matching at `p`.
    pub fn at(&mut self, src: &str, p: usize) -> Option<(K, usize)> {
        let mut next = self.next?;
        if p < self.failed {
            return None;
        }
        if next < p {
            self.next = self.find(src, p);
            next = self.next?;
        }
        let line_end = match self.line_end {
            Some(end) if end >= p => end,
            _ => {
                let end = src[p..]
                    .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                    .map_or(src.len(), |at| p + at);
                self.line_end = Some(end);
                end
            }
        };
        if next >= line_end {
            return None;
        }
        for &(kind, name) in self.rules {
            if let Some(end) = self.rule_end(src, next, line_end, name) {
                return Some((kind, end));
            }
        }
        self.failed = line_end;
        None
    }
}
