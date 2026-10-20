//! Jison's `.*direction\s+TB[^\n]*` rules and their siblings, which a lexer
//! tries at every position of `INITIAL`.

use crate::{js::Lazy, util::chars::starts_with_caseless};

/// The direction rules of one grammar, in its order, and what earlier
/// positions found out about the text. Positions must be asked about in
/// increasing order.
pub(crate) struct Directions<K: 'static> {
    rules: &'static [(K, Lazy)],
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
    pub fn new(src: &str, rules: &'static [(K, Lazy)]) -> Self {
        Self::with_case(src, rules, false)
    }

    /// Rules for a grammar whose `direction` keyword is caseless.
    pub fn caseless(src: &str, rules: &'static [(K, Lazy)]) -> Self {
        Self::with_case(src, rules, true)
    }

    fn with_case(src: &str, rules: &'static [(K, Lazy)], caseless: bool) -> Self {
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
        for (kind, rule) in self.rules {
            if let Some(end) = rule.match_end(src, p) {
                return Some((*kind, end));
            }
        }
        self.failed = line_end;
        None
    }
}
