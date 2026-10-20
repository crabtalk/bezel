//! `railroad-ebnf-beta`: EBNF in the W3C and ISO 14977 styles. Pele's
//! `diagrams/railroad/ebnf.ts`.

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
    js,
    util::chars::{inner, is_space},
};

token_kinds! {
    enum Token {
        Keyword = keyword("railroad-ebnf-beta").longer(&[Token::Id]),
        Defines = TokenType::literal("::="),
        Equals = TokenType::literal("="),
        Semicolon = TokenType::literal(";"),
        Bar = TokenType::literal("|"),
        Comma = TokenType::literal(","),
        Group = TokenType::literal("(").longer(&[Token::IsoComment]),
        GroupEnd = TokenType::literal(")"),
        Optional = TokenType::literal("["),
        OptionalEnd = TokenType::literal("]"),
        Repeat = TokenType::literal("{"),
        RepeatEnd = TokenType::literal("}"),
        Question = TokenType::literal("?").longer(&[Token::Special]),
        Star = TokenType::literal("*"),
        Plus = TokenType::literal("+"),
        Minus = TokenType::literal("-").longer(&[Token::Yaml]),
        Title = title(),
        AccTitle = acc_title(),
        AccDescr = acc_descr(),
        Id = identifier("EBNF_ID"),
        String = quoted("EBNF_STRING"),
        Special = TokenType::regex(
            "EBNF_SPECIAL_SEQUENCE",
            r"\?(?=[^?;]*[^?\s;][^?;]*\?)[^?;]*\?",
            Some("?"),
        )
        .matcher(special_sequence),
        Whitespace = blank(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        BlockComment = TokenType::regex("EBNF_BLOCK_COMMENT", r"\/\*[\s\S]*?\*\/", Some("/"))
            .hidden()
            .opener(r"\/\*"),
        IsoComment = TokenType::regex("EBNF_ISO_COMMENT", r"\(\*[\s\S]*?\*\)", Some("("))
            .hidden()
            .opener(r"\(\*"),
    }
}

/// A question mark, text with something other than blanks in it and no `?`
/// or `;`, and a question mark. Mermaid's pattern looks ahead with two nested
/// runs, which rescans; this reads the text once.
fn special_sequence(src: &str, at: usize) -> Option<usize> {
    let body = src[at..].strip_prefix('?')?;
    let mut solid = false;
    for (i, c) in body.char_indices() {
        match c {
            '?' => return solid.then_some(at + 1 + i + 1),
            ';' => return None,
            _ => solid = solid || !is_space(c),
        }
    }
    None
}

pub enum EbnfPostfix {
    Optional,
    ZeroOrMore,
    OneOrMore,
    /// Its right side is the term's next part after the base.
    Exception,
}

/// A term's parts are its base and the right side of each exception.
pub enum EbnfPart {
    Choice,
    Sequence,
    Term(Vec<EbnfPostfix>),
    Terminal(String),
    NonTerminal(String),
    Special(String),
    Group,
    Optional,
    Repetition,
}

/// One open bracket, or the rule itself at the bottom of the stack.
struct Frame {
    choice: usize,
    sequence: usize,
    term: Option<usize>,
    /// The bracket's node, its closing token, and whether it is the right
    /// side of an exception.
    node: Option<usize>,
    close: Option<Token>,
    except: bool,
}

impl Frame {
    fn open(
        syntax: &mut Syntax<EbnfPart>,
        node: Option<usize>,
        choice: usize,
        close: Option<Token>,
        except: bool,
    ) -> Self {
        let sequence = syntax.add(EbnfPart::Sequence, Vec::new());
        syntax.parts[choice].kids.push(sequence);
        Self {
            choice,
            sequence,
            term: None,
            node,
            close,
            except,
        }
    }
}

impl Parser<'_, Token, EbnfPart> {
    fn rule(&mut self) -> Result<SyntaxRule, Error> {
        let name = self.r.expect(Token::Id)?.to_string();
        if !self.r.accept(Token::Equals) {
            self.r.expect(Token::Defines)?;
        }
        let definition = self.definition()?;
        self.r.expect(Token::Semicolon)?;
        Ok(SyntaxRule { name, definition })
    }

    fn definition(&mut self) -> Result<usize, Error> {
        let syntax = &mut self.syntax;
        let r = &mut self.r;
        let mut stack: Vec<Frame> = Vec::new();
        let root = syntax.add(EbnfPart::Choice, Vec::new());
        let mut top = Frame::open(syntax, None, root, None, false);
        let mut except = false;
        loop {
            let mut primary = match r.kind() {
                Some(Token::String) => {
                    syntax.add(EbnfPart::Terminal(unescape(r.take())), Vec::new())
                }
                Some(Token::Id) => {
                    syntax.add(EbnfPart::NonTerminal(r.take().to_string()), Vec::new())
                }
                Some(Token::Special) => {
                    let text = js::trim(inner(r.take())).to_string();
                    syntax.add(EbnfPart::Special(text), Vec::new())
                }
                Some(kind @ (Token::Group | Token::Optional | Token::Repeat)) => {
                    r.bump();
                    let element = syntax.add(EbnfPart::Choice, Vec::new());
                    let (bracket, close) = match kind {
                        Token::Group => (EbnfPart::Group, Token::GroupEnd),
                        Token::Optional => (EbnfPart::Optional, Token::OptionalEnd),
                        _ => (EbnfPart::Repetition, Token::RepeatEnd),
                    };
                    let bracket = syntax.add(bracket, vec![element]);
                    let next = Frame::open(syntax, Some(bracket), element, Some(close), except);
                    stack.push(std::mem::replace(&mut top, next));
                    except = false;
                    continue;
                }
                _ => return Err(r.fail("a terminal, a name, or an opening bracket")),
            };
            loop {
                let term = match top.term {
                    Some(term) if except => {
                        let part = &mut syntax.parts[term];
                        if let EbnfPart::Term(postfixes) = &mut part.kind {
                            postfixes.push(EbnfPostfix::Exception);
                        }
                        part.kids.push(primary);
                        except = false;
                        term
                    }
                    _ => {
                        let term = syntax.add(EbnfPart::Term(Vec::new()), vec![primary]);
                        top.term = Some(term);
                        syntax.parts[top.sequence].kids.push(term);
                        term
                    }
                };
                loop {
                    let postfix = match r.kind() {
                        Some(Token::Question) => EbnfPostfix::Optional,
                        Some(Token::Star) => EbnfPostfix::ZeroOrMore,
                        Some(Token::Plus) => EbnfPostfix::OneOrMore,
                        _ => break,
                    };
                    r.bump();
                    if let EbnfPart::Term(postfixes) = &mut syntax.parts[term].kind {
                        postfixes.push(postfix);
                    }
                }
                if top.close.is_some_and(|close| r.at(close)) {
                    r.bump();
                    primary = top.node.unwrap_or_default();
                    except = top.except;
                    top = stack
                        .pop()
                        .unwrap_or_else(|| unreachable!("a bracket has a parent"));
                    continue;
                }
                match r.kind() {
                    Some(Token::Minus) => {
                        r.bump();
                        except = true;
                    }
                    Some(Token::Comma) => r.bump(),
                    Some(Token::Bar) => {
                        r.bump();
                        top.sequence = syntax.add(EbnfPart::Sequence, Vec::new());
                        syntax.parts[top.choice].kids.push(top.sequence);
                    }
                    Some(
                        Token::String
                        | Token::Id
                        | Token::Special
                        | Token::Group
                        | Token::Optional
                        | Token::Repeat,
                    ) => {}
                    _ => {
                        return match top.close {
                            Some(close) => Err(r.expected(close)),
                            None => Ok(top.choice),
                        };
                    }
                }
                break;
            }
        }
    }
}

pub fn parse_ebnf(src: &str) -> Result<Grammar<EbnfPart>, Error> {
    Parser::parse(src, Token::Keyword, unescape, Parser::rule)
}

impl ToNode for EbnfPart {
    fn to_node(&self, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
        match self {
            EbnfPart::Choice => nodes.choice(kids),
            EbnfPart::Sequence => nodes.sequence(kids),
            EbnfPart::Term(postfixes) => {
                // Each postfix operator applies to everything before it in the
                // term.
                let mut result = kids[0];
                let mut next = 1;
                for postfix in postfixes {
                    result = match postfix {
                        EbnfPostfix::Optional => nodes.optional(result),
                        EbnfPostfix::Exception => {
                            let minus = nodes.terminal("-".into());
                            let except = kids[next];
                            next += 1;
                            nodes.add(RailroadNode::Sequence {
                                elements: vec![result, minus, except],
                            })
                        }
                        EbnfPostfix::OneOrMore => nodes.repetition(result, 1.0, f64::INFINITY),
                        EbnfPostfix::ZeroOrMore => nodes.repetition(result, 0.0, f64::INFINITY),
                    };
                }
                result
            }
            EbnfPart::Terminal(value) => nodes.terminal(value.clone()),
            EbnfPart::NonTerminal(name) => nodes.nonterminal(name.clone()),
            EbnfPart::Special(text) => nodes.special(text.clone()),
            EbnfPart::Group => kids[0],
            EbnfPart::Optional => nodes.optional(kids[0]),
            EbnfPart::Repetition => nodes.repetition(kids[0], 0.0, f64::INFINITY),
        }
    }
}
