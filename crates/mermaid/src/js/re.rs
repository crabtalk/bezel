//! Regular expressions with JavaScript's syntax and matching rules, for the
//! patterns ported from Pele.
//!
//! A backtracking matcher: the pattern compiled to a program, run as a loop
//! with an explicit stack. It reads what Pele's patterns use: classes,
//! `\d \w \s \b`, `\p{L}` and `\p{N}` under `u`, groups, alternation,
//! greedy and lazy quantifiers, lookahead, and the flags `g i m s u y`. `{`
//! that does not open a quantifier, and `]` and `}` outside a class, are
//! literal, as in JavaScript without `u`. No backreferences and no lookbehind.
//!
//! Positions are byte offsets into UTF-8, and `.` takes a code point where
//! JavaScript takes a UTF-16 unit.

use std::fmt::Write as _;

use crate::util::chars::{self, is_line_end, is_space, is_word};
use std::{
    ops::{Deref, Range},
    sync::OnceLock,
};

/// Group positions kept on the stack, for up to seven groups.
const STACK_CAPS: usize = 16;

/// A pattern compiled on first use, for a `static`.
pub(crate) struct Lazy {
    pattern: &'static str,
    flags: &'static str,
    cell: OnceLock<Regex>,
}

impl Lazy {
    pub(crate) const fn new(pattern: &'static str, flags: &'static str) -> Self {
        Self {
            pattern,
            flags,
            cell: OnceLock::new(),
        }
    }
}

impl Deref for Lazy {
    type Target = Regex;

    fn deref(&self) -> &Regex {
        self.cell
            .get_or_init(|| Regex::new(self.pattern, self.flags))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Regex {
    prog: Vec<Inst>,
    /// How many unbounded-node repeats the program counts.
    repeats: usize,
    groups: usize,
    flags: Flags,
    /// The characters a match can start with, when that is known; at 0 it
    /// also starts any `^` alternative.
    first: Option<Set>,
    /// Some alternative opens with `^` and the pattern is not multiline.
    at_zero: bool,
    /// What each such alternative needs at 0, when that is known for all.
    leads: Option<Vec<Lead>>,
    /// The whole pattern is one character, class or `.` repeated greedily
    /// without bound, at least this many times.
    run: Option<(Node, usize)>,
    /// Every alternative opens with `^` and the pattern is not multiline, so
    /// a match can only start at 0.
    anchored: bool,
}

/// An alternative opening `^X*Y`, where `X` is a class `Y` cannot start
/// with: the ASCII characters `X*` steps over, and the set `Y` starts with.
#[derive(Clone, Copy, Debug)]
struct Lead {
    skip: [u64; 2],
    first: Set,
}

impl Lead {
    fn of(seq: &[Node], ignore_case: bool) -> Option<Self> {
        if let Some(Node::Repeat {
            node,
            min: 0,
            max: usize::MAX,
            ..
        }) = seq.first()
            && let Node::Class(class) = &**node
        {
            let first = first_of_seq(&seq[1..], ignore_case)?;
            let disjoint = (0..2).all(|i| first.ascii[i] & class.ascii[i] == 0);
            return disjoint.then_some(Self {
                skip: class.ascii,
                first,
            });
        }
        Some(Self {
            skip: [0; 2],
            first: first_of_seq(seq, ignore_case)?,
        })
    }

    /// Whether a match can start at the start of `text`. A character outside
    /// ASCII in the run `X*` steps over passes.
    fn may_start(&self, text: &[u8]) -> bool {
        let mut at = 0;
        while let Some(&b) = text.get(at) {
            if b >= 128 {
                return true;
            }
            if self.skip[usize::from(b >> 6)] & (1 << (b & 63)) == 0 {
                return self.first.ascii[usize::from(b >> 6)] & (1 << (b & 63)) != 0;
            }
            at += 1;
        }
        false
    }
}

/// A set of characters: ASCII in a bitmap, anything else as one flag.
#[derive(Clone, Copy, Debug, Default)]
struct Set {
    ascii: [u64; 2],
    other: bool,
}

impl Set {
    fn insert(&mut self, c: u8) {
        self.ascii[usize::from(c >> 6)] |= 1 << (c & 63);
    }

    fn has(&self, c: char) -> bool {
        if c.is_ascii() {
            let b = c as u8;
            self.ascii[usize::from(b >> 6)] & (1 << (b & 63)) != 0
        } else {
            self.other
        }
    }

    fn union(&mut self, other: Set) {
        self.ascii[0] |= other.ascii[0];
        self.ascii[1] |= other.ascii[1];
        self.other |= other.other;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Flags {
    global: bool,
    ignore_case: bool,
    multiline: bool,
    dot_all: bool,
    unicode: bool,
    sticky: bool,
}

type Alt = Vec<Vec<Node>>;

#[derive(Clone, Debug)]
enum Node {
    Char(char),
    Any,
    Class(Box<Class>),
    Start,
    End,
    Boundary(bool),
    Group(Box<Alt>, Option<usize>),
    /// The lookahead, whether it is negative, and whether it holds a
    /// capturing group.
    Look(Box<Alt>, bool, bool),
    Repeat {
        node: Box<Node>,
        min: usize,
        max: usize,
        greedy: bool,
    },
}

#[derive(Clone, Debug, Default)]
struct Class {
    negated: bool,
    items: Vec<Item>,
    /// Which ASCII characters match, case and negation applied.
    ascii: [u64; 2],
}

#[derive(Clone, Copy, Debug)]
enum Item {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
    Letter(bool),
    Number(bool),
}

/// What one match found: the whole match and each group, as byte ranges.
#[derive(Clone, Debug)]
pub(crate) struct Captures<'t> {
    text: &'t str,
    spans: Vec<Option<Range<usize>>>,
}

impl<'t> Captures<'t> {
    /// Group `index`'s text, `0` the whole match; `None` where it took no part.
    pub(crate) fn get(&self, index: usize) -> Option<&'t str> {
        self.spans
            .get(index)?
            .clone()
            .map(|range| &self.text[range])
    }

    /// Group `index`'s text, or `""` where it took no part.
    pub(crate) fn str(&self, index: usize) -> &'t str {
        self.get(index).unwrap_or("")
    }

    pub(crate) fn start(&self) -> usize {
        self.spans[0].as_ref().map_or(0, |range| range.start)
    }

    pub(crate) fn end(&self) -> usize {
        self.spans[0].as_ref().map_or(0, |range| range.end)
    }

    pub(crate) fn len(&self) -> usize {
        self.spans.len()
    }
}

impl Regex {
    /// Compiles `pattern` under `flags`. A pattern that does not parse is a
    /// bug in the port, so it panics.
    pub(crate) fn new(pattern: &str, flags: &str) -> Self {
        let mut parsed = Flags::default();
        for flag in flags.chars() {
            match flag {
                'g' => parsed.global = true,
                'i' => parsed.ignore_case = true,
                'm' => parsed.multiline = true,
                's' => parsed.dot_all = true,
                'u' => parsed.unicode = true,
                'y' => parsed.sticky = true,
                'd' => {}
                _ => panic!("regex flag {flag:?} in /{pattern}/"),
            }
        }
        let mut parser = Parser {
            chars: pattern.chars().collect(),
            at: 0,
            groups: 0,
            unicode: parsed.unicode,
        };
        let alt = parser.alt();
        assert!(
            parser.at == parser.chars.len(),
            "regex /{pattern}/ does not parse at {}",
            parser.at
        );
        let mut alt = alt;
        prepare(&mut alt, parsed.ignore_case);
        let anchored = !parsed.multiline
            && alt
                .iter()
                .all(|seq| matches!(seq.first(), Some(Node::Start)));
        let opens_at_zero = |seq: &Vec<Node>| matches!(seq.first(), Some(Node::Start));
        let at_zero = !parsed.multiline && alt.iter().any(opens_at_zero);
        let rest: Alt = if at_zero {
            alt.iter()
                .filter(|seq| !opens_at_zero(seq))
                .cloned()
                .collect()
        } else {
            alt.clone()
        };
        let first = first_of_alt(&rest, parsed.ignore_case);
        let leads = at_zero
            .then(|| {
                alt.iter()
                    .filter(|seq| opens_at_zero(seq))
                    .map(|seq| Lead::of(&seq[1..], parsed.ignore_case))
                    .collect()
            })
            .flatten();
        let run = match alt.as_slice() {
            [seq] => match seq.as_slice() {
                [
                    Node::Repeat {
                        node,
                        min,
                        max: usize::MAX,
                        greedy: true,
                    },
                ] if matches!(**node, Node::Char(_) | Node::Any | Node::Class(_)) => {
                    Some(((**node).clone(), *min))
                }
                _ => None,
            },
            _ => None,
        };
        let mut prog = Vec::new();
        let mut repeats = 0;
        compile_alt(&alt, &mut prog, &mut repeats);
        prog.push(Inst::Match);
        Self {
            prog,
            repeats,
            groups: parser.groups,
            flags: parsed,
            first,
            at_zero,
            leads,
            run,
            anchored,
        }
    }

    /// Whether the pattern matches anywhere in `text`, as `test` does on a
    /// pattern without `g` or `y`; a sticky pattern only at the start.
    pub(crate) fn test(&self, text: &str) -> bool {
        self.exec_at(text, 0).is_some()
    }

    /// The first match at or after `from`, or only at `from` for a sticky
    /// pattern: `exec` with `lastIndex = from`.
    pub(crate) fn exec_at<'t>(&self, text: &'t str, from: usize) -> Option<Captures<'t>> {
        if from > text.len() {
            return None;
        }
        if self.flags.sticky {
            return self.match_here(text, from);
        }
        if self.anchored {
            return if from == 0 {
                self.match_here(text, 0)
            } else {
                None
            };
        }
        let mut stack = [None; STACK_CAPS];
        let mut heap = Vec::new();
        let caps = self.caps_buffer(&mut stack, &mut heap);
        let bytes = text.as_bytes();
        let mut at = from;
        loop {
            if at == 0 && self.at_zero {
                if self.may_start(text, 0)
                    && let Some(end) = self.try_at(text, 0, caps)
                {
                    return Some(self.captures(text, 0, end, caps));
                }
                if text.is_empty() {
                    return None;
                }
                at = char_len(text, 0);
                continue;
            }
            if let Some(first) = &self.first {
                // Only a character the set holds can start a match: step
                // over the rest a byte at a time, never stopping inside one.
                while let Some(&b) = bytes.get(at) {
                    let start = if b < 128 {
                        first.ascii[usize::from(b >> 6)] & (1 << (b & 63)) != 0
                    } else {
                        b >= 0xc0 && first.other
                    };
                    if start {
                        break;
                    }
                    at += 1;
                }
                if at >= bytes.len() {
                    return None;
                }
            }
            if let Some(end) = self.try_at(text, at, caps) {
                return Some(self.captures(text, at, end, caps));
            }
            if at >= text.len() {
                return None;
            }
            at += char_len(text, at);
        }
    }

    /// The first match in `text`.
    pub(crate) fn exec<'t>(&self, text: &'t str) -> Option<Captures<'t>> {
        self.exec_at(text, 0)
    }

    /// Where a match starting exactly at `at` ends.
    pub(crate) fn match_end(&self, text: &str, at: usize) -> Option<usize> {
        if !self.may_start(text, at) {
            return None;
        }
        let mut stack = [None; STACK_CAPS];
        let mut heap = Vec::new();
        let caps = self.caps_buffer(&mut stack, &mut heap);
        self.try_at(text, at, caps)
    }

    /// Whether a match can start at `at`, by the characters it can start
    /// with.
    fn may_start(&self, text: &str, at: usize) -> bool {
        let next = || text[at..].chars().next();
        let unanchored = self
            .first
            .is_none_or(|first| next().is_some_and(|c| first.has(c)));
        if at != 0 || !self.at_zero {
            return unanchored;
        }
        unanchored
            || self
                .leads
                .as_ref()
                .is_none_or(|leads| leads.iter().any(|lead| lead.may_start(text.as_bytes())))
    }

    /// Room for the group positions: on the stack when they fit.
    fn caps_buffer<'b>(
        &self,
        stack: &'b mut [Option<usize>; STACK_CAPS],
        heap: &'b mut Vec<Option<usize>>,
    ) -> &'b mut Caps {
        let len = (self.groups + 1) * 2;
        if len <= STACK_CAPS {
            &mut stack[..len]
        } else {
            *heap = vec![None; len];
            heap
        }
    }

    /// The match starting exactly at `at`.
    pub(crate) fn match_here<'t>(&self, text: &'t str, at: usize) -> Option<Captures<'t>> {
        if !self.may_start(text, at) {
            return None;
        }
        let mut stack = [None; STACK_CAPS];
        let mut heap = Vec::new();
        let caps = self.caps_buffer(&mut stack, &mut heap);
        let end = self.try_at(text, at, caps)?;
        Some(self.captures(text, at, end, caps))
    }

    /// Where a match starting exactly at `at` ends, its groups left in
    /// `caps`, cleared first.
    fn try_at(&self, text: &str, at: usize, caps: &mut Caps) -> Option<usize> {
        caps.fill(None);
        let matcher = Matcher {
            text,
            flags: self.flags,
        };
        if let Some((node, min)) = &self.run {
            let (mut end, mut count) = (at, 0);
            while let Some(next) = matcher.single(node, end) {
                end = next;
                count += 1;
            }
            return (count >= *min).then_some(end);
        }
        STATE.with(|cell| {
            let mut state = cell.take();
            state.regs.clear();
            state.regs.resize(self.repeats, (0, 0));
            let end = matcher.run(&self.prog, 0, at, caps, &mut state);
            state.stack.clear();
            state.saved.clear();
            cell.set(state);
            end
        })
    }

    fn captures<'t>(&self, text: &'t str, at: usize, end: usize, caps: &Caps) -> Captures<'t> {
        let mut spans = Vec::with_capacity(self.groups + 1);
        spans.push(Some(at..end));
        for group in 1..=self.groups {
            spans.push(match (caps[group * 2], caps[group * 2 + 1]) {
                (Some(start), Some(end)) => Some(start..end),
                _ => None,
            });
        }
        Captures { text, spans }
    }

    /// Every match from the start, as a global pattern's `exec` loop finds
    /// them: an empty match steps one character on.
    pub(crate) fn captures_iter<'r, 't>(&'r self, text: &'t str) -> CapturesIter<'r, 't> {
        CapturesIter {
            regex: self,
            text,
            at: 0,
            done: false,
        }
    }

    /// `text.replace(re, f)`: every match when the pattern is global, else the
    /// first.
    pub(crate) fn replace(&self, text: &str, mut with: impl FnMut(&Captures) -> String) -> String {
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        let mut any = false;
        for caps in self.captures_iter(text) {
            any = true;
            out.push_str(&text[last..caps.start()]);
            let _ = write!(out, "{}", with(&caps));
            last = caps.end();
            if !self.flags.global {
                break;
            }
        }
        if !any {
            return text.to_string();
        }
        out.push_str(&text[last..]);
        out
    }

    /// `text.split(re)`: groups are spliced in between the pieces.
    pub(crate) fn split<'t>(&self, text: &'t str) -> Vec<Option<&'t str>> {
        if text.is_empty() {
            return match self.match_here(text, 0) {
                Some(_) => Vec::new(),
                None => vec![Some(text)],
            };
        }
        let mut out = Vec::new();
        let (mut p, mut q) = (0, 0);
        while q < text.len() {
            match self.match_here(text, q) {
                Some(caps) if caps.end() != p => {
                    out.push(Some(&text[p..q]));
                    for group in 1..caps.len() {
                        out.push(caps.get(group));
                    }
                    p = caps.end();
                    q = p;
                }
                _ => q += char_len(text, q),
            }
        }
        out.push(Some(&text[p..]));
        out
    }

    /// [`Self::split`] with a missing group as `""`.
    pub(crate) fn split_str<'t>(&self, text: &'t str) -> Vec<&'t str> {
        self.split(text)
            .into_iter()
            .map(|piece| piece.unwrap_or(""))
            .collect()
    }

    /// `text.match(re)` on a global pattern: every match's text.
    pub(crate) fn find_all<'t>(&self, text: &'t str) -> Vec<&'t str> {
        self.captures_iter(text)
            .map(|caps| caps.get(0).unwrap_or(""))
            .collect()
    }
}

pub(crate) struct CapturesIter<'r, 't> {
    regex: &'r Regex,
    text: &'t str,
    at: usize,
    done: bool,
}

impl<'t> Iterator for CapturesIter<'_, 't> {
    type Item = Captures<'t>;

    fn next(&mut self) -> Option<Captures<'t>> {
        if self.done {
            return None;
        }
        let Some(caps) = self.regex.exec_at(self.text, self.at) else {
            self.done = true;
            return None;
        };
        self.at = if caps.end() == caps.start() {
            if caps.end() >= self.text.len() {
                self.done = true;
            }
            caps.end() + char_len(self.text, caps.end())
        } else {
            caps.end()
        };
        Some(caps)
    }
}

fn char_len(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(1, char::len_utf8)
}

struct Parser {
    chars: Vec<char>,
    at: usize,
    groups: usize,
    unicode: bool,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn alt(&mut self) -> Alt {
        let mut alt = vec![self.seq()];
        while self.eat('|') {
            alt.push(self.seq());
        }
        alt
    }

    fn seq(&mut self) -> Vec<Node> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom();
            let atom = self.quantified(atom);
            seq.push(atom);
        }
        seq
    }

    fn atom(&mut self) -> Node {
        let c = self.peek().expect("an atom");
        self.at += 1;
        match c {
            '.' => Node::Any,
            '^' => Node::Start,
            '$' => Node::End,
            '(' => {
                if self.eat('?') {
                    if self.eat(':') {
                        let alt = self.alt();
                        assert!(self.eat(')'), "unclosed group");
                        return Node::Group(Box::new(alt), None);
                    }
                    let negative = if self.eat('=') {
                        false
                    } else if self.eat('!') {
                        true
                    } else {
                        panic!("unsupported group at {}", self.at)
                    };
                    let alt = self.alt();
                    assert!(self.eat(')'), "unclosed lookahead");
                    let groups = captures(&alt);
                    return Node::Look(Box::new(alt), negative, groups);
                }
                self.groups += 1;
                let index = self.groups;
                let alt = self.alt();
                assert!(self.eat(')'), "unclosed group");
                Node::Group(Box::new(alt), Some(index))
            }
            '[' => Node::Class(Box::new(self.class())),
            '\\' => self.escape(false).node(),
            _ => Node::Char(c),
        }
    }

    fn quantified(&mut self, atom: Node) -> Node {
        let start = self.at;
        let (min, max) = match self.peek() {
            Some('*') => {
                self.at += 1;
                (0, usize::MAX)
            }
            Some('+') => {
                self.at += 1;
                (1, usize::MAX)
            }
            Some('?') => {
                self.at += 1;
                (0, 1)
            }
            Some('{') => match self.braces() {
                Some(range) => range,
                None => {
                    self.at = start;
                    return atom;
                }
            },
            _ => return atom,
        };
        let greedy = !self.eat('?');
        Node::Repeat {
            node: Box::new(atom),
            min,
            max,
            greedy,
        }
    }

    /// `{n}`, `{n,}` or `{n,m}`; anything else leaves `{` a literal.
    fn braces(&mut self) -> Option<(usize, usize)> {
        let start = self.at;
        self.at += 1;
        let number = |p: &mut Self| -> Option<usize> {
            let from = p.at;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.at += 1;
            }
            (p.at > from).then(|| {
                p.chars[from..p.at]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(usize::MAX)
            })
        };
        let parsed = (|| {
            let min = number(self)?;
            let max = if self.eat(',') {
                number(self).unwrap_or(usize::MAX)
            } else {
                min
            };
            self.eat('}').then_some((min, max))
        })();
        if parsed.is_none() {
            self.at = start;
        }
        parsed
    }

    fn class(&mut self) -> Class {
        let mut class = Class {
            negated: self.eat('^'),
            items: Vec::new(),
            ascii: [0; 2],
        };
        loop {
            let c = self.peek().expect("unclosed class");
            // `[]` matches nothing and `[^]` anything, as in JavaScript.
            if c == ']' {
                self.at += 1;
                break;
            }
            let low = self.class_atom();
            if self.peek() == Some('-')
                && self.chars.get(self.at + 1).is_some_and(|&c| c != ']')
                && let Escape::Char(lo) = low
            {
                self.at += 1;
                match self.class_atom() {
                    Escape::Char(hi) => class.items.push(Item::Range(lo, hi)),
                    other => {
                        class.items.push(Item::Range(lo, lo));
                        class.items.push(Item::Range('-', '-'));
                        other.push(&mut class.items);
                    }
                }
                continue;
            }
            low.push(&mut class.items);
        }
        class
    }

    fn class_atom(&mut self) -> Escape {
        let c = self.peek().expect("class atom");
        self.at += 1;
        if c == '\\' {
            self.escape(true)
        } else {
            Escape::Char(c)
        }
    }

    fn escape(&mut self, in_class: bool) -> Escape {
        let c = self.peek().expect("escape");
        self.at += 1;
        match c {
            'd' => Escape::Item(Item::Digit(false)),
            'D' => Escape::Item(Item::Digit(true)),
            'w' => Escape::Item(Item::Word(false)),
            'W' => Escape::Item(Item::Word(true)),
            's' => Escape::Item(Item::Space(false)),
            'S' => Escape::Item(Item::Space(true)),
            'b' if in_class => Escape::Char('\u{8}'),
            'b' => Escape::Boundary(true),
            'B' => Escape::Boundary(false),
            'n' => Escape::Char('\n'),
            'r' => Escape::Char('\r'),
            't' => Escape::Char('\t'),
            'v' => Escape::Char('\u{b}'),
            'f' => Escape::Char('\u{c}'),
            '0' => Escape::Char('\0'),
            'x' => {
                let code = self.hex(2);
                Escape::Char(char::from_u32(code).unwrap_or('\u{fffd}'))
            }
            'u' => {
                if self.unicode && self.eat('{') {
                    let mut code = 0;
                    while let Some(d) = self.peek().and_then(|c| c.to_digit(16)) {
                        code = code * 16 + d;
                        self.at += 1;
                    }
                    assert!(self.eat('}'), "unclosed \\u{{");
                    return Escape::Char(char::from_u32(code).unwrap_or('\u{fffd}'));
                }
                let code = self.hex(4);
                Escape::Char(char::from_u32(code).unwrap_or('\u{fffd}'))
            }
            'p' | 'P' if self.unicode => {
                assert!(self.eat('{'), "\\p without a property");
                let from = self.at;
                while self.peek().is_some_and(|c| c != '}') {
                    self.at += 1;
                }
                let name: String = self.chars[from..self.at].iter().collect();
                self.at += 1;
                let negated = c == 'P';
                match name.as_str() {
                    "L" | "Letter" => Escape::Item(Item::Letter(negated)),
                    "N" | "Number" => Escape::Item(Item::Number(negated)),
                    _ => panic!("unsupported property \\p{{{name}}}"),
                }
            }
            other => Escape::Char(other),
        }
    }

    fn hex(&mut self, digits: usize) -> u32 {
        let mut code = 0;
        for _ in 0..digits {
            let d = self.peek().and_then(|c| c.to_digit(16)).expect("hex digit");
            code = code * 16 + d;
            self.at += 1;
        }
        code
    }
}

enum Escape {
    Char(char),
    Item(Item),
    Boundary(bool),
}

impl Escape {
    fn node(self) -> Node {
        match self {
            Self::Char(c) => Node::Char(c),
            Self::Item(item) => Node::Class(Box::new(Class {
                negated: false,
                items: vec![item],
                ascii: [0; 2],
            })),
            Self::Boundary(at) => Node::Boundary(at),
        }
    }

    fn push(self, items: &mut Vec<Item>) {
        match self {
            Self::Char(c) => items.push(Item::Range(c, c)),
            Self::Item(item) => items.push(item),
            Self::Boundary(_) => items.push(Item::Range('\u{8}', '\u{8}')),
        }
    }
}

impl Item {
    fn matches(self, c: char) -> bool {
        match self {
            Self::Range(lo, hi) => lo <= c && c <= hi,
            Self::Digit(negated) => c.is_ascii_digit() != negated,
            Self::Word(negated) => is_word(c) != negated,
            Self::Space(negated) => is_space(c) != negated,
            Self::Letter(negated) => c.is_alphabetic() != negated,
            Self::Number(negated) => c.is_numeric() != negated,
        }
    }
}

impl Class {
    fn matches(&self, c: char, ignore_case: bool) -> bool {
        let hit = |c: char| self.items.iter().any(|item| item.matches(c));
        let found = hit(c)
            || (ignore_case && {
                let lower = fold(c);
                lower != c && hit(lower) || {
                    let upper = upper(c);
                    upper != c && hit(upper)
                }
            });
        found != self.negated
    }
}

fn fold(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

fn upper(c: char) -> char {
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(one), None) => one,
        _ => c,
    }
}

struct Matcher<'t> {
    text: &'t str,
    flags: Flags,
}

type Caps = [Option<usize>];

impl Matcher<'_> {
    fn char_at(&self, at: usize) -> Option<char> {
        chars::char_at(self.text, at)
    }

    fn char_before(&self, at: usize) -> Option<char> {
        self.text.get(..at)?.chars().next_back()
    }

    /// Where `node`, matching one character, ends when it matches at `at`.
    fn single(&self, node: &Node, at: usize) -> Option<usize> {
        let byte = *self.text.as_bytes().get(at)?;
        if byte < 128 {
            let c = byte as char;
            let hit = match node {
                Node::Char(want) => {
                    c == *want || (self.flags.ignore_case && c.eq_ignore_ascii_case(want))
                }
                Node::Any => self.flags.dot_all || !is_line_end(c),
                Node::Class(class) => class.ascii[usize::from(byte >> 6)] & (1 << (byte & 63)) != 0,
                _ => unreachable!(),
            };
            return hit.then_some(at + 1);
        }
        let c = self.char_at(at)?;
        let hit = match node {
            Node::Char(want) => c == *want || (self.flags.ignore_case && fold(c) == fold(*want)),
            Node::Any => self.flags.dot_all || !is_line_end(c),
            Node::Class(class) => class.matches(c, self.flags.ignore_case),
            _ => unreachable!(),
        };
        hit.then(|| at + c.len_utf8())
    }

    /// Runs `prog` from `pc` at `at`: where the match ends, or `None`. What
    /// it pushes on `state` past the height it started at is gone after.
    fn run(
        &self,
        prog: &[Inst],
        mut pc: usize,
        mut at: usize,
        caps: &mut Caps,
        state: &mut State,
    ) -> Option<usize> {
        let base = state.stack.len();
        loop {
            let ok = match &prog[pc] {
                Inst::One(node) => match self.single(node, at) {
                    Some(end) => {
                        at = end;
                        pc += 1;
                        true
                    }
                    None => false,
                },
                Inst::Start => {
                    pc += 1;
                    at == 0
                        || (self.flags.multiline && self.char_before(at).is_some_and(is_line_end))
                }
                Inst::End => {
                    pc += 1;
                    at == self.text.len()
                        || (self.flags.multiline && self.char_at(at).is_some_and(is_line_end))
                }
                Inst::Boundary(want) => {
                    pc += 1;
                    let before = self.char_before(at).is_some_and(is_word);
                    let after = self.char_at(at).is_some_and(is_word);
                    (before != after) == *want
                }
                Inst::Split(first, then) => {
                    state.stack.push(Frame::Branch { pc: *then, at });
                    pc = *first;
                    true
                }
                Inst::Jump(to) => {
                    pc = *to;
                    true
                }
                Inst::Save(slot) => {
                    state.stack.push(Frame::Cap {
                        slot: *slot,
                        old: caps[*slot],
                    });
                    caps[*slot] = Some(at);
                    pc += 1;
                    true
                }
                Inst::Look {
                    next,
                    negative,
                    captures: false,
                } => {
                    let found = self.run(prog, pc + 1, at, caps, state).is_some();
                    pc = *next;
                    found != *negative
                }
                Inst::Look {
                    next,
                    negative,
                    captures: true,
                } => {
                    let mut inner = caps.to_vec();
                    let found = self.run(prog, pc + 1, at, &mut inner, state).is_some();
                    pc = *next;
                    if found && !*negative {
                        state.stack.push(Frame::Caps {
                            from: state.saved.len(),
                        });
                        state.saved.extend_from_slice(caps);
                        caps.copy_from_slice(&inner);
                    }
                    found != *negative
                }
                Inst::RepeatInit(id) => {
                    set_reg(state, *id, (0, at));
                    pc += 1;
                    true
                }
                Inst::Repeat {
                    id,
                    exit,
                    min,
                    max,
                    greedy,
                } => {
                    let (count, _) = state.regs[*id];
                    if count < *min {
                        set_reg(state, *id, (count, at));
                        pc += 1;
                        true
                    } else if count >= *max {
                        pc = *exit;
                        true
                    } else if *greedy {
                        state.stack.push(Frame::Branch { pc: *exit, at });
                        set_reg(state, *id, (count, at));
                        pc += 1;
                        true
                    } else {
                        state.stack.push(Frame::Enter {
                            pc: pc + 1,
                            at,
                            id: *id,
                        });
                        pc = *exit;
                        true
                    }
                }
                Inst::RepeatNext { id, min, back } => {
                    let (count, start) = state.regs[*id];
                    // An iteration that takes nothing past the minimum ends
                    // the loop.
                    if at == start && count >= *min {
                        false
                    } else {
                        set_reg(state, *id, (count + 1, start));
                        pc = *back;
                        true
                    }
                }
                Inst::Run {
                    node,
                    min,
                    max,
                    greedy: true,
                } => {
                    let mut count = 0;
                    while count < *max {
                        match self.single(node, at) {
                            Some(end) => {
                                at = end;
                                count += 1;
                            }
                            None => break,
                        }
                    }
                    if count > *min {
                        state.stack.push(Frame::Shorter { pc, at, count });
                    }
                    pc += 1;
                    count >= *min
                }
                Inst::Run {
                    node,
                    min,
                    max,
                    greedy: false,
                } => {
                    let mut count = 0;
                    let mut ok = true;
                    while count < *min {
                        match self.single(node, at) {
                            Some(end) => {
                                at = end;
                                count += 1;
                            }
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok && count < *max {
                        state.stack.push(Frame::Longer { pc, at, count });
                    }
                    pc += 1;
                    ok
                }
                Inst::Match => {
                    state.stack.truncate(base);
                    return Some(at);
                }
            };
            if ok {
                continue;
            }
            // Back to the last choice still open.
            loop {
                if state.stack.len() == base {
                    return None;
                }
                match state.stack.pop().expect("a frame") {
                    Frame::Branch { pc: to, at: from } => {
                        pc = to;
                        at = from;
                        break;
                    }
                    Frame::Cap { slot, old } => caps[slot] = old,
                    Frame::Reg { id, old } => state.regs[id] = old,
                    Frame::Caps { from } => {
                        caps.copy_from_slice(&state.saved[from..from + caps.len()]);
                        state.saved.truncate(from);
                    }
                    Frame::Enter {
                        pc: to,
                        at: from,
                        id,
                    } => {
                        let (count, _) = state.regs[id];
                        set_reg(state, id, (count, from));
                        pc = to;
                        at = from;
                        break;
                    }
                    Frame::Shorter {
                        pc: run,
                        at: end,
                        count,
                    } => {
                        let Inst::Run { min, .. } = prog[run] else {
                            unreachable!()
                        };
                        // Each step back undoes one character the run took.
                        let end = end - self.char_before(end).map_or(1, char::len_utf8);
                        if count - 1 > min {
                            state.stack.push(Frame::Shorter {
                                pc: run,
                                at: end,
                                count: count - 1,
                            });
                        }
                        pc = run + 1;
                        at = end;
                        break;
                    }
                    Frame::Longer {
                        pc: run,
                        at: end,
                        count,
                    } => {
                        let Inst::Run { node, max, .. } = &prog[run] else {
                            unreachable!()
                        };
                        let Some(next) = self.single(node, end) else {
                            continue;
                        };
                        if count + 1 < *max {
                            state.stack.push(Frame::Longer {
                                pc: run,
                                at: next,
                                count: count + 1,
                            });
                        }
                        pc = run + 1;
                        at = next;
                        break;
                    }
                }
            }
        }
    }
}

/// Sets repeat `id`'s count and where its iteration started, to be undone
/// on the way back.
fn set_reg(state: &mut State, id: usize, value: (usize, usize)) {
    state.stack.push(Frame::Reg {
        id,
        old: state.regs[id],
    });
    state.regs[id] = value;
}

/// One step of a compiled pattern.
#[derive(Clone, Debug)]
enum Inst {
    /// One character, class or `.`.
    One(Node),
    Start,
    End,
    Boundary(bool),
    /// Go on at the first, and at the second on the way back.
    Split(usize, usize),
    Jump(usize),
    /// Where a group starts or ends.
    Save(usize),
    /// A lookahead whose program follows, ending in [`Inst::Match`]; `next`
    /// is the step after that.
    Look {
        next: usize,
        negative: bool,
        captures: bool,
    },
    /// The count of repeat `id` back to 0.
    RepeatInit(usize),
    /// Another iteration of the node that follows, or on to `exit`.
    Repeat {
        id: usize,
        exit: usize,
        min: usize,
        max: usize,
        greedy: bool,
    },
    /// The end of an iteration of repeat `id`, whose [`Inst::Repeat`] is at
    /// `back`.
    RepeatNext {
        id: usize,
        min: usize,
        back: usize,
    },
    /// One character, class or `.` repeated.
    Run {
        node: Node,
        min: usize,
        max: usize,
        greedy: bool,
    },
    Match,
}

/// A way back: a choice still open, or what to undo on the way.
#[derive(Clone, Copy, Debug)]
enum Frame {
    Branch {
        pc: usize,
        at: usize,
    },
    Cap {
        slot: usize,
        old: Option<usize>,
    },
    Reg {
        id: usize,
        old: (usize, usize),
    },
    /// The group positions before a lookahead set them, in
    /// [`State::saved`] from `from`.
    Caps {
        from: usize,
    },
    /// A lazy repeat's next iteration, started at `at`.
    Enter {
        pc: usize,
        at: usize,
        id: usize,
    },
    /// A greedy [`Inst::Run`] at `pc` that took `count` to `at`, to give one
    /// back.
    Shorter {
        pc: usize,
        at: usize,
        count: usize,
    },
    /// A lazy [`Inst::Run`] at `pc` that took `count` to `at`, to take one
    /// more.
    Longer {
        pc: usize,
        at: usize,
        count: usize,
    },
}

/// What a match keeps between steps, kept per thread to be reused.
#[derive(Default)]
struct State {
    stack: Vec<Frame>,
    /// Each repeat's count and where its iteration started.
    regs: Vec<(usize, usize)>,
    saved: Vec<Option<usize>>,
}

thread_local! {
    static STATE: std::cell::Cell<State> = std::cell::Cell::new(State::default());
}

fn compile_alt(alt: &Alt, prog: &mut Vec<Inst>, repeats: &mut usize) {
    let mut jumps = Vec::new();
    for (k, seq) in alt.iter().enumerate() {
        let split = (k + 1 < alt.len()).then(|| {
            prog.push(Inst::Split(prog.len() + 1, 0));
            prog.len() - 1
        });
        for node in seq {
            compile_node(node, prog, repeats);
        }
        if let Some(split) = split {
            jumps.push(prog.len());
            prog.push(Inst::Jump(0));
            prog[split] = Inst::Split(split + 1, prog.len());
        }
    }
    for jump in jumps {
        prog[jump] = Inst::Jump(prog.len());
    }
}

fn compile_node(node: &Node, prog: &mut Vec<Inst>, repeats: &mut usize) {
    match node {
        Node::Char(_) | Node::Any | Node::Class(_) => prog.push(Inst::One(node.clone())),
        Node::Start => prog.push(Inst::Start),
        Node::End => prog.push(Inst::End),
        Node::Boundary(want) => prog.push(Inst::Boundary(*want)),
        Node::Group(alt, None) => compile_alt(alt, prog, repeats),
        Node::Group(alt, Some(index)) => {
            prog.push(Inst::Save(index * 2));
            compile_alt(alt, prog, repeats);
            prog.push(Inst::Save(index * 2 + 1));
        }
        Node::Look(alt, negative, captures) => {
            let look = prog.len();
            prog.push(Inst::Match);
            compile_alt(alt, prog, repeats);
            prog.push(Inst::Match);
            prog[look] = Inst::Look {
                next: prog.len(),
                negative: *negative,
                captures: *captures,
            };
        }
        Node::Repeat {
            node: inner,
            min,
            max,
            greedy,
        } => {
            if matches!(**inner, Node::Char(_) | Node::Any | Node::Class(_)) {
                prog.push(Inst::Run {
                    node: (**inner).clone(),
                    min: *min,
                    max: *max,
                    greedy: *greedy,
                });
                return;
            }
            let id = *repeats;
            *repeats += 1;
            prog.push(Inst::RepeatInit(id));
            let back = prog.len();
            prog.push(Inst::Match);
            compile_node(inner, prog, repeats);
            prog.push(Inst::RepeatNext {
                id,
                min: *min,
                back,
            });
            prog[back] = Inst::Repeat {
                id,
                exit: prog.len(),
                min: *min,
                max: *max,
                greedy: *greedy,
            };
        }
    }
}

/// Whether `alt` holds a capturing group.
fn captures(alt: &Alt) -> bool {
    alt.iter().flatten().any(captures_node)
}

fn captures_node(node: &Node) -> bool {
    match node {
        Node::Group(_, Some(_)) => true,
        Node::Group(inner, None) | Node::Look(inner, _, _) => captures(inner),
        Node::Repeat { node, .. } => captures_node(node),
        _ => false,
    }
}

/// Fills each class's ASCII bitmap.
fn prepare(alt: &mut Alt, ignore_case: bool) {
    for node in alt.iter_mut().flatten() {
        prepare_node(node, ignore_case);
    }
}

fn prepare_node(node: &mut Node, ignore_case: bool) {
    match node {
        Node::Class(class) => {
            let mut ascii = [0u64; 2];
            for b in 0..128u8 {
                if class.matches(b as char, ignore_case) {
                    ascii[usize::from(b >> 6)] |= 1 << (b & 63);
                }
            }
            class.ascii = ascii;
        }
        Node::Group(alt, _) | Node::Look(alt, _, _) => prepare(alt, ignore_case),
        Node::Repeat { node, .. } => prepare_node(node, ignore_case),
        _ => {}
    }
}

/// The characters any match of `alt` starts with, or `None` when it can
/// start with anything or with nothing.
fn first_of_alt(alt: &Alt, ignore_case: bool) -> Option<Set> {
    let mut set = Set::default();
    for seq in alt {
        set.union(first_of_seq(seq, ignore_case)?);
    }
    Some(set)
}

fn first_of_seq(seq: &[Node], ignore_case: bool) -> Option<Set> {
    let node = seq.first()?;
    match node {
        Node::Char(c) => {
            let mut set = Set::default();
            if c.is_ascii() {
                set.insert(*c as u8);
                if ignore_case {
                    set.insert(c.to_ascii_lowercase() as u8);
                    set.insert(c.to_ascii_uppercase() as u8);
                }
            } else {
                set.other = true;
                if ignore_case {
                    for other in [fold(*c), upper(*c)] {
                        if other.is_ascii() {
                            set.insert(other as u8);
                        }
                    }
                }
            }
            Some(set)
        }
        Node::Class(class) => Some(Set {
            ascii: class.ascii,
            other: true,
        }),
        Node::Group(alt, _) => first_of_alt(alt, ignore_case),
        Node::Repeat { node, min, .. } if *min >= 1 => {
            first_of_seq(std::slice::from_ref(&**node), ignore_case)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::Regex;

    fn groups(pattern: &str, flags: &str, text: &str) -> Option<Vec<Option<String>>> {
        let caps = Regex::new(pattern, flags).exec(text)?;
        Some(
            (0..caps.len())
                .map(|i| caps.get(i).map(str::to_string))
                .collect(),
        )
    }

    fn found(pattern: &str, flags: &str, text: &str) -> Vec<Option<&'static str>> {
        groups(pattern, flags, text)
            .expect("a match")
            .into_iter()
            .map(|g| g.map(|g| &*g.leak()))
            .collect()
    }

    #[test]
    fn alternatives_are_tried_in_order() {
        assert_eq!(found("a|ab", "", "abc"), [Some("a")]);
        assert_eq!(
            found("(a|ab)(c|bcd)", "", "abcd"),
            [Some("abcd"), Some("a"), Some("bcd")]
        );
    }

    #[test]
    fn greedy_and_lazy_quantifiers() {
        assert_eq!(found("a+", "", "caaat"), [Some("aaa")]);
        assert_eq!(found("a+?", "", "caaat"), [Some("a")]);
        assert_eq!(found("<.*>", "", "<a><b>"), [Some("<a><b>")]);
        assert_eq!(found("<.*?>", "", "<a><b>"), [Some("<a>")]);
        assert_eq!(found("a{2,3}", "", "aaaa"), [Some("aaa")]);
        assert_eq!(found("a{2,3}?", "", "aaaa"), [Some("aa")]);
        assert_eq!(found("x{2}", "", "xxx"), [Some("xx")]);
        assert_eq!(found("(?:ab){2,}", "", "abababa"), [Some("ababab")]);
        assert_eq!(found("(?:ab)+?b", "", "ababb"), [Some("ababb")]);
        assert_eq!(found("a{,2}", "", "a{,2}"), [Some("a{,2}")]);
    }

    #[test]
    fn a_group_keeps_its_last_iteration() {
        assert_eq!(found("(a|b)+", "", "abab"), [Some("abab"), Some("b")]);
        assert_eq!(found("(?:(a)|b)+", "", "ab"), [Some("ab"), Some("a")]);
        assert_eq!(
            found("(\\w+)\\s(\\w+)", "", "hello big world"),
            [Some("hello big"), Some("hello"), Some("big")]
        );
    }

    #[test]
    fn a_group_that_backtracks_out_takes_no_part() {
        assert_eq!(found("(a)?b", "", "b"), [Some("b"), None]);
        assert_eq!(found("(?:(x)y|xz)", "", "xz"), [Some("xz"), None]);
    }

    #[test]
    fn an_empty_iteration_ends_a_loop() {
        assert_eq!(found("(a*)*", "", "b"), [Some(""), None]);
        assert_eq!(found("(a*)+", "", "b"), [Some(""), Some("")]);
        assert_eq!(found("(?:a?)*?b", "", "aab"), [Some("aab")]);
        assert_eq!(found("(a|)*b", "", "aab"), [Some("aab"), Some("a")]);
    }

    #[test]
    fn lookahead_is_atomic() {
        assert_eq!(found("a(?=b)", "", "acab"), [Some("a")]);
        assert_eq!(Regex::new("a(?=b)", "").exec("acab").unwrap().start(), 2);
        assert_eq!(found("a(?!b)", "", "abac"), [Some("a")]);
        assert_eq!(Regex::new("a(?!b)", "").exec("abac").unwrap().start(), 2);
        assert_eq!(
            found("(?=(a+))a*b", "", "aaab"),
            [Some("aaab"), Some("aaa")]
        );
        assert_eq!(found("(?!(a))b", "", "b"), [Some("b"), None]);
    }

    #[test]
    fn anchors_and_boundaries() {
        assert!(Regex::new("^b", "").exec("a\nb").is_none());
        assert_eq!(found("^b$", "m", "a\nb\nc"), [Some("b")]);
        assert_eq!(found("\\bis\\b", "", "this is"), [Some("is")]);
        assert_eq!(Regex::new("\\Bis", "").exec("this is").unwrap().start(), 2);
        assert_eq!(found(".+", "", "a\nb"), [Some("a")]);
        assert_eq!(found(".+", "s", "a\nb"), [Some("a\nb")]);
    }

    #[test]
    fn classes_and_case() {
        assert_eq!(found("[^a-c]+", "", "abcdefa"), [Some("def")]);
        assert_eq!(found("[a-c]+", "i", "xABCx"), [Some("ABC")]);
        assert_eq!(found("\\d+\\s*\\w", "", "x 12  z"), [Some("12  z")]);
        assert_eq!(found("\\p{L}+", "u", "1 héllo"), [Some("héllo")]);
        assert_eq!(found("ß|é", "i", "É"), [Some("É")]);
    }

    #[test]
    fn sticky_matches_only_at_the_start() {
        let re = Regex::new("b", "y");
        assert!(re.exec_at("ab", 0).is_none());
        assert_eq!(re.exec_at("ab", 1).unwrap().get(0), Some("b"));
    }

    #[test]
    fn split_replace_and_find_all() {
        let re = Regex::new("(,)\\s*", "");
        assert_eq!(
            re.split("a, b,c"),
            [Some("a"), Some(","), Some("b"), Some(","), Some("c")]
        );
        let re = Regex::new("o", "g");
        assert_eq!(re.replace("foo boo", |_| "0".into()), "f00 b00");
        assert_eq!(re.find_all("foo"), ["o", "o"]);
        let re = Regex::new("x*", "g");
        assert_eq!(re.replace("abc", |_| "-".into()), "-a-b-c-");
    }

    #[test]
    fn a_long_input_does_not_exhaust_the_stack() {
        let text = "ab".repeat(100_000);
        assert_eq!(
            Regex::new("(?:a|b)*$", "").exec(&text).unwrap().end(),
            text.len()
        );
        assert_eq!(
            Regex::new("(?:ab)+c|(?:ab)+", "")
                .exec(&text)
                .unwrap()
                .end(),
            text.len()
        );
    }
}
