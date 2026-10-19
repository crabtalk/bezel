//! What the four railroad grammars share: the title and accessibility tokens
//! (which, unlike the ones in Mermaid's common grammar, do not take leading
//! blanks), the hidden tokens, the statements before the first rule, string
//! values, and the walk from a syntax tree to the rule model. Pele's
//! `diagrams/railroad/common.ts`.

use super::types::{NodeId, Nodes, RailroadNode, RailroadRule};
use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Scanner, Table, TokenType, acc_descr_value, acc_title_value, directive, keyword,
        single_line_comment, title_value, tokenize, yaml,
    },
    error::Error,
    util::chars::{code_at, is_space, width_at},
};

pub const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const NAME_START: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_";

/// The braced form scans to the next closing brace. Once none is found, none
/// exists further on either, so later braced descriptions fail without
/// scanning again.
fn acc_descr_scanner() -> Scanner {
    let mut unclosed = usize::MAX;
    Box::new(move |src: &str, at: usize| {
        if !src[at..].starts_with("accDescr") {
            return None;
        }
        let n = src.len();
        let mut i = at + 8;
        while i < n && matches!(code_at(src, i), 32 | 9) {
            i += 1;
        }
        if code_at(src, i) == 58 {
            i += 1;
            while i < n {
                let c = code_at(src, i);
                if c == 10 || c == 13 || (c == 37 && code_at(src, i + 1) == 37) {
                    break;
                }
                i += width_at(src, i);
            }
            return Some(i);
        }
        while i < n && is_space(code_at(src, i)) {
            i += width_at(src, i);
        }
        if code_at(src, i) != 123 || i >= unclosed {
            return None;
        }
        match src[i + 1..].find('}') {
            Some(close) => Some(i + 1 + close + 1),
            None => {
                unclosed = i;
                None
            }
        }
    })
}

pub fn title() -> TokenType {
    TokenType::regex(
        "TITLE",
        r"title(?:[\t ][^\n\r]*?(?=%%)|[\t ][^\n\r]*|)",
        Some("t"),
    )
}

pub fn acc_title() -> TokenType {
    TokenType::regex(
        "ACC_TITLE",
        r"accTitle[\t ]*:(?:[^\n\r]*?(?=%%)|[^\n\r]*)",
        Some("a"),
    )
}

pub fn acc_descr() -> TokenType {
    TokenType::regex(
        "ACC_DESCR",
        r"accDescr(?:[\t ]*:([^\n\r]*?(?=%%)|[^\n\r]*)|\s*{([^}]*)})",
        Some("a"),
    )
    .scanner(acc_descr_scanner)
}

pub fn literal(name: &'static str) -> TokenType {
    TokenType::literal(name, name)
}

/// The diagram's keyword. A name that starts with it and goes on is a name.
pub fn opening(word: &'static str, name: &'static str) -> TokenType {
    keyword(word).longer(&[name])
}

pub fn identifier(name: &'static str) -> TokenType {
    TokenType::regex(name, r"[A-Z_a-z][\w-]*", Some(NAME_START))
}

pub fn quoted(name: &'static str) -> TokenType {
    TokenType::regex(name, r#""([^"\\]|\\.)*"|'([^'\\]|\\.)*'"#, Some("\"'"))
}

/// Whitespace, front matter, directive, and `%%` comment, in the order every
/// railroad grammar lists them.
pub fn hidden_tokens(prefix: &str) -> Vec<TokenType> {
    vec![
        TokenType::regex("", r"[\t \r\n]+", Some("\t \r\n"))
            .hidden()
            .named(format!("{prefix}_WHITESPACE")),
        yaml().named(format!("{prefix}_YAML")),
        directive().named(format!("{prefix}_DIRECTIVE")),
        single_line_comment().named(format!("{prefix}_SINGLE_LINE_COMMENT")),
    ]
}

/// `text.slice(1, -1)`.
pub fn inner(text: &str) -> &str {
    if text.len() < 2 {
        return "";
    }
    &text[1..text.len() - 1]
}

/// Mermaid's railroad value converters know three escapes; any other escaped
/// character stands for itself.
pub fn unescape(text: &str) -> String {
    let body = inner(text);
    if !text.contains('\\') {
        return body.to_string();
    }
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c == '\\'
            && let Some(next) = chars.next()
        {
            out.push(match next {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
        } else {
            out.push(c);
        }
    }
    out
}

/// A syntax tree, its parts in one list.
pub struct Syntax<K> {
    pub parts: Vec<Part<K>>,
}

/// A node of a syntax tree, with the places of the nodes that hold its parts.
pub struct Part<K> {
    pub kind: K,
    pub kids: Vec<usize>,
}

impl<K> Default for Syntax<K> {
    fn default() -> Self {
        Self { parts: Vec::new() }
    }
}

impl<K> Syntax<K> {
    pub fn add(&mut self, kind: K, kids: Vec<usize>) -> usize {
        self.parts.push(Part { kind, kids });
        self.parts.len() - 1
    }
}

pub struct SyntaxRule {
    pub name: String,
    pub definition: usize,
}

#[derive(Default)]
pub struct Grammar {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub rules: Vec<SyntaxRule>,
}

/// Parses the keyword, then any titles and accessibility statements, then
/// rules to the end of the text. Every table starts with its keyword and has
/// TITLE, ACC_TITLE, and ACC_DESCR in a row from `meta`.
pub fn parse_grammar(
    src: &str,
    table: &Table,
    meta: i32,
    unquote: fn(&str) -> String,
    mut rule: impl FnMut(&mut Reader) -> Result<SyntaxRule, Error>,
) -> Result<Grammar, Error> {
    let tokens = tokenize(src, table, Kind::Railroad)?;
    let mut r = Reader::new(&tokens, table, Kind::Railroad);
    let mut ast = Grammar::default();
    r.expect(0)?;
    loop {
        let kind = r.kind();
        if kind == meta {
            let title = title_value(r.take());
            let quoted = match title.chars().next() {
                Some(quote @ ('"' | '\'')) => title.ends_with(quote),
                _ => false,
            };
            ast.title = Some(if quoted { unquote(&title) } else { title });
        } else if kind == meta + 1 {
            ast.acc_title = Some(acc_title_value(r.take()));
        } else if kind == meta + 2 {
            ast.acc_descr = Some(acc_descr_value(r.take()));
        } else {
            break;
        }
    }
    while r.kind() != -1 {
        ast.rules.push(rule(&mut r)?);
    }
    Ok(ast)
}

pub fn terminal(nodes: &mut Nodes, value: String) -> NodeId {
    nodes.add(RailroadNode::Terminal { value })
}

pub fn nonterminal(nodes: &mut Nodes, name: String) -> NodeId {
    nodes.add(RailroadNode::NonTerminal { name })
}

pub fn special(nodes: &mut Nodes, text: String) -> NodeId {
    nodes.add(RailroadNode::Special { text })
}

pub fn optional(nodes: &mut Nodes, element: NodeId) -> NodeId {
    nodes.add(RailroadNode::Optional { element })
}

pub fn repetition(nodes: &mut Nodes, element: NodeId, min: f64, max: f64) -> NodeId {
    nodes.add(RailroadNode::Repetition { element, min, max })
}

/// A sequence or a choice of one is that one, in every notation.
pub fn sequence(nodes: &mut Nodes, elements: Vec<NodeId>) -> NodeId {
    if elements.len() == 1 {
        return elements[0];
    }
    nodes.add(RailroadNode::Sequence { elements })
}

pub fn choice(nodes: &mut Nodes, alternatives: Vec<NodeId>) -> NodeId {
    if alternatives.len() == 1 {
        return alternatives[0];
    }
    nodes.add(RailroadNode::Choice { alternatives })
}

/// Turns each rule's syntax tree into the common model, given how one node is
/// built from its parts.
pub fn to_rules<K>(
    ast: &Grammar,
    syntax: &Syntax<K>,
    nodes: &mut Nodes,
    mut build: impl FnMut(&K, Vec<NodeId>, &mut Nodes) -> NodeId,
) -> Vec<RailroadRule> {
    ast.rules
        .iter()
        .map(|rule| RailroadRule {
            name: rule.name.clone(),
            definition: fold(
                rule.definition,
                |at| syntax.parts[at].kids.as_slice(),
                |at, kids| build(&syntax.parts[at].kind, kids, nodes),
            ),
        })
        .collect()
}

/// Rebuilds a tree bottom-up without recursion, so that nesting depth is
/// bounded by memory, not by the stack.
pub fn fold<'a, T: Copy + 'a, R>(
    root: T,
    children: impl Fn(T) -> &'a [T],
    mut build: impl FnMut(T, Vec<R>) -> R,
) -> R {
    let mut nodes = vec![root];
    let mut lists = vec![children(root)];
    let mut done: Vec<Vec<R>> = vec![Vec::new()];
    loop {
        let top = nodes.len() - 1;
        if done[top].len() < lists[top].len() {
            let child = lists[top][done[top].len()];
            nodes.push(child);
            lists.push(children(child));
            done.push(Vec::new());
        } else {
            let results = done.pop().unwrap_or_default();
            let value = build(nodes[top], results);
            if top == 0 {
                return value;
            }
            nodes.pop();
            lists.pop();
            done[top - 1].push(value);
        }
    }
}
