//! `railroad-peg-beta`: parsing expression grammars. Pele's
//! `diagrams/railroad/peg.ts`.

use super::{
    common::{
        Grammar, Parser, Syntax, SyntaxRule, ToNode, acc_descr, acc_title, blank, identifier,
        quoted, title, unescape,
    },
    types::{NodeId, Nodes, RailroadNode},
};
use crate::{
    diagrams::common::tokens::{
        TokenType, directive, keyword, single_line_comment, token_kinds, yaml,
    },
    error::Error,
};

token_kinds! {
    enum Token {
        Keyword = keyword("railroad-peg-beta").longer(&[Token::Id]),
        Arrow = TokenType::literal("<-"),
        Semicolon = TokenType::literal(";"),
        Slash = TokenType::literal("/"),
        And = TokenType::literal("&"),
        Not = TokenType::literal("!"),
        Question = TokenType::literal("?"),
        Star = TokenType::literal("*"),
        Plus = TokenType::literal("+"),
        Group = TokenType::literal("("),
        GroupEnd = TokenType::literal(")"),
        Dot = TokenType::literal("."),
        Title = title(),
        AccTitle = acc_title(),
        AccDescr = acc_descr(),
        Id = identifier("PEG_ID"),
        String = quoted("PEG_STRING"),
        Whitespace = blank(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        LineComment = TokenType::regex("PEG_LINE_COMMENT", r"#[^\n\r]*", Some("#")).hidden(),
    }
}

pub enum PegPart {
    OrderedChoice,
    Sequence,
    Prefix(Option<Predicate>),
    Suffix(Option<Suffix>),
    Literal(String),
    Identifier(String),
    /// `.`
    Any,
    Group,
}

/// `&` or `!`.
#[derive(Clone, Copy)]
pub enum Predicate {
    And,
    Not,
}

/// `?`, `*` or `+`.
#[derive(Clone, Copy)]
pub enum Suffix {
    Optional,
    ZeroOrMore,
    OneOrMore,
}

impl Token {
    fn predicate(self) -> Option<Predicate> {
        match self {
            Self::And => Some(Predicate::And),
            Self::Not => Some(Predicate::Not),
            _ => None,
        }
    }

    fn suffix(self) -> Option<Suffix> {
        match self {
            Self::Question => Some(Suffix::Optional),
            Self::Star => Some(Suffix::ZeroOrMore),
            Self::Plus => Some(Suffix::OneOrMore),
            _ => None,
        }
    }
}

/// One open group, or the rule itself at the bottom of the stack.
struct Frame {
    choice: usize,
    sequence: usize,
    /// The group's node and the predicate written before it.
    node: Option<usize>,
    operator: Option<Predicate>,
}

impl Frame {
    fn open(
        syntax: &mut Syntax<PegPart>,
        node: Option<usize>,
        choice: usize,
        operator: Option<Predicate>,
    ) -> Self {
        let sequence = syntax.add(PegPart::Sequence, Vec::new());
        syntax.parts[choice].kids.push(sequence);
        Self {
            choice,
            sequence,
            node,
            operator,
        }
    }
}

impl Parser<'_, Token, PegPart> {
    fn rule(&mut self) -> Result<SyntaxRule, Error> {
        let name = self.r.expect(Token::Id)?.to_string();
        self.r.expect(Token::Arrow)?;
        let definition = self.definition()?;
        self.r.expect(Token::Semicolon)?;
        Ok(SyntaxRule { name, definition })
    }

    fn definition(&mut self) -> Result<usize, Error> {
        let syntax = &mut self.syntax;
        let r = &mut self.r;
        let mut stack: Vec<Frame> = Vec::new();
        let root = syntax.add(PegPart::OrderedChoice, Vec::new());
        let mut top = Frame::open(syntax, None, root, None);
        loop {
            let mut operator = r.kind().and_then(Token::predicate);
            if operator.is_some() {
                r.bump();
            }
            let mut primary = match r.kind() {
                Some(Token::String) => syntax.add(PegPart::Literal(unescape(r.take())), Vec::new()),
                Some(Token::Id) => {
                    syntax.add(PegPart::Identifier(r.take().to_string()), Vec::new())
                }
                Some(Token::Dot) => {
                    r.bump();
                    syntax.add(PegPart::Any, Vec::new())
                }
                Some(Token::Group) => {
                    r.bump();
                    let element = syntax.add(PegPart::OrderedChoice, Vec::new());
                    let group = syntax.add(PegPart::Group, vec![element]);
                    let next = Frame::open(syntax, Some(group), element, operator);
                    stack.push(std::mem::replace(&mut top, next));
                    continue;
                }
                _ => {
                    return Err(r.fail("a literal, a name, a dot, or an opening parenthesis"));
                }
            };
            loop {
                let suffix_operator = r.kind().and_then(Token::suffix);
                if suffix_operator.is_some() {
                    r.bump();
                }
                let suffix = syntax.add(PegPart::Suffix(suffix_operator), vec![primary]);
                let prefix = syntax.add(PegPart::Prefix(operator.take()), vec![suffix]);
                syntax.parts[top.sequence].kids.push(prefix);
                if r.at(Token::GroupEnd) && !stack.is_empty() {
                    r.bump();
                    primary = top.node.unwrap_or_default();
                    let parent = stack
                        .pop()
                        .unwrap_or_else(|| unreachable!("a group has a parent"));
                    operator = std::mem::replace(&mut top, parent).operator;
                    continue;
                }
                match r.kind() {
                    Some(Token::Slash) => {
                        r.bump();
                        top.sequence = syntax.add(PegPart::Sequence, Vec::new());
                        syntax.parts[top.choice].kids.push(top.sequence);
                    }
                    Some(
                        Token::And
                        | Token::Not
                        | Token::String
                        | Token::Id
                        | Token::Dot
                        | Token::Group,
                    ) => {}
                    _ if stack.is_empty() => return Ok(top.choice),
                    _ => return Err(r.expected(Token::GroupEnd)),
                }
                break;
            }
        }
    }
}

pub fn parse_peg(src: &str) -> Result<Grammar<PegPart>, Error> {
    Parser::parse(src, Token::Keyword, unescape, Parser::rule)
}

impl RailroadNode {
    /// What a predicate shows of the expression it looks ahead for.
    fn predicate_label(&self) -> String {
        match self {
            Self::Terminal { value } => format!("\"{value}\""),
            Self::NonTerminal { name } => name.clone(),
            Self::Special { text } => text.clone(),
            _ => "(...)".into(),
        }
    }
}

impl ToNode for PegPart {
    fn to_node(&self, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
        let kid = kids.first().copied().unwrap_or_default();
        match self {
            PegPart::OrderedChoice => nodes.choice(kids),
            PegPart::Sequence => nodes.sequence(kids),
            PegPart::Prefix(Some(predicate)) => {
                let symbol = match predicate {
                    Predicate::And => '&',
                    Predicate::Not => '!',
                };
                let text = format!("{symbol}{}", nodes.get(kid).predicate_label());
                nodes.special(text)
            }
            PegPart::Suffix(Some(Suffix::Optional)) => nodes.optional(kid),
            PegPart::Suffix(Some(Suffix::ZeroOrMore)) => nodes.repetition(kid, 0.0, f64::INFINITY),
            PegPart::Suffix(Some(Suffix::OneOrMore)) => nodes.repetition(kid, 1.0, f64::INFINITY),
            PegPart::Prefix(None) | PegPart::Suffix(None) => kid,
            PegPart::Literal(value) => nodes.terminal(value.clone()),
            PegPart::Identifier(name) => nodes.nonterminal(name.clone()),
            PegPart::Any => nodes.special(".".into()),
            PegPart::Group => kid,
        }
    }
}
