//! `railroad-beta`: Mermaid's own notation, one constructor call per node.
//! Pele's `diagrams/railroad/ir.ts`.

use std::sync::LazyLock;

use super::{
    common::{
        Grammar, Syntax, SyntaxRule, acc_descr, acc_title, choice, hidden_tokens, identifier,
        literal, nonterminal, opening, optional, parse_grammar, quoted, repetition, sequence,
        special, terminal, title, to_rules, unescape,
    },
    types::{NodeId, Nodes, RailroadRule},
};
use crate::mermaid::{
    diagrams::common::tokens::{Reader, Table, TokenType},
    error::Error,
};

const NONTERMINAL: i32 = 1;
const ZERO_OR_MORE: i32 = 2;
const ONE_OR_MORE: i32 = 3;
const SEQUENCE: i32 = 4;
const OPTIONAL: i32 = 5;
const TERMINAL: i32 = 6;
const SPECIAL: i32 = 7;
const CHOICE: i32 = 8;
const EQUALS: i32 = 9;
const SEMICOLON: i32 = 10;
const OPEN: i32 = 11;
const COMMA: i32 = 12;
const CLOSE: i32 = 13;
const TITLE: i32 = 14;
const ID: i32 = 17;
const STRING: i32 = 18;
const ID_NAME: &str = "RR_ID";

pub static IR_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let word = |name: &'static str| literal(name).longer(&[ID_NAME]);
    let mut types = vec![
        opening("railroad-beta", ID_NAME),
        word("nonterminal"),
        word("zeroOrMore"),
        word("oneOrMore"),
        word("sequence"),
        word("optional"),
        word("terminal"),
        word("special"),
        word("choice"),
        literal("="),
        literal(";"),
        literal("("),
        literal(","),
        literal(")"),
        title(),
        acc_title(),
        acc_descr(),
        identifier(ID_NAME),
        quoted("RR_STRING"),
    ];
    types.extend(hidden_tokens("RR"));
    types.push(
        TokenType::regex("RR_BLOCK_COMMENT", r"\/\*[\s\S]*?\*\/", Some("/"))
            .hidden()
            .opener(r"\/\*"),
    );
    Table::new(types)
});

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

pub struct IrAst {
    pub grammar: Grammar,
    pub syntax: Syntax<IrExpression>,
}

fn argument(r: &mut Reader) -> Result<String, Error> {
    r.i += 1;
    r.expect(OPEN)?;
    let value = unescape(r.expect(STRING)?);
    r.expect(CLOSE)?;
    Ok(value)
}

fn expression(r: &mut Reader, syntax: &mut Syntax<IrExpression>) -> Result<usize, Error> {
    let mut open: Vec<usize> = Vec::new();
    loop {
        let kind = match r.kind() {
            TERMINAL => IrExpression::Terminal(argument(r)?),
            NONTERMINAL => IrExpression::NonTerminal(argument(r)?),
            SPECIAL => IrExpression::Special(argument(r)?),
            SEQUENCE => IrExpression::Sequence,
            CHOICE => IrExpression::Choice,
            OPTIONAL => IrExpression::Optional,
            ONE_OR_MORE => IrExpression::OneOrMore,
            ZERO_OR_MORE => IrExpression::ZeroOrMore,
            _ => return Err(r.fail("an expression")),
        };
        let leaf = matches!(
            kind,
            IrExpression::Terminal(_) | IrExpression::NonTerminal(_) | IrExpression::Special(_)
        );
        let mut value = syntax.add(kind, Vec::new());
        if !leaf {
            r.i += 1;
            r.expect(OPEN)?;
            open.push(value);
            continue;
        }
        loop {
            let Some(&parent) = open.last() else {
                return Ok(value);
            };
            let part = &mut syntax.parts[parent];
            if matches!(part.kind, IrExpression::Sequence | IrExpression::Choice) {
                part.kids.push(value);
                if r.accept(COMMA) {
                    break;
                }
            } else {
                part.kids = vec![value];
            }
            r.expect(CLOSE)?;
            open.pop();
            value = parent;
        }
    }
}

pub fn parse_ir(src: &str) -> Result<IrAst, Error> {
    let mut syntax = Syntax::default();
    let grammar = parse_grammar(src, &IR_TOKENS, TITLE, unescape, |r| {
        let name = r.expect(ID)?.to_string();
        r.expect(EQUALS)?;
        let definition = expression(r, &mut syntax)?;
        r.expect(SEMICOLON)?;
        Ok(SyntaxRule { name, definition })
    })?;
    Ok(IrAst { grammar, syntax })
}

fn node(e: &IrExpression, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
    match e {
        IrExpression::Terminal(value) => terminal(nodes, value.clone()),
        IrExpression::NonTerminal(name) => nonterminal(nodes, name.clone()),
        IrExpression::Special(text) => special(nodes, text.clone()),
        IrExpression::Sequence => sequence(nodes, kids),
        IrExpression::Choice => choice(nodes, kids),
        IrExpression::Optional => optional(nodes, kids[0]),
        IrExpression::OneOrMore => repetition(nodes, kids[0], 1.0, f64::INFINITY),
        IrExpression::ZeroOrMore => repetition(nodes, kids[0], 0.0, f64::INFINITY),
    }
}

pub fn ir_rules(ast: &IrAst, nodes: &mut Nodes) -> Vec<RailroadRule> {
    to_rules(&ast.grammar, &ast.syntax, nodes, node)
}
