//! `railroad-abnf-beta`: ABNF as in RFC 5234, with each rule closed by a
//! semicolon. Pele's `diagrams/railroad/abnf.ts`.

use crate::util::chars::inner;

use super::{
    common::{
        Grammar, LETTERS, Parser, Syntax, SyntaxRule, ToNode, acc_descr, acc_title, blank, title,
    },
    types::{NodeId, Nodes},
};
use crate::{
    diagrams::common::tokens::{
        TokenType, directive, keyword, single_line_comment, token_kinds, yaml,
    },
    error::Error,
    js,
};

const DIGITS: &str = "0123456789";
const REPEAT_FIRST: &str = "0123456789*";

token_kinds! {
    enum Token {
        Keyword = keyword("railroad-abnf-beta").longer(&[Token::Name]),
        Equals = TokenType::literal("="),
        // A semicolon closes a rule only at the end of its line; with anything
        // after it, it starts a comment.
        Semicolon = TokenType::literal(";").longer(&[Token::LineComment]),
        Slash = TokenType::literal("/"),
        Group = TokenType::literal("("),
        GroupEnd = TokenType::literal(")"),
        Optional = TokenType::literal("["),
        OptionalEnd = TokenType::literal("]"),
        Title = title(),
        AccTitle = acc_title(),
        AccDescr = acc_descr(),
        Name = TokenType::regex("ABNF_RULENAME", r"[A-Za-z][A-Za-z0-9-]*", Some(LETTERS)),
        String = TokenType::regex("ABNF_STRING", r#""[^"]*""#, Some("\"")),
        Number = TokenType::regex(
            "ABNF_NUMVAL",
            r"%[xXdDbB][0-9A-Fa-f]+(?:-[0-9A-Fa-f]+|\.[0-9A-Fa-f]+)*",
            Some("%"),
        ),
        Repeat = TokenType::regex("ABNF_REPEAT", r"[0-9]*\*[0-9]*", Some(REPEAT_FIRST)),
        Exact = TokenType::regex("ABNF_EXACT_REPEAT", r"[0-9]+", Some(DIGITS)),
        Whitespace = blank(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        LineComment = TokenType::regex("ABNF_COMMENT", r";[^\n\r]*", Some(";")).hidden(),
    }
}

pub enum AbnfPart {
    Alternation,
    Concatenation,
    Element(Option<String>),
    StringLiteral(String),
    NumVal(String),
    RuleName(String),
    Group,
    OptionalGroup,
}

/// One open bracket, or the rule itself at the bottom of the stack.
struct Frame {
    alternation: usize,
    concatenation: usize,
    /// The bracket's node, its closing token, and the repeat written before
    /// it.
    node: Option<usize>,
    close: Option<Token>,
    repeat: Option<String>,
}

impl Frame {
    fn open(
        syntax: &mut Syntax<AbnfPart>,
        node: Option<usize>,
        alternation: usize,
        close: Option<Token>,
        repeat: Option<String>,
    ) -> Self {
        let concatenation = syntax.add(AbnfPart::Concatenation, Vec::new());
        syntax.parts[alternation].kids.push(concatenation);
        Self {
            alternation,
            concatenation,
            node,
            close,
            repeat,
        }
    }
}

impl Parser<'_, Token, AbnfPart> {
    fn rule(&mut self) -> Result<SyntaxRule, Error> {
        let name = self.r.expect(Token::Name)?.to_string();
        self.r.expect(Token::Equals)?;
        let definition = self.alternation()?;
        self.r.expect(Token::Semicolon)?;
        Ok(SyntaxRule { name, definition })
    }

    fn alternation(&mut self) -> Result<usize, Error> {
        let syntax = &mut self.syntax;
        let r = &mut self.r;
        let mut stack: Vec<Frame> = Vec::new();
        let root = syntax.add(AbnfPart::Alternation, Vec::new());
        let mut top = Frame::open(syntax, None, root, None, None);
        loop {
            let mut repeat = if r.at(Token::Repeat) || r.at(Token::Exact) {
                Some(r.take().to_string())
            } else {
                None
            };
            let kind = r.kind();
            let mut primary = match kind {
                Some(Token::String) => syntax.add(
                    AbnfPart::StringLiteral(inner(r.take()).to_string()),
                    Vec::new(),
                ),
                Some(Token::Number) => {
                    syntax.add(AbnfPart::NumVal(r.take().to_string()), Vec::new())
                }
                Some(Token::Name) => {
                    syntax.add(AbnfPart::RuleName(r.take().to_string()), Vec::new())
                }
                Some(Token::Group | Token::Optional) => {
                    r.bump();
                    let element = syntax.add(AbnfPart::Alternation, Vec::new());
                    let (bracket, close) = if kind == Some(Token::Group) {
                        (AbnfPart::Group, Token::GroupEnd)
                    } else {
                        (AbnfPart::OptionalGroup, Token::OptionalEnd)
                    };
                    let bracket = syntax.add(bracket, vec![element]);
                    let next = Frame::open(syntax, Some(bracket), element, Some(close), repeat);
                    stack.push(std::mem::replace(&mut top, next));
                    continue;
                }
                _ => {
                    return Err(r.fail("a string, a number, a name, or an opening bracket"));
                }
            };
            loop {
                let element = syntax.add(AbnfPart::Element(repeat.take()), vec![primary]);
                syntax.parts[top.concatenation].kids.push(element);
                if top.close.is_some_and(|close| r.at(close)) {
                    r.bump();
                    primary = top.node.unwrap_or_default();
                    let parent = stack
                        .pop()
                        .unwrap_or_else(|| unreachable!("a bracket has a parent"));
                    repeat = std::mem::replace(&mut top, parent).repeat;
                    continue;
                }
                match r.kind() {
                    Some(Token::Slash) => {
                        r.bump();
                        top.concatenation = syntax.add(AbnfPart::Concatenation, Vec::new());
                        syntax.parts[top.alternation].kids.push(top.concatenation);
                    }
                    Some(
                        Token::String
                        | Token::Number
                        | Token::Name
                        | Token::Group
                        | Token::Optional
                        | Token::Repeat
                        | Token::Exact,
                    ) => {}
                    _ => {
                        return match top.close {
                            Some(close) => Err(r.expected(close)),
                            None => Ok(top.alternation),
                        };
                    }
                }
                break;
            }
        }
    }
}

pub fn parse_abnf(src: &str) -> Result<Grammar<AbnfPart>, Error> {
    Parser::parse(
        src,
        Token::Keyword,
        |text| inner(text).to_string(),
        Parser::rule,
    )
}

impl ToNode for AbnfPart {
    fn to_node(&self, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
        match self {
            AbnfPart::Alternation => nodes.choice(kids),
            AbnfPart::Concatenation => nodes.sequence(kids),
            AbnfPart::Element(repeat) => {
                let Some(repeat) = repeat.as_deref().filter(|repeat| !repeat.is_empty()) else {
                    return kids[0];
                };
                let star = repeat.find('*');
                let exact = star.is_none();
                let min = if star.is_some_and(|star| star > 0) || exact {
                    js::parse_int(repeat, 10)
                } else {
                    0.0
                };
                let max = match star {
                    None => min,
                    Some(star) if star < repeat.len() - 1 => js::parse_int(&repeat[star + 1..], 10),
                    Some(_) => f64::INFINITY,
                };
                if min == 0.0 && max == 1.0 {
                    nodes.optional(kids[0])
                } else {
                    nodes.repetition(kids[0], min, max)
                }
            }
            AbnfPart::StringLiteral(value) | AbnfPart::NumVal(value) => {
                nodes.terminal(value.clone())
            }
            AbnfPart::RuleName(name) => nodes.nonterminal(name.clone()),
            AbnfPart::Group => kids[0],
            AbnfPart::OptionalGroup => nodes.optional(kids[0]),
        }
    }
}
