//! Pele's `diagrams/git/parser.ts`.

use super::db::CommitType;
use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, directive, keyword, newline,
        single_line_comment, string, string_value, title, token_kinds, whitespace, yaml,
    },
    error::Error,
    geom::Direction,
    js,
};

token_kinds! {
    enum Token {
        CherryPick = word("cherry-pick"),
        GitGraphColon = TokenType::literal("gitGraph:"),
        Highlight = word("HIGHLIGHT"),
        GitGraph = keyword("gitGraph").longer(&[Token::Reference]),
        Checkout = word("checkout"),
        Reverse = word("REVERSE"),
        Parent = TokenType::literal("parent:"),
        Commit = word("commit"),
        Normal = word("NORMAL"),
        Branch = word("branch"),
        Order = TokenType::literal("order:"),
        Switch = word("switch"),
        Type = TokenType::literal("type:"),
        Merge = word("merge"),
        Msg = TokenType::literal("msg:"),
        Tag = TokenType::literal("tag:"),
        Id = TokenType::literal("id:"),
        Lr = word("LR"),
        Tb = word("TB"),
        Bt = word("BT"),
        Colon = TokenType::literal(":"),
        // Without a closing brace the block form scans to the end of the
        // text, and a brace is not a token on its own, so the first failure
        // is final.
        AccDescr = acc_descr().opener(r"[\t ]*accDescr\s*\{"),
        AccTitle = acc_title(),
        Title = title(),
        Int = TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", None),
        String = string(),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        Reference = TokenType::regex("REFERENCE", r"\w([-\./\w]*[-\w])?", None),
    }
}

/// A keyword that is the start of a longer name is part of that name:
/// `commits` is a branch.
fn word(name: &'static str) -> TokenType<Token> {
    TokenType::literal(name).longer(&[Token::Reference])
}

impl Token {
    fn direction(self) -> Option<Direction> {
        match self {
            Self::Lr => Some(Direction::LeftToRight),
            Self::Tb => Some(Direction::TopToBottom),
            Self::Bt => Some(Direction::BottomToTop),
            _ => None,
        }
    }
}

pub enum StatementAst {
    Commit {
        id: Option<String>,
        message: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitType>,
    },
    Branch {
        name: String,
        order: Option<f64>,
    },
    Merge {
        branch: String,
        id: Option<String>,
        tags: Vec<String>,
        kind: Option<CommitType>,
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
    pub dir: Option<Direction>,
    pub header: Header,
    pub statements: Vec<StatementAst>,
}

struct Parser<'s> {
    r: Reader<'s, Token>,
}

impl Parser<'_> {
    fn name(&mut self) -> Result<String, Error> {
        match self.r.kind() {
            Some(Token::Reference) => Ok(self.r.take().to_string()),
            Some(Token::String) => Ok(string_value(self.r.take())),
            _ => Err(self.r.fail("a branch name")),
        }
    }

    fn quoted(&mut self) -> Result<String, Error> {
        Ok(string_value(self.r.expect(Token::String)?))
    }

    fn commit_type(&mut self) -> Result<CommitType, Error> {
        let kind = match self.r.kind() {
            Some(Token::Normal) => CommitType::Normal,
            Some(Token::Reverse) => CommitType::Reverse,
            Some(Token::Highlight) => CommitType::Highlight,
            _ => return Err(self.r.fail("NORMAL, REVERSE, or HIGHLIGHT")),
        };
        self.r.bump();
        Ok(kind)
    }

    fn statement(&mut self, kind: Token) -> Result<StatementAst, Error> {
        self.r.bump();
        let statement = match kind {
            Token::Commit => {
                let (mut id, mut message, mut tags, mut kind) = (None, None, Vec::new(), None);
                loop {
                    if self.r.accept(Token::Id) {
                        id = Some(self.quoted()?);
                    } else if self.r.accept(Token::Tag) {
                        tags.push(self.quoted()?);
                    } else if self.r.accept(Token::Type) {
                        kind = Some(self.commit_type()?);
                    } else if self.r.accept(Token::Msg) || self.r.at(Token::String) {
                        message = Some(self.quoted()?);
                    } else {
                        break;
                    }
                }
                StatementAst::Commit {
                    id,
                    message,
                    tags,
                    kind,
                }
            }
            Token::Branch => {
                let name = self.name()?;
                let mut order = None;
                if self.r.accept(Token::Order) {
                    order = Some(js::parse_int(self.r.expect(Token::Int)?, 10));
                }
                StatementAst::Branch { name, order }
            }
            Token::Merge => {
                let branch = self.name()?;
                let (mut id, mut tags, mut kind) = (None, Vec::new(), None);
                loop {
                    if self.r.accept(Token::Id) {
                        id = Some(self.quoted()?);
                    } else if self.r.accept(Token::Tag) {
                        tags.push(self.quoted()?);
                    } else if self.r.accept(Token::Type) {
                        kind = Some(self.commit_type()?);
                    } else {
                        break;
                    }
                }
                StatementAst::Merge {
                    branch,
                    id,
                    tags,
                    kind,
                }
            }
            Token::Checkout | Token::Switch => StatementAst::Checkout {
                branch: self.name()?,
            },
            _ => {
                let (mut id, mut tags, mut parent) = (None, Vec::new(), None);
                loop {
                    if self.r.accept(Token::Id) {
                        id = Some(self.quoted()?);
                    } else if self.r.accept(Token::Tag) {
                        tags.push(self.quoted()?);
                    } else if self.r.accept(Token::Parent) {
                        parent = Some(self.quoted()?);
                    } else {
                        break;
                    }
                }
                StatementAst::CherryPicking { id, tags, parent }
            }
        };
        self.r.end_of_line(Token::Newline)?;
        Ok(statement)
    }
}

pub fn parse_git(src: &str) -> Result<GitGraphAst, Error> {
    let mut p = Parser {
        r: Reader::new(src, Kind::GitGraph)?,
    };
    let mut ast = GitGraphAst::default();

    p.r.skip(Token::Newline);
    if !p.r.accept(Token::GitGraphColon) {
        p.r.expect(Token::GitGraph)?;
        match p.r.kind().and_then(Token::direction) {
            Some(dir) if p.r.peek(1) == Some(Token::Colon) => {
                ast.dir = Some(dir);
                p.r.bump();
                p.r.bump();
            }
            _ => {
                p.r.accept(Token::Colon);
            }
        }
    }

    while let Some(kind) = p.r.kind() {
        if p.r.heading(&mut ast.header) {
            p.r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => p.r.bump(),
            Token::Commit
            | Token::Branch
            | Token::Merge
            | Token::Checkout
            | Token::Switch
            | Token::CherryPick => ast.statements.push(p.statement(kind)?),
            _ => return Err(p.r.fail("a statement, a title, or a line break")),
        }
    }
    Ok(ast)
}
