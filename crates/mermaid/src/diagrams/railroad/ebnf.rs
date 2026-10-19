//! `railroad-ebnf-beta`: EBNF in the W3C and ISO 14977 styles. Pele's
//! `diagrams/railroad/ebnf.ts`.

use std::sync::LazyLock;

use super::{
    common::{
        Grammar, Syntax, SyntaxRule, acc_descr, acc_title, choice, hidden_tokens, identifier,
        inner, literal, nonterminal, opening, optional, parse_grammar, quoted, repetition,
        sequence, special, terminal, title, to_rules, unescape,
    },
    types::{NodeId, Nodes, RailroadNode, RailroadRule},
};
use crate::{
    diagrams::common::tokens::{Reader, Table, TokenType},
    error::Error,
    js,
    util::chars::{code_at, is_space, width_at},
};

const DEFINES: i32 = 1;
const EQUALS: i32 = 2;
const SEMICOLON: i32 = 3;
const BAR: i32 = 4;
const COMMA: i32 = 5;
const GROUP: i32 = 6;
const OPTION: i32 = 8;
const REPEAT: i32 = 10;
const QUESTION: i32 = 12;
const STAR: i32 = 13;
const PLUS: i32 = 14;
const MINUS: i32 = 15;
const TITLE: i32 = 16;
const ID: i32 = 19;
const STRING: i32 = 20;
const SPECIAL: i32 = 21;
const ID_NAME: &str = "EBNF_ID";
const SPECIAL_NAME: &str = "EBNF_SPECIAL_SEQUENCE";
const ISO_COMMENT_NAME: &str = "EBNF_ISO_COMMENT";

pub static EBNF_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let iso_comment = || {
        TokenType::regex(ISO_COMMENT_NAME, r"\(\*[\s\S]*?\*\)", Some("("))
            .hidden()
            .opener(r"\(\*")
    };
    let mut types = vec![
        opening("railroad-ebnf-beta", ID_NAME),
        literal("::="),
        literal("="),
        literal(";"),
        literal("|"),
        literal(","),
        literal("(").longer(&[ISO_COMMENT_NAME]),
        literal(")"),
        literal("["),
        literal("]"),
        literal("{"),
        literal("}"),
        literal("?").longer(&[SPECIAL_NAME]),
        literal("*"),
        literal("+"),
        literal("-").longer(&["EBNF_YAML"]),
        title(),
        acc_title(),
        acc_descr(),
        identifier(ID_NAME),
        quoted("EBNF_STRING"),
        TokenType::regex(
            SPECIAL_NAME,
            r"\?(?=[^?;]*[^?\s;][^?;]*\?)[^?;]*\?",
            Some("?"),
        )
        .matcher(special_sequence),
    ];
    types.extend(hidden_tokens("EBNF"));
    types.push(
        TokenType::regex("EBNF_BLOCK_COMMENT", r"\/\*[\s\S]*?\*\/", Some("/"))
            .hidden()
            .opener(r"\/\*"),
    );
    types.push(iso_comment());
    Table::new(types)
});

/// A question mark, text with something other than blanks in it and no `?`
/// or `;`, and a question mark. Mermaid's pattern looks ahead with two nested
/// runs, which rescans; this reads the text once.
fn special_sequence(src: &str, at: usize) -> Option<usize> {
    if code_at(src, at) != 63 {
        return None;
    }
    let mut solid = false;
    let mut i = at + 1;
    while i < src.len() {
        let c = code_at(src, i);
        if c == 63 {
            return solid.then_some(i + 1);
        }
        if c == 59 {
            return None;
        }
        solid = solid || !is_space(c);
        i += width_at(src, i);
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

pub struct EbnfAst {
    pub grammar: Grammar,
    pub syntax: Syntax<EbnfPart>,
}

/// One open bracket, or the rule itself at the bottom of the stack.
struct Frame {
    choice: usize,
    sequence: usize,
    term: Option<usize>,
    /// The bracket's node, its closing token, and whether it is the right
    /// side of an exception.
    node: Option<usize>,
    close: i32,
    except: bool,
}

fn frame(
    syntax: &mut Syntax<EbnfPart>,
    node: Option<usize>,
    choice: usize,
    close: i32,
    except: bool,
) -> Frame {
    let sequence = syntax.add(EbnfPart::Sequence, Vec::new());
    syntax.parts[choice].kids.push(sequence);
    Frame {
        choice,
        sequence,
        term: None,
        node,
        close,
        except,
    }
}

fn definition(r: &mut Reader, syntax: &mut Syntax<EbnfPart>) -> Result<usize, Error> {
    let mut stack: Vec<Frame> = Vec::new();
    let root = syntax.add(EbnfPart::Choice, Vec::new());
    let mut top = frame(syntax, None, root, -2, false);
    let mut except = false;
    loop {
        let kind = r.kind();
        let mut primary = match kind {
            STRING => syntax.add(EbnfPart::Terminal(unescape(r.take())), Vec::new()),
            ID => syntax.add(EbnfPart::NonTerminal(r.take().to_string()), Vec::new()),
            SPECIAL => {
                let text = js::trim(inner(r.take())).to_string();
                syntax.add(EbnfPart::Special(text), Vec::new())
            }
            GROUP | OPTION | REPEAT => {
                r.i += 1;
                let element = syntax.add(EbnfPart::Choice, Vec::new());
                let bracket = match (kind - GROUP) >> 1 {
                    0 => EbnfPart::Group,
                    1 => EbnfPart::Optional,
                    _ => EbnfPart::Repetition,
                };
                let bracket = syntax.add(bracket, vec![element]);
                let next = frame(syntax, Some(bracket), element, kind + 1, except);
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
                    QUESTION => EbnfPostfix::Optional,
                    STAR => EbnfPostfix::ZeroOrMore,
                    PLUS => EbnfPostfix::OneOrMore,
                    _ => break,
                };
                r.i += 1;
                if let EbnfPart::Term(postfixes) = &mut syntax.parts[term].kind {
                    postfixes.push(postfix);
                }
            }
            let k = r.kind();
            if k == top.close {
                r.i += 1;
                primary = top.node.unwrap_or_default();
                except = top.except;
                top = stack
                    .pop()
                    .unwrap_or_else(|| unreachable!("a bracket has a parent"));
                continue;
            }
            if k == MINUS {
                r.i += 1;
                except = true;
            } else if k == COMMA {
                r.i += 1;
            } else if k == BAR {
                r.i += 1;
                top.sequence = syntax.add(EbnfPart::Sequence, Vec::new());
                syntax.parts[top.choice].kids.push(top.sequence);
            } else if !matches!(k, STRING | ID | SPECIAL | GROUP | OPTION | REPEAT) {
                if stack.is_empty() {
                    return Ok(top.choice);
                }
                return Err(r.fail(&format!(
                    "token of type '{}'",
                    EBNF_TOKENS.types[top.close as usize].name
                )));
            }
            break;
        }
    }
}

pub fn parse_ebnf(src: &str) -> Result<EbnfAst, Error> {
    let mut syntax = Syntax::default();
    let grammar = parse_grammar(src, &EBNF_TOKENS, TITLE, unescape, |r| {
        let name = r.expect(ID)?.to_string();
        if !r.accept(EQUALS) {
            r.expect(DEFINES)?;
        }
        let definition = definition(r, &mut syntax)?;
        r.expect(SEMICOLON)?;
        Ok(SyntaxRule { name, definition })
    })?;
    Ok(EbnfAst { grammar, syntax })
}

fn node(part: &EbnfPart, kids: Vec<NodeId>, nodes: &mut Nodes) -> NodeId {
    match part {
        EbnfPart::Choice => choice(nodes, kids),
        EbnfPart::Sequence => sequence(nodes, kids),
        EbnfPart::Term(postfixes) => {
            // Each postfix operator applies to everything before it in the
            // term.
            let mut result = kids[0];
            let mut next = 1;
            for postfix in postfixes {
                result = match postfix {
                    EbnfPostfix::Optional => optional(nodes, result),
                    EbnfPostfix::Exception => {
                        let minus = terminal(nodes, "-".into());
                        let except = kids[next];
                        next += 1;
                        nodes.add(RailroadNode::Sequence {
                            elements: vec![result, minus, except],
                        })
                    }
                    EbnfPostfix::OneOrMore => repetition(nodes, result, 1.0, f64::INFINITY),
                    EbnfPostfix::ZeroOrMore => repetition(nodes, result, 0.0, f64::INFINITY),
                };
            }
            result
        }
        EbnfPart::Terminal(value) => terminal(nodes, value.clone()),
        EbnfPart::NonTerminal(name) => nonterminal(nodes, name.clone()),
        EbnfPart::Special(text) => special(nodes, text.clone()),
        EbnfPart::Group => kids[0],
        EbnfPart::Optional => optional(nodes, kids[0]),
        EbnfPart::Repetition => repetition(nodes, kids[0], 0.0, f64::INFINITY),
    }
}

pub fn ebnf_rules(ast: &EbnfAst, nodes: &mut Nodes) -> Vec<RailroadRule> {
    to_rules(&ast.grammar, &ast.syntax, nodes, node)
}
