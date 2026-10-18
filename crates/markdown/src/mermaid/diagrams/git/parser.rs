//! Pele's `diagrams/git/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Table, TokenType, acc_descr, acc_descr_value, acc_title, acc_title_value,
        directive, keyword, newline, single_line_comment, string, string_value, title, title_value,
        tokenize, whitespace, yaml,
    },
    error::Error,
    js,
};

const CHERRY_PICK: i32 = 0;
const GIT_GRAPH_COLON: i32 = 1;
const HIGHLIGHT: i32 = 2;
const GIT_GRAPH: i32 = 3;
const CHECKOUT: i32 = 4;
const REVERSE: i32 = 5;
const PARENT: i32 = 6;
const COMMIT: i32 = 7;
const NORMAL: i32 = 8;
const BRANCH: i32 = 9;
const ORDER: i32 = 10;
const SWITCH: i32 = 11;
const TYPE: i32 = 12;
const MERGE: i32 = 13;
const MSG: i32 = 14;
const TAG: i32 = 15;
const ID: i32 = 16;
const LR: i32 = 17;
const TB: i32 = 18;
const BT: i32 = 19;
const COLON: i32 = 20;
const ACC_DESCR: i32 = 21;
const ACC_TITLE: i32 = 22;
const TITLE: i32 = 23;
const INT: i32 = 24;
const STRING: i32 = 25;
const NEWLINE: i32 = 26;
const REFERENCE: i32 = 31;

static TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        word("cherry-pick"),
        TokenType::literal("gitGraph:", "gitGraph:"),
        word("HIGHLIGHT"),
        keyword("gitGraph").longer(NAME),
        word("checkout"),
        word("REVERSE"),
        TokenType::literal("parent:", "parent:"),
        word("commit"),
        word("NORMAL"),
        word("branch"),
        TokenType::literal("order:", "order:"),
        word("switch"),
        TokenType::literal("type:", "type:"),
        word("merge"),
        TokenType::literal("msg:", "msg:"),
        TokenType::literal("tag:", "tag:"),
        TokenType::literal("id:", "id:"),
        word("LR"),
        word("TB"),
        word("BT"),
        TokenType::literal(":", ":"),
        // Without a closing brace the block form scans to the end of the
        // text, and a brace is not a token on its own, so the first failure
        // is final.
        acc_descr().opener(r"[\t ]*accDescr\s*\{"),
        acc_title(),
        title(),
        TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", None),
        string(),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
        TokenType::regex("REFERENCE", r"\w([-\./\w]*[-\w])?", None),
    ])
});

const NAME: &[&str] = &["REFERENCE"];

/// A keyword that is the start of a longer name is part of that name:
/// `commits` is a branch.
fn word(name: &'static str) -> TokenType {
    TokenType::literal(name, name).longer(NAME)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitTypeName {
    Normal,
    Reverse,
    Highlight,
}

pub enum StatementAst {
    Commit {
        id: Option<String>,
        message: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitTypeName>,
    },
    Branch {
        name: String,
        order: Option<f64>,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitTypeName>,
    },
    Checkout {
        branch: String,
    },
    CherryPicking {
        id: Option<String>,
        tags: Vec<String>,
        parent: Option<String>,
    },
}

#[derive(Default)]
pub struct GitGraphAst {
    pub dir: Option<&'static str>,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub statements: Vec<StatementAst>,
}

fn end_of_line(r: &mut Reader) -> Result<(), Error> {
    if r.kind() == -1 {
        return Ok(());
    }
    if r.kind() != NEWLINE {
        return Err(r.fail("a line break or the end of input"));
    }
    while r.kind() == NEWLINE {
        r.i += 1;
    }
    Ok(())
}

fn name(r: &mut Reader) -> Result<String, Error> {
    match r.kind() {
        REFERENCE => Ok(r.take().to_string()),
        STRING => Ok(string_value(r.take())),
        _ => Err(r.fail("a branch name")),
    }
}

fn quoted(r: &mut Reader) -> Result<String, Error> {
    Ok(string_value(r.expect(STRING)?))
}

fn commit_type(r: &mut Reader) -> Result<CommitTypeName, Error> {
    let kind = match r.kind() {
        NORMAL => CommitTypeName::Normal,
        REVERSE => CommitTypeName::Reverse,
        HIGHLIGHT => CommitTypeName::Highlight,
        _ => return Err(r.fail("NORMAL, REVERSE, or HIGHLIGHT")),
    };
    r.i += 1;
    Ok(kind)
}

pub fn parse_git(src: &str) -> Result<GitGraphAst, Error> {
    let tokens = tokenize(src, &TOKENS, Kind::GitGraph)?;
    let mut r = Reader::new(&tokens, &TOKENS, Kind::GitGraph);
    let mut ast = GitGraphAst::default();

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    if !r.accept(GIT_GRAPH_COLON) {
        r.expect(GIT_GRAPH)?;
        let next = tokens.kinds.get(r.i + 1).copied();
        if matches!(r.kind(), LR | TB | BT) && next == Some(COLON) {
            ast.dir = Some(match r.kind() {
                LR => "LR",
                TB => "TB",
                _ => "BT",
            });
            r.i += 2;
        } else {
            r.accept(COLON);
        }
    }

    while r.kind() != -1 {
        match r.kind() {
            NEWLINE => r.i += 1,
            ACC_DESCR => {
                ast.acc_descr = Some(acc_descr_value(r.take()));
                end_of_line(&mut r)?;
            }
            ACC_TITLE => {
                ast.acc_title = Some(acc_title_value(r.take()));
                end_of_line(&mut r)?;
            }
            TITLE => {
                ast.title = Some(title_value(r.take()));
                end_of_line(&mut r)?;
            }
            COMMIT => {
                r.i += 1;
                let (mut id, mut message, mut tags, mut kind) = (None, None, Vec::new(), None);
                loop {
                    if r.accept(ID) {
                        id = Some(quoted(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(quoted(&mut r)?);
                    } else if r.accept(TYPE) {
                        kind = Some(commit_type(&mut r)?);
                    } else if r.accept(MSG) || r.kind() == STRING {
                        message = Some(quoted(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements.push(StatementAst::Commit {
                    id,
                    message,
                    tags,
                    kind,
                });
            }
            BRANCH => {
                r.i += 1;
                let name = name(&mut r)?;
                let mut order = None;
                if r.accept(ORDER) {
                    order = Some(js::parse_int(r.expect(INT)?, 10));
                }
                end_of_line(&mut r)?;
                ast.statements.push(StatementAst::Branch { name, order });
            }
            MERGE => {
                r.i += 1;
                let branch = name(&mut r)?;
                let (mut id, mut tags, mut kind) = (None, Vec::new(), None);
                loop {
                    if r.accept(ID) {
                        id = Some(quoted(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(quoted(&mut r)?);
                    } else if r.accept(TYPE) {
                        kind = Some(commit_type(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements.push(StatementAst::Merge {
                    branch,
                    id,
                    tags,
                    kind,
                });
            }
            CHECKOUT | SWITCH => {
                r.i += 1;
                let branch = name(&mut r)?;
                end_of_line(&mut r)?;
                ast.statements.push(StatementAst::Checkout { branch });
            }
            CHERRY_PICK => {
                r.i += 1;
                let (mut id, mut tags, mut parent) = (None, Vec::new(), None);
                loop {
                    if r.accept(ID) {
                        id = Some(quoted(&mut r)?);
                    } else if r.accept(TAG) {
                        tags.push(quoted(&mut r)?);
                    } else if r.accept(PARENT) {
                        parent = Some(quoted(&mut r)?);
                    } else {
                        break;
                    }
                }
                end_of_line(&mut r)?;
                ast.statements
                    .push(StatementAst::CherryPicking { id, tags, parent });
            }
            _ => return Err(r.fail("a statement, a title, or a line break")),
        }
    }
    Ok(ast)
}
