//! The lexer Mermaid's journey and timeline grammars share, ported from the
//! compiled Jison rules: one token set, comments, whitespace, line breaks,
//! and the `accTitle` and `accDescr` statements. Pele's
//! `diagrams/common/scan.ts`.

use crate::{
    token::{Cursor, Token, grammar},
    util::chars::is_space,
};

grammar! {
    /// The journey and timeline lexers' token kinds.
    pub enum T {
        End = "$end",
        /// Never lexed: every text matches some rule.
        Error = "INVALID",
        Eof = "EOF",
        Invalid = "INVALID",
        Newline = "NEWLINE",
        Journey = "journey",
        Timeline = "timeline",
        TimelineLr = "timeline_lr",
        TimelineTd = "timeline_td",
        Title = "title",
        AccTitle = "acc_title",
        AccTitleValue = "acc_title_value",
        AccDescr = "acc_descr",
        AccDescrValue = "acc_descr_value",
        AccDescrMultilineValue = "acc_descr_multiline_value",
        Section = "section",
        TaskName = "taskName",
        TaskData = "taskData",
        Colon = ":",
        Period = "period",
        Event = "event",
    }

    fn got(self, _text, _previous) {
        match self {
            Self::End => "1",
            _ => self.name(),
        }
    }
}

pub struct Lexer<'s> {
    pub cur: Cursor<'s, T, ()>,
}

impl<'s> Lexer<'s> {
    pub fn new(src: &'s str) -> Self {
        Self {
            cur: Cursor::new(src, ()),
        }
    }

    /// Skips comments and whitespace, and answers whether a token follows.
    /// Stops at a line break that starts a token, which is a NEWLINE. Jison
    /// compiles `\%%` to a single `%`, so one percent sign starts a comment,
    /// and the second comment rule takes any character before `%%` with it,
    /// a line break included.
    pub fn skip_trivia(&mut self) -> bool {
        let cur = &mut self.cur;
        while let Some(c) = cur.char_at(cur.p) {
            let next = cur.p + c.len_utf8();
            if c == '%' && cur.char_at(next) != Some('{') {
                cur.p = cur.line_end(cur.p);
            } else if c != '}' && cur.starts_with("%%", next) {
                cur.p = cur.line_end(next + 2);
            } else if c == '\n' {
                return true;
            } else if is_space(c) {
                cur.p = cur.space_end(next);
            } else if c == '#' {
                cur.p = cur.line_end(cur.p);
            } else {
                return true;
            }
        }
        cur.p < cur.src.len()
    }

    /// A NEWLINE token of the run of line feeds at the cursor.
    pub fn newline(&mut self) -> bool {
        let p = self.cur.p;
        let end = self.cur.run_end(p, |c| c == '\n');
        end > p && {
            self.cur.emit(T::Newline, p, end);
            true
        }
    }

    /// An `accTitle` or `accDescr` statement at the cursor.
    pub fn accessibility(&mut self) -> bool {
        let cur = &mut self.cur;
        let p = cur.p;
        let descr = cur.starts_with_caseless("accdescr", p);
        if !descr && !cur.starts_with_caseless("acctitle", p) {
            return false;
        }
        let mut q = cur.space_end(p + 8);
        match cur.char_at(q) {
            Some(':') => {
                q = cur.space_end(q + 1);
                let end = cur.line_end(q);
                let (keyword, value) = if descr {
                    (T::AccDescr, T::AccDescrValue)
                } else {
                    (T::AccTitle, T::AccTitleValue)
                };
                cur.emit(keyword, p, q);
                cur.emit(value, q, end);
            }
            Some('{') if descr => {
                // An unclosed description gives its text, then an empty
                // value at the end.
                let n = cur.src.len();
                q = cur.space_end(q + 1);
                while cur.char_at(q) != Some('}') {
                    let end = cur.src[q..].find('}').map_or(n, |at| q + at);
                    cur.emit(T::AccDescrMultilineValue, q, end);
                    if q == n {
                        return true;
                    }
                    q = end;
                }
                cur.p = q + 1;
            }
            _ => return false,
        }
        true
    }

    /// The tokens, ended. Jison marks its lexer done when a rule runs on
    /// empty input, so an accessibility value that matches nothing at the
    /// very end of the text is followed by no EOF token.
    pub fn finish(mut self) -> Vec<Token<'s, T>> {
        let n = self.cur.src.len();
        if self.cur.tokens.last().is_none_or(|last| last.start < n) {
            self.cur.emit(T::Eof, n, n);
        }
        self.cur.emit(T::End, n, n);
        self.cur.tokens
    }
}
