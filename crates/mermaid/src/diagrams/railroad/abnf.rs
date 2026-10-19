//! `railroad-abnf-beta`: ABNF as in RFC 5234, with each rule closed by a
//! semicolon. Pele's `diagrams/railroad/abnf.ts`.

use std::sync::LazyLock;

use super::{
    common::{
        Grammar, LETTERS, Syntax, SyntaxRule, acc_descr, acc_title, choice, hidden_tokens, inner,
        literal, nonterminal, opening, optional, parse_grammar, repetition, sequence, terminal,
        title, to_rules,
    },
    types::{NodeId, Nodes, RailroadRule},
};
use crate::{
    diagrams::common::tokens::{Reader, Table, TokenType},
    error::Error,
    js,
};

const EQUALS: i32 = 1;
const SEMICOLON: i32 = 2;
const SLASH: i32 = 3;
const GROUP: i32 = 4;
const OPTION: i32 = 6;
const TITLE: i32 = 8;
const NAME: i32 = 11;
const STRING: i32 = 12;
const NUMBER: i32 = 13;
const REPEAT: i32 = 14;
const EXACT: i32 = 15;
const DIGITS: &str = "0123456789";
const REPEAT_FIRST: &str = "0123456789*";
const NAME_NAME: &str = "ABNF_RULENAME";

pub static ABNF_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let mut types = vec![
        opening("railroad-abnf-beta", NAME_NAME),
        literal("="),
        // A semicolon closes a rule only at the end of its line; with anything
        // after it, it starts a comment.
        literal(";").longer(&["ABNF_COMMENT"]),
        literal("/"),
        literal("("),
        literal(")"),
        literal("["),
        literal("]"),
        title(),
        acc_title(),
        acc_descr(),
        TokenType::regex(NAME_NAME, r"[A-Za-z][A-Za-z0-9-]*", Some(LETTERS)),
        TokenType::regex("ABNF_STRING", r#""[^"]*""#, Some("\"")),
        TokenType::regex(
            "ABNF_NUMVAL",
            r"%[xXdDbB][0-9A-Fa-f]+(?:-[0-9A-Fa-f]+|\.[0-9A-Fa-f]+)*",
            Some("%"),
        ),
        TokenType::regex("ABNF_REPEAT", r"[0-9]*\*[0-9]*", Some(REPEAT_FIRST)),
        TokenType::regex("ABNF_EXACT_REPEAT", r"[0-9]+", Some(DIGITS)),
    ];
    types.extend(hidden_tokens("ABNF"));
    types.push(TokenType::regex("ABNF_COMMENT", r";[^\n\r]*", Some(";")).hidden());
    Table::new(types)
});

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

pub struct AbnfAst {
    pub grammar: Grammar,
    pub syntax: Syntax<AbnfPart>,
}

/// One open bracket, or the rule itself at the bottom of the stack.
struct Frame {
    alternation: usize,
    concatenation: usize,
    /// The bracket's node, its closing token, and the repeat written before
    /// it.
    node: Option<usize>,
    close: i32,
    repeat: Option<String>,
}

fn frame(
    syntax: &mut Syntax<AbnfPart>,
    node: Option<usize>,
    alternation: usize,
    close: i32,
    repeat: Option<String>,
) -> Frame {
    let concatenation = syntax.add(AbnfPart::Concatenation, Vec::new());
    syntax.parts[alternation].kids.push(concatenation);
    Frame {
        alternation,
        concatenation,
        node,
        close,
        repeat,
    }
}

fn alternation(r: &mut Reader, syntax: &mut Syntax<AbnfPart>) -> Result<usize, Error> {
    let mut stack: Vec<Frame> = Vec::new();
    let root = syntax.add(AbnfPart::Alternation, Vec::new());
    let mut top = frame(syntax, None, root, -2, None);
    loop {
        let mut repeat = if r.kind() == REPEAT || r.kind() == EXACT {
            Some(r.take().to_string())
        } else {
            None
        };
        let kind = r.kind();
        let mut primary = match kind {
            STRING => syntax.add(
                AbnfPart::StringLiteral(inner(r.take()).to_string()),
                Vec::new(),
            ),
            NUMBER => syntax.add(AbnfPart::NumVal(r.take().to_string()), Vec::new()),
            NAME => syntax.add(AbnfPart::RuleName(r.take().to_string()), Vec::new()),
            GROUP | OPTION => {
                r.i += 1;
                let element = syntax.add(AbnfPart::Alternation, Vec::new());
                let bracket = if kind == GROUP {
                    AbnfPart::Group
                } else {
                    AbnfPart::OptionalGroup
                };
                let bracket = syntax.add(bracket, vec![element]);
                let next = frame(syntax, Some(bracket), element, kind + 1, repeat);
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
            let k = r.kind();
            if k == top.close {
                r.i += 1;
                primary = top.node.unwrap_or_default();
                let parent = stack
                    .pop()
                    .unwrap_or_else(|| unreachable!("a bracket has a parent"));
                repeat = std::mem::replace(&mut top, parent).repeat;
                continue;
            }
            if k == SLASH {
                r.i += 1;
                top.concatenation = syntax.add(AbnfPart::Concatenation, Vec::new());
                syntax.parts[top.alternation].kids.push(top.concatenation);
            } else if !matches!(k, STRING | NUMBER | NAME | GROUP | OPTION | REPEAT | EXACT) {
                if stack.is_empty() {
                    return Ok(top.alternation);
                }
                return Err(r.fail(&format!(
                    "token of type '{}'",
                    ABNF_TOKENS.types[top.close as usize].name
                )));
            }
            break;
        }
    }
}

pub fn parse_abnf(src: &str) -> Result<AbnfAst, Error> {
    let mut syntax = Syntax::default();
    let grammar = parse_grammar(
        src,
        &ABNF_TOKENS,
        TITLE,
        |text| inner(text).to_string(),
        |r| {
            let name = r.expect(NAME)?.to_string();
            r.expect(EQUALS)?;
            let definition = alternation(r, &mut syntax)?;
            r.expect(SEMICOLON)?;
            Ok(SyntaxRule { name, definition })
        },
    )?;
    Ok(AbnfAst { grammar, syntax })
}

fn node(part: &AbnfPart, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
    match part {
        AbnfPart::Alternation => choice(nodes, kids),
        AbnfPart::Concatenation => sequence(nodes, kids),
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
                optional(nodes, kids[0])
            } else {
                repetition(nodes, kids[0], min, max)
            }
        }
        AbnfPart::StringLiteral(value) | AbnfPart::NumVal(value) => terminal(nodes, value.clone()),
        AbnfPart::RuleName(name) => nonterminal(nodes, name.clone()),
        AbnfPart::Group => kids[0],
        AbnfPart::OptionalGroup => optional(nodes, kids[0]),
    }
}

pub fn abnf_rules(ast: &AbnfAst, nodes: &mut Nodes) -> Vec<RailroadRule> {
    to_rules(&ast.grammar, &ast.syntax, nodes, node)
}
