//! What the four railroad grammars share: the title and accessibility tokens
//! (which, unlike the ones in Mermaid's common grammar, do not take leading
//! blanks), the hidden tokens, the statements before the first rule, string
//! values, and the walk from a syntax tree to the rule model. Pele's
//! `diagrams/railroad/common.ts`.

use super::types::{NodeId, Nodes, RailroadRule};
use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr as common_acc_descr,
        tokens::{self, Header, Kinds, Reader, TokenType},
    },
    error::Error,
    util::chars::inner,
};

pub const LETTERS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const NAME_START: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_";

pub fn title<K>() -> TokenType<K> {
    tokens::title().first("t")
}

pub fn acc_title<K>() -> TokenType<K> {
    tokens::acc_title().first("a")
}

pub fn acc_descr<K>() -> TokenType<K> {
    common_acc_descr("ACC_DESCR").first("a")
}

pub fn identifier<K>(name: &'static str) -> TokenType<K> {
    TokenType::regex(name, r"[A-Z_a-z][\w-]*", Some(NAME_START))
}

pub fn quoted<K>(name: &'static str) -> TokenType<K> {
    tokens::string().named(name)
}

/// The whitespace every railroad grammar hides, line breaks included.
pub fn blank<K>() -> TokenType<K> {
    TokenType::regex("WHITESPACE", r"[\t \r\n]+", Some("\t \r\n")).hidden()
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
pub struct Syntax<P> {
    pub parts: Vec<Part<P>>,
}

/// A node of a syntax tree, with the places of the nodes that hold its parts.
pub struct Part<P> {
    pub kind: P,
    pub kids: Vec<usize>,
}

impl<P> Default for Syntax<P> {
    fn default() -> Self {
        Self { parts: Vec::new() }
    }
}

impl<P> Syntax<P> {
    pub fn add(&mut self, kind: P, kids: Vec<usize>) -> usize {
        self.parts.push(Part { kind, kids });
        self.parts.len() - 1
    }
}

pub struct SyntaxRule {
    pub name: String,
    pub definition: usize,
}

/// A grammar as written in one notation, its rules' definitions in `syntax`.
pub struct Grammar<P> {
    pub header: Header,
    pub rules: Vec<SyntaxRule>,
    pub syntax: Syntax<P>,
}

/// A notation's syntax tree node, which becomes a model node.
pub trait ToNode {
    /// The model node, given the model nodes of the parts.
    fn to_node(&self, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId;
}

impl<P: ToNode> Grammar<P> {
    /// The header, and each rule in the common model, its nodes added to
    /// `nodes`.
    pub fn into_rules(self, nodes: &mut Nodes) -> (Header, Vec<RailroadRule>) {
        let parts = &self.syntax.parts;
        let rules = self
            .rules
            .into_iter()
            .map(|rule| RailroadRule {
                name: rule.name,
                definition: fold(
                    rule.definition,
                    |at| parts[at].kids.as_slice(),
                    |at, kids| parts[at].kind.to_node(kids, nodes),
                ),
            })
            .collect();
        (self.header, rules)
    }
}

/// A notation's tokens, and the syntax tree its rules are read into.
pub struct Parser<'s, K, P> {
    pub r: Reader<'s, K>,
    pub syntax: Syntax<P>,
}

impl<'s, K: Kinds, P> Parser<'s, K, P> {
    /// Parses the `keyword`, then any titles and accessibility statements,
    /// then rules to the end of the text. A quoted title is read with
    /// `unquote`.
    pub fn parse(
        src: &'s str,
        keyword: K,
        unquote: fn(&str) -> String,
        rule: fn(&mut Self) -> Result<SyntaxRule, Error>,
    ) -> Result<Grammar<P>, Error> {
        let mut p = Self {
            r: Reader::new(src, Kind::Railroad)?,
            syntax: Syntax::default(),
        };
        let mut header = Header::default();
        p.r.expect(keyword)?;
        while p.r.heading(&mut header) {}
        if let Some(title) = &mut header.title {
            let quoted = match title.chars().next() {
                Some(quote @ ('"' | '\'')) => title.ends_with(quote),
                _ => false,
            };
            if quoted {
                *title = unquote(title);
            }
        }
        let mut rules = Vec::new();
        while !p.r.at_end() {
            rules.push(rule(&mut p)?);
        }
        Ok(Grammar {
            header,
            rules,
            syntax: p.syntax,
        })
    }
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
