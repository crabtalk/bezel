//! Edits within one run of text: splicing, and marks over a range.

use super::*;

impl Text {
    /// Insert at a byte offset, moving the marks with it.
    pub fn insert(&mut self, at: usize, s: &str) {
        let at = at.min(self.text.len());
        if s.is_empty() {
            return;
        }
        let n = s.len();
        self.text.insert_str(at, s);
        for span in &mut self.marks {
            if at <= span.range.start {
                span.range.start += n;
                span.range.end += n;
            } else if at <= span.range.end {
                // Inside, or exactly at the end — the left-sticky rule.
                span.range.end += n;
            }
        }
        self.normalize_marks();
    }

    /// Remove a byte range, collapsing any mark that covered it.
    pub fn remove(&mut self, range: Range<usize>) {
        let range = self.clamp(range);
        if range.is_empty() {
            return;
        }
        self.text.replace_range(range.clone(), "");
        let shift = |offset: usize| {
            if offset <= range.start {
                offset
            } else if offset >= range.end {
                offset - range.len()
            } else {
                range.start
            }
        };
        for span in &mut self.marks {
            span.range = shift(span.range.start)..shift(span.range.end);
        }
        self.normalize_marks();
    }

    /// Add `mark` over `range`, or take it away if the whole range has it.
    pub fn toggle(&mut self, range: Range<usize>, mark: Mark) {
        let range = self.clamp(range);
        if range.is_empty() {
            return;
        }
        if self.covered_by(&range, &mark) {
            self.marks = std::mem::take(&mut self.marks)
                .into_iter()
                .flat_map(|span| subtract(span, &range, &mark))
                .collect();
        } else {
            self.marks.push(MarkSpan { range, mark });
        }
        self.normalize_marks();
    }

    /// Whether every byte of `range` already carries `mark`.
    pub fn covered_by(&self, range: &Range<usize>, mark: &Mark) -> bool {
        !range.is_empty()
            && self.marks.iter().any(|span| {
                span.mark == *mark && span.range.start <= range.start && span.range.end >= range.end
            })
    }

    /// Cut at `at`, returning the tail. The head keeps this [`Text`].
    pub fn split_off(&mut self, at: usize) -> Text {
        let at = at.min(self.text.len());
        let mut tail = Text {
            text: self.text.split_off(at),
            marks: Vec::new(),
        };
        let mut head = Vec::new();
        for span in std::mem::take(&mut self.marks) {
            if span.range.start < at {
                head.push(MarkSpan {
                    range: span.range.start..span.range.end.min(at),
                    mark: span.mark.clone(),
                });
            }
            if span.range.end > at {
                tail.marks.push(MarkSpan {
                    range: span.range.start.saturating_sub(at)..span.range.end - at,
                    mark: span.mark,
                });
            }
        }
        self.marks = head;
        self.normalize_marks();
        tail.normalize_marks();
        tail
    }

    /// Append `other`, shifting its marks onto the end of this text.
    pub fn append(&mut self, other: Text) {
        let offset = self.text.len();
        self.text.push_str(&other.text);
        self.marks
            .extend(other.marks.into_iter().map(|span| MarkSpan {
                range: span.range.start + offset..span.range.end + offset,
                mark: span.mark,
            }));
        self.normalize_marks();
    }

    pub(super) fn clamp(&self, range: Range<usize>) -> Range<usize> {
        let start = range.start.min(self.text.len());
        let end = range.end.clamp(start, self.text.len());
        start..end
    }

    /// Drop marks that cover nothing and merge ones that touch.
    ///
    /// Both matter to the round trip rather than to tidiness: an empty bold
    /// span serializes to `****`, which is literal text, and two abutting bold
    /// spans serialize to `**a****b**`, which is not one bold run.
    pub(crate) fn normalize_marks(&mut self) {
        // Emphasis cannot open or close against whitespace — `* t*` is two
        // literal asterisks, not italic — so a mark reaching over a space has
        // no spelling that survives a round trip. Shrinking it to the text it
        // can actually cover is also what a user means when a drag-selection
        // catches the trailing space.
        for ix in 0..self.marks.len() {
            if !matches!(
                self.marks[ix].mark,
                Mark::Bold | Mark::Italic | Mark::Strike | Mark::Code
            ) {
                continue;
            }
            let range = self.marks[ix].range.clone();
            if range.end > self.text.len() {
                continue;
            }
            let slice = &self.text[range.clone()];
            let start = range.start + (slice.len() - slice.trim_start().len());
            let end = (range.end - (slice.len() - slice.trim_end().len())).max(start);
            self.marks[ix].range = start..end;
        }

        let len = self.text.len();
        self.marks.retain(|span| {
            span.range.end <= len && (!span.range.is_empty() || matches!(span.mark, Mark::Image(_)))
        });

        // A code span is atomic: nothing can start or stop inside one. A mark
        // that only half covers it has no spelling, so it grows to take the
        // whole span — which is also what the markdown for it reads back as.
        let code: Vec<Range<usize>> = self
            .marks
            .iter()
            .filter(|span| span.mark == Mark::Code)
            .map(|span| span.range.clone())
            .collect();
        for span in &mut self.marks {
            if span.mark == Mark::Code {
                continue;
            }
            for range in &code {
                let crosses = span.range.start > range.start && span.range.start < range.end
                    || span.range.end > range.start && span.range.end < range.end;
                if crosses {
                    span.range.start = span.range.start.min(range.start);
                    span.range.end = span.range.end.max(range.end);
                }
            }
        }

        // Emphasis nests or it is disjoint; it cannot cross. `**a*b**c*` is
        // not bold-then-italic overlapping, it is a parse error waiting to
        // happen — so when two spans cross, the one that opened first grows to
        // contain the other. Growing rather than clipping keeps every mark the
        // user applied; only its reach changes, and only where markdown left
        // no alternative.
        for _ in 0..self.marks.len().max(1) {
            let mut crossed = false;
            for a in 0..self.marks.len() {
                for b in 0..self.marks.len() {
                    let (first, second) = (&self.marks[a].range, &self.marks[b].range);
                    if second.start > first.start
                        && second.start < first.end
                        && second.end > first.end
                    {
                        let end = second.end;
                        self.marks[a].range.end = end;
                        crossed = true;
                    }
                }
            }
            if !crossed {
                break;
            }
        }

        // Two marks that end at the same offset close as two delimiter runs
        // back to back — `**b**~~`. CommonMark will not let the outer one close
        // there if a letter follows: a run preceded by punctuation has to be
        // followed by whitespace or punctuation to be right-flanking, so
        // `~~a **b**~~c` cannot be written at all. Nudging the outer end past
        // the word separates the two runs and it can.
        for _ in 0..self.marks.len().max(1) {
            let mut nudged = false;
            for a in 0..self.marks.len() {
                let end = self.marks[a].range.end;
                let followed_by_word = self.text[end..]
                    .chars()
                    .next()
                    .is_some_and(char::is_alphanumeric);
                let shared = self.marks.iter().enumerate().any(|(b, other)| {
                    b != a
                        && other.range.end == end
                        && other.range.start > self.marks[a].range.start
                });
                if followed_by_word && shared {
                    let extra = self.text[end..]
                        .find(|c: char| !c.is_alphanumeric())
                        .unwrap_or(self.text.len() - end);
                    self.marks[a].range.end = end + extra;
                    nudged = true;
                }
            }
            if !nudged {
                break;
            }
        }

        let mut ix = 0;
        while ix < self.marks.len() {
            let mut merged = None;
            for other in ix + 1..self.marks.len() {
                let (a, b) = (&self.marks[ix], &self.marks[other]);
                if a.mark == b.mark
                    && a.range.start <= b.range.end
                    && b.range.start <= a.range.end
                    && !matches!(a.mark, Mark::Image(_) | Mark::Mention { .. })
                {
                    merged = Some((
                        other,
                        a.range.start.min(b.range.start),
                        a.range.end.max(b.range.end),
                    ));
                    break;
                }
            }
            match merged {
                Some((other, start, end)) => {
                    self.marks[ix].range = start..end;
                    self.marks.remove(other);
                }
                None => ix += 1,
            }
        }

        // Document order, outermost first — the order a parse produces, so an
        // edited document compares equal to the same document read from disk.
        // The sort is stable, which is what keeps `**_x_**` and `_**x**_`
        // apart: their spans are identical and only their order differs.
        self.marks.sort_by(|a, b| {
            a.range
                .start
                .cmp(&b.range.start)
                .then(b.range.end.cmp(&a.range.end))
        });
    }
}

/// `span` minus `range`, when they share a mark — zero, one or two pieces.
fn subtract(span: MarkSpan, range: &Range<usize>, mark: &Mark) -> Vec<MarkSpan> {
    if span.mark != *mark || span.range.end <= range.start || span.range.start >= range.end {
        return vec![span];
    }
    let mut out = Vec::new();
    if span.range.start < range.start {
        out.push(MarkSpan {
            range: span.range.start..range.start,
            mark: span.mark.clone(),
        });
    }
    if span.range.end > range.end {
        out.push(MarkSpan {
            range: range.end..span.range.end,
            mark: span.mark,
        });
    }
    out
}
