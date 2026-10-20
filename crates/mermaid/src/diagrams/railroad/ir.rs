//! `railroad-beta`: Mermaid's own notation, one constructor call per node.
//! Pele's `diagrams/railroad/ir.ts`.

use super::{
    common::{
        Grammar, Parser, SyntaxRule, ToNode, acc_descr, acc_title, blank, identifier, quoted,
        title, unescape,
    },
    types::{NodeId, Nodes},
};
use crate::{
    diagrams::common::tokens::{
        TokenType, directive, keyword, single_line_comment, token_kinds, yaml,
    },
    error::Error,
};

token_kinds! {
    enum Token {
        Keyword = keyword("railroad-beta").longer(&[Token::Id]),
        Nonterminal = word("nonterminal"),
        ZeroOrMore = word("zeroOrMore"),
        OneOrMore = word("oneOrMore"),
        Sequence = word("sequence"),
        Optional = word("optional"),
        Terminal = word("terminal"),
        Special = word("special"),
        Choice = word("choice"),
        Equals = TokenType::literal("="),
        Semicolon = TokenType::literal(";"),
        Open = TokenType::literal("("),
        Comma = TokenType::literal(","),
        Close = TokenType::literal(")"),
        Title = title(),
        AccTitle = acc_title(),
        AccDescr = acc_descr(),
        Id = identifier("RR_ID"),
        String = quoted("RR_STRING"),
        Whitespace = blank(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        BlockComment = TokenType::regex("RR_BLOCK_COMMENT", r"\/\*[\s\S]*?\*\/", Some("/"))
            .hidden()
            .opener(r"\/\*"),
    }
}

fn word(name: &'static str) -> TokenType<Token> {
    TokenType::literal(name).longer(&[Token::Id])
}

pub enum IrExpression {
    Sequence,
    Choice,
    Optional,
    OneOrMore,
    ZeroOrMore,
    Terminal(String),
    NonTerminal(String),
    Special(String),
}

impl Parser<'_, Token, IrExpression> {
    fn rule(&mut self) -> Result<SyntaxRule, Error> {
        let name = self.r.expect(Token::Id)?.to_string();
        self.r.expect(Token::Equals)?;
        let definition = self.expression()?;
        self.r.expect(Token::Semicolon)?;
        Ok(SyntaxRule { name, definition })
    }

    /// The quoted argument of the constructor here.
    fn argument(&mut self) -> Result<String, Error> {
        self.r.bump();
        self.r.expect(Token::Open)?;
        let value = unescape(self.r.expect(Token::String)?);
        self.r.expect(Token::Close)?;
        Ok(value)
    }

    fn expression(&mut self) -> Result<usize, Error> {
        let mut open: Vec<usize> = Vec::new();
        loop {
            let kind = match self.r.kind() {
                Some(Token::Terminal) => IrExpression::Terminal(self.argument()?),
                Some(Token::Nonterminal) => IrExpression::NonTerminal(self.argument()?),
                Some(Token::Special) => IrExpression::Special(self.argument()?),
                Some(Token::Sequence) => IrExpression::Sequence,
                Some(Token::Choice) => IrExpression::Choice,
                Some(Token::Optional) => IrExpression::Optional,
                Some(Token::OneOrMore) => IrExpression::OneOrMore,
                Some(Token::ZeroOrMore) => IrExpression::ZeroOrMore,
                _ => return Err(self.r.fail("an expression")),
            };
            let leaf = matches!(
                kind,
                IrExpression::Terminal(_) | IrExpression::NonTerminal(_) | IrExpression::Special(_)
            );
            let mut value = self.syntax.add(kind, Vec::new());
            if !leaf {
                self.r.bump();
                self.r.expect(Token::Open)?;
                open.push(value);
                continue;
            }
            loop {
                let Some(&parent) = open.last() else {
                    return Ok(value);
                };
                let part = &mut self.syntax.parts[parent];
                if matches!(part.kind, IrExpression::Sequence | IrExpression::Choice) {
                    part.kids.push(value);
                    if self.r.accept(Token::Comma) {
                        break;
                    }
                } else {
                    part.kids = vec![value];
                }
                self.r.expect(Token::Close)?;
                open.pop();
                value = parent;
            }
        }
    }
}

pub fn parse_ir(src: &str) -> Result<Grammar<IrExpression>, Error> {
    Parser::parse(src, Token::Keyword, unescape, Parser::rule)
}

impl ToNode for IrExpression {
    fn to_node(&self, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
        match self {
            IrExpression::Terminal(value) => nodes.terminal(value.clone()),
            IrExpression::NonTerminal(name) => nodes.nonterminal(name.clone()),
            IrExpression::Special(text) => nodes.special(text.clone()),
            IrExpression::Sequence => nodes.sequence(kids),
            IrExpression::Choice => nodes.choice(kids),
            IrExpression::Optional => nodes.optional(kids[0]),
            IrExpression::OneOrMore => nodes.repetition(kids[0], 1.0, f64::INFINITY),
            IrExpression::ZeroOrMore => nodes.repetition(kids[0], 0.0, f64::INFINITY),
        }
    }
}
