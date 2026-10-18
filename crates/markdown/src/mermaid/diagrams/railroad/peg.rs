//! `railroad-peg-beta`: parsing expression grammars. Pele's
//! `diagrams/railroad/peg.ts`.

use std::sync::LazyLock;

use super::{
    common::{
        Grammar, Syntax, SyntaxRule, acc_descr, acc_title, choice, hidden_tokens, identifier,
        literal, nonterminal, opening, optional, parse_grammar, quoted, repetition, sequence,
        special, terminal, title, to_rules, unescape,
    },
    types::{NodeId, Nodes, RailroadNode, RailroadRule},
};
use crate::mermaid::{
    diagrams::common::tokens::{Reader, Table, TokenType},
    error::Error,
};

const ARROW: i32 = 1;
const SEMICOLON: i32 = 2;
const SLASH: i32 = 3;
const AND: i32 = 4;
const NOT: i32 = 5;
const QUESTION: i32 = 6;
const STAR: i32 = 7;
const PLUS: i32 = 8;
const GROUP: i32 = 9;
const GROUP_END: i32 = 10;
const DOT: i32 = 11;
const TITLE: i32 = 12;
const ID: i32 = 15;
const STRING: i32 = 16;
const ID_NAME: &str = "PEG_ID";

pub static PEG_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let mut types = vec![
        opening("railroad-peg-beta", ID_NAME),
        literal("<-"),
        literal(";"),
        literal("/"),
        literal("&"),
        literal("!"),
        literal("?"),
        literal("*"),
        literal("+"),
        literal("("),
        literal(")"),
        literal("."),
        title(),
        acc_title(),
        acc_descr(),
        identifier(ID_NAME),
        quoted("PEG_STRING"),
    ];
    types.extend(hidden_tokens("PEG"));
    types.push(TokenType::regex("PEG_LINE_COMMENT", r"#[^\n\r]*", Some("#")).hidden());
    Table::new(types)
});

pub enum PegPart {
    OrderedChoice,
    Sequence,
    Prefix(Option<String>),
    Suffix(Option<String>),
    Literal(String),
    Identifier(String),
    Any(String),
    Group,
}

pub struct PegAst {
    pub grammar: Grammar,
    pub syntax: Syntax<PegPart>,
}

/// One open group, or the rule itself at the bottom of the stack.
struct Frame {
    choice: usize,
    sequence: usize,
    /// The group's node and the predicate written before it.
    node: Option<usize>,
    operator: Option<String>,
}

fn frame(
    syntax: &mut Syntax<PegPart>,
    node: Option<usize>,
    choice: usize,
    operator: Option<String>,
) -> Frame {
    let sequence = syntax.add(PegPart::Sequence, Vec::new());
    syntax.parts[choice].kids.push(sequence);
    Frame {
        choice,
        sequence,
        node,
        operator,
    }
}

fn definition(r: &mut Reader, syntax: &mut Syntax<PegPart>) -> Result<usize, Error> {
    let mut stack: Vec<Frame> = Vec::new();
    let root = syntax.add(PegPart::OrderedChoice, Vec::new());
    let mut top = frame(syntax, None, root, None);
    loop {
        let mut operator = if r.kind() == AND || r.kind() == NOT {
            Some(r.take().to_string())
        } else {
            None
        };
        let mut primary = match r.kind() {
            STRING => syntax.add(PegPart::Literal(unescape(r.take())), Vec::new()),
            ID => syntax.add(PegPart::Identifier(r.take().to_string()), Vec::new()),
            DOT => syntax.add(PegPart::Any(r.take().to_string()), Vec::new()),
            GROUP => {
                r.i += 1;
                let element = syntax.add(PegPart::OrderedChoice, Vec::new());
                let group = syntax.add(PegPart::Group, vec![element]);
                let next = frame(syntax, Some(group), element, operator);
                stack.push(std::mem::replace(&mut top, next));
                continue;
            }
            _ => {
                return Err(r.fail("a literal, a name, a dot, or an opening parenthesis"));
            }
        };
        loop {
            let suffix_operator = if matches!(r.kind(), QUESTION | STAR | PLUS) {
                Some(r.take().to_string())
            } else {
                None
            };
            let suffix = syntax.add(PegPart::Suffix(suffix_operator), vec![primary]);
            let prefix = syntax.add(PegPart::Prefix(operator.take()), vec![suffix]);
            syntax.parts[top.sequence].kids.push(prefix);
            let k = r.kind();
            if k == GROUP_END && !stack.is_empty() {
                r.i += 1;
                primary = top.node.unwrap_or_default();
                let parent = stack
                    .pop()
                    .unwrap_or_else(|| unreachable!("a group has a parent"));
                operator = std::mem::replace(&mut top, parent).operator;
                continue;
            }
            if k == SLASH {
                r.i += 1;
                top.sequence = syntax.add(PegPart::Sequence, Vec::new());
                syntax.parts[top.choice].kids.push(top.sequence);
            } else if !matches!(k, AND | NOT | STRING | ID | DOT | GROUP) {
                if stack.is_empty() {
                    return Ok(top.choice);
                }
                return Err(r.fail("token of type ')'"));
            }
            break;
        }
    }
}

pub fn parse_peg(src: &str) -> Result<PegAst, Error> {
    let mut syntax = Syntax::default();
    let grammar = parse_grammar(src, &PEG_TOKENS, TITLE, unescape, |r| {
        let name = r.expect(ID)?.to_string();
        r.expect(ARROW)?;
        let definition = definition(r, &mut syntax)?;
        r.expect(SEMICOLON)?;
        Ok(SyntaxRule { name, definition })
    })?;
    Ok(PegAst { grammar, syntax })
}

/// What a predicate shows of the expression it looks ahead for.
fn label(node: &RailroadNode) -> String {
    match node {
        RailroadNode::Terminal { value } => format!("\"{value}\""),
        RailroadNode::NonTerminal { name } => name.clone(),
        RailroadNode::Special { text } => text.clone(),
        _ => "(...)".into(),
    }
}

fn node(part: &PegPart, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
    let kid = kids.first().copied().unwrap_or_default();
    match part {
        PegPart::OrderedChoice => choice(nodes, kids),
        PegPart::Sequence => sequence(nodes, kids),
        PegPart::Prefix(Some(operator)) if !operator.is_empty() => {
            let text = format!("{operator}{}", label(nodes.get(kid)));
            special(nodes, text)
        }
        PegPart::Prefix(_) => kid,
        PegPart::Suffix(Some(operator)) if !operator.is_empty() => {
            if operator == "?" {
                optional(nodes, kid)
            } else {
                let min = if operator == "+" { 1.0 } else { 0.0 };
                repetition(nodes, kid, min, f64::INFINITY)
            }
        }
        PegPart::Suffix(_) => kid,
        PegPart::Literal(value) => terminal(nodes, value.clone()),
        PegPart::Identifier(name) => nonterminal(nodes, name.clone()),
        PegPart::Any(dot) => special(nodes, dot.clone()),
        PegPart::Group => kid,
    }
}

pub fn peg_rules(ast: &PegAst, nodes: &mut Nodes) -> Vec<RailroadRule> {
    to_rules(&ast.grammar, &ast.syntax, nodes, node)
}
