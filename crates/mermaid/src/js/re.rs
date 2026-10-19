//! Regular expressions with JavaScript's syntax and matching rules, for the
//! patterns ported from Pele.
//!
//! A backtracking matcher over the pattern's tree. It reads what Pele's
//! patterns use: classes, `\d \w \s \b`, `\p{L}` and `\p{N}` under `u`,
//! groups, alternation, greedy and lazy quantifiers, lookahead, and the flags
//! `g i m s u y`. `{` that does not open a quantifier, and `]` and `}` outside
//! a class, are literal, as in JavaScript without `u`. No backreferences and no
//! lookbehind.
//!
//! Positions are byte offsets into UTF-8, and `.` takes a code point where
//! JavaScript takes a UTF-16 unit.

use std::fmt::Write as _;
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
    alt: Alt,
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
        Self {
            alt,
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
        let mut end = None;
        matcher
            .alt(&self.alt, at, caps, &mut |pos, _| {
                end = Some(pos);
                true
            })
            .then(|| end.unwrap_or(at))
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

/// `\s`: JavaScript's whitespace and line terminators.
pub(crate) fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `\w`.
pub(crate) fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
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
type Next<'k> = &'k mut dyn FnMut(usize, &mut Caps) -> bool;

impl Matcher<'_> {
    fn char_at(&self, at: usize) -> Option<char> {
        self.text.get(at..)?.chars().next()
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

    fn alt(&self, alt: &Alt, at: usize, caps: &mut Caps, next: Next) -> bool {
        for seq in alt {
            if self.seq(seq, 0, at, caps, next) {
                return true;
            }
        }
        false
    }

    fn seq(&self, seq: &[Node], i: usize, at: usize, caps: &mut Caps, next: Next) -> bool {
        let Some(node) = seq.get(i) else {
            return next(at, caps);
        };
        match node {
            Node::Char(_) | Node::Any | Node::Class(_) => match self.single(node, at) {
                Some(end) => self.seq(seq, i + 1, end, caps, next),
                None => false,
            },
            Node::Start => {
                let ok = at == 0
                    || (self.flags.multiline && self.char_before(at).is_some_and(is_line_end));
                ok && self.seq(seq, i + 1, at, caps, next)
            }
            Node::End => {
                let ok = at == self.text.len()
                    || (self.flags.multiline && self.char_at(at).is_some_and(is_line_end));
                ok && self.seq(seq, i + 1, at, caps, next)
            }
            Node::Boundary(want) => {
                let before = self.char_before(at).is_some_and(is_word);
                let after = self.char_at(at).is_some_and(is_word);
                ((before != after) == *want) && self.seq(seq, i + 1, at, caps, next)
            }
            Node::Group(alt, index) => {
                let saved = index.map(|index| (caps[index * 2], caps[index * 2 + 1]));
                let found = self.alt(alt, at, caps, &mut |end, caps| {
                    let before = index.map(|index| (caps[index * 2], caps[index * 2 + 1]));
                    if let Some(index) = index {
                        caps[index * 2] = Some(at);
                        caps[index * 2 + 1] = Some(end);
                    }
                    if self.seq(seq, i + 1, end, caps, next) {
                        return true;
                    }
                    if let (Some(index), Some((start, stop))) = (index, before) {
                        caps[*index * 2] = start;
                        caps[*index * 2 + 1] = stop;
                    }
                    false
                });
                if !found && let (Some(index), Some((start, stop))) = (index, saved) {
                    caps[*index * 2] = start;
                    caps[*index * 2 + 1] = stop;
                }
                found
            }
            Node::Look(alt, negative, false) => {
                let found = self.alt(alt, at, caps, &mut |_, _| true);
                found != *negative && self.seq(seq, i + 1, at, caps, next)
            }
            Node::Look(alt, negative, true) => {
                let mut inner = caps.to_vec();
                let found = self.alt(alt, at, &mut inner, &mut |_, _| true);
                if found == *negative {
                    return false;
                }
                if found {
                    let saved = caps.to_vec();
                    caps.copy_from_slice(&inner);
                    if self.seq(seq, i + 1, at, caps, next) {
                        return true;
                    }
                    caps.copy_from_slice(&saved);
                    false
                } else {
                    self.seq(seq, i + 1, at, caps, next)
                }
            }
            Node::Repeat {
                node: inner,
                min,
                max,
                greedy,
            } => {
                if matches!(**inner, Node::Char(_) | Node::Any | Node::Class(_)) {
                    if !*greedy {
                        let mut pos = at;
                        let mut count = 0;
                        loop {
                            if count >= *min && self.seq(seq, i + 1, pos, caps, next) {
                                return true;
                            }
                            if count >= *max {
                                return false;
                            }
                            match self.single(inner, pos) {
                                Some(end) => {
                                    pos = end;
                                    count += 1;
                                }
                                None => return false,
                            }
                        }
                    }
                    let mut pos = at;
                    let mut count = 0;
                    while count < *max {
                        match self.single(inner, pos) {
                            Some(end) => {
                                pos = end;
                                count += 1;
                            }
                            None => break,
                        }
                    }
                    if count < *min {
                        return false;
                    }
                    loop {
                        if self.seq(seq, i + 1, pos, caps, next) {
                            return true;
                        }
                        if count == *min {
                            return false;
                        }
                        // Each step back undoes one character the run took.
                        pos -= self.char_before(pos).map_or(1, char::len_utf8);
                        count -= 1;
                    }
                }
                self.repeat(inner, *min, *max, *greedy, 0, at, caps, &mut |end, caps| {
                    self.seq(seq, i + 1, end, caps, next)
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &self,
        node: &Node,
        min: usize,
        max: usize,
        greedy: bool,
        count: usize,
        at: usize,
        caps: &mut Caps,
        next: Next,
    ) -> bool {
        let more = |caps: &mut Caps, next: Next| -> bool {
            if count >= max {
                return false;
            }
            let one = std::slice::from_ref(node);
            self.seq(one, 0, at, caps, &mut |end, caps| {
                // An iteration that takes nothing past the minimum ends the loop.
                if end == at && count >= min {
                    return false;
                }
                self.repeat(node, min, max, greedy, count + 1, end, caps, next)
            })
        };
        if count < min {
            return more(caps, next);
        }
        if greedy {
            more(caps, &mut *next) || next(at, caps)
        } else {
            next(at, caps) || more(caps, next)
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
