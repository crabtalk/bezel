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

use std::{
    ops::{Deref, Range},
    sync::OnceLock,
};

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
    Look(Box<Alt>, bool),
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
        Self {
            alt,
            groups: parser.groups,
            flags: parsed,
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
        let mut at = from;
        loop {
            if let Some(found) = self.match_here(text, at) {
                return Some(found);
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
        self.match_here(text, at).map(|found| found.end())
    }

    /// The match starting exactly at `at`.
    pub(crate) fn match_here<'t>(&self, text: &'t str, at: usize) -> Option<Captures<'t>> {
        let mut caps = vec![None; (self.groups + 1) * 2];
        let matcher = Matcher {
            text,
            flags: self.flags,
        };
        let mut end = None;
        if matcher.alt(&self.alt, at, &mut caps, &mut |pos, _| {
            end = Some(pos);
            true
        }) {
            let mut spans = vec![Some(at..end.unwrap_or(at))];
            for group in 1..=self.groups {
                spans.push(match (caps[group * 2], caps[group * 2 + 1]) {
                    (Some(start), Some(end)) => Some(start..end),
                    _ => None,
                });
            }
            Some(Captures { text, spans })
        } else {
            None
        }
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
            out.push_str(&with(&caps));
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
                    return Node::Look(Box::new(alt), negative);
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

type Caps = Vec<Option<usize>>;
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
            Node::Look(alt, negative) => {
                let mut inner = caps.clone();
                let found = self.alt(alt, at, &mut inner, &mut |_, _| true);
                if found == *negative {
                    return false;
                }
                if found {
                    let saved = std::mem::replace(caps, inner);
                    if self.seq(seq, i + 1, at, caps, next) {
                        return true;
                    }
                    *caps = saved;
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
                    let mut ends = vec![at];
                    let mut pos = at;
                    while ends.len() <= *max {
                        match self.single(inner, pos) {
                            Some(end) => {
                                pos = end;
                                ends.push(end);
                            }
                            None => break,
                        }
                    }
                    let count = ends.len() - 1;
                    if count < *min {
                        return false;
                    }
                    if *greedy {
                        for n in (*min..=count).rev() {
                            if self.seq(seq, i + 1, ends[n], caps, next) {
                                return true;
                            }
                        }
                    } else {
                        for &end in &ends[*min..=count] {
                            if self.seq(seq, i + 1, end, caps, next) {
                                return true;
                            }
                        }
                    }
                    return false;
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
