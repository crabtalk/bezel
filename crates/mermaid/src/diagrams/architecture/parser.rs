//! Pele's `diagrams/architecture/parser.ts`.

use crate::util::chars::inner;
use serde::Serialize;

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, directive, newline, single_line_comment,
        string, string_value, title, token_kinds, whitespace, yaml,
    },
    error::Error,
    geom::Side,
    js,
};

// The direction letters come before identifiers and have no longer
// alternative, so Mermaid reads an id that starts with L, R, T or B as a
// direction followed by the rest.
token_kinds! {
    enum Token {
        Header = word("architecture-beta"),
        Junction = word("junction"),
        Service = word("service"),
        Column = word("column"),
        Group = word("group"),
        Align = word("align"),
        Row = word("row"),
        Dashes = TokenType::literal("--").longer(&[Token::Yaml]),
        In = word("in"),
        Colon = TokenType::literal(":"),
        Dash = TokenType::literal("-").longer(&[Token::Yaml]),
        Direction = TokenType::regex("ARROW_DIRECTION", r"L|R|T|B", Some("LRTB")),
        ArrowGroup = TokenType::regex("ARROW_GROUP", r"\{group\}", Some("{")),
        Into = TokenType::regex("ARROW_INTO", r"<|>", Some("<>")),
        AccDescr = acc_descr(),
        AccTitle = acc_title(),
        Title = title(),
        String = string(),
        Id = TokenType::regex("ID", r"[\w]([-\w]*\w)?", None),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
        Icon = TokenType::regex("ARCH_ICON", r"\([\w-:]+\)", Some("(")),
        Label = TokenType::regex(
            "ARCH_TITLE",
            r#"\[(?:"([^"\\]|\\.)*"|'([^'\\]|\\.)*'|[^\[\]\r\n]+)\]"#,
            Some("["),
        ),
    }
}

fn word(text: &'static str) -> TokenType<Token> {
    TokenType::literal(text).longer(&[Token::Id])
}

/// The side a direction letter names: `L`, `R`, `T` or `B`.
fn side(letter: &str) -> Side {
    match letter {
        "L" => Side::Left,
        "R" => Side::Right,
        "T" => Side::Top,
        _ => Side::Bottom,
    }
}

/// A side as the source names it.
pub(super) fn side_letter<S: serde::Serializer>(side: &Side, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(match side {
        Side::Left => "L",
        Side::Right => "R",
        Side::Top => "T",
        Side::Bottom => "B",
    })
}

/// Which way an `align` directive lines its members up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Alignment {
    Row,
    Column,
}

impl Alignment {
    pub fn name(self) -> &'static str {
        match self {
            Alignment::Row => "row",
            Alignment::Column => "column",
        }
    }
}

pub struct GroupAst {
    pub id: String,
    pub icon: Option<String>,
    pub title: Option<String>,
    pub r#in: Option<String>,
}

pub struct ServiceAst {
    pub id: String,
    pub icon: Option<String>,
    pub icon_text: Option<String>,
    pub title: Option<String>,
    pub r#in: Option<String>,
}

pub struct JunctionAst {
    pub id: String,
    pub r#in: Option<String>,
}

pub struct EdgeAst {
    pub lhs_id: String,
    pub lhs_group: bool,
    pub lhs_dir: Side,
    pub lhs_into: bool,
    pub title: Option<String>,
    pub rhs_into: bool,
    pub rhs_dir: Side,
    pub rhs_id: String,
    pub rhs_group: bool,
}

pub struct AlignmentAst {
    pub direction: Alignment,
    pub members: Vec<String>,
}

#[derive(Default)]
pub struct ArchitectureAst {
    pub header: Header,
    pub groups: Vec<GroupAst>,
    pub services: Vec<ServiceAst>,
    pub junctions: Vec<JunctionAst>,
    pub edges: Vec<EdgeAst>,
    pub alignments: Vec<AlignmentAst>,
}

/// Mermaid's ArchitectureValueConverter for ARCH_TITLE.
fn label_value(text: &str) -> String {
    let mut result = js::trim(inner(text)).to_string();
    if (result.starts_with('"') && result.ends_with('"') && result.len() >= 2)
        || (result.starts_with('\'') && result.ends_with('\'') && result.len() >= 2)
    {
        result = inner(&result).replace("\\\"", "\"").replace("\\'", "'");
    } else if result == "\"" || result == "'" {
        result = String::new();
    }
    js::trim(&result).to_string()
}

struct Parser<'s> {
    r: Reader<'s, Token>,
}

impl Parser<'_> {
    /// The `in` clause after a node or a group.
    fn parent(&mut self) -> Result<Option<String>, Error> {
        if !self.r.accept(Token::In) {
            return Ok(None);
        }
        Ok(Some(self.r.expect(Token::Id)?.to_string()))
    }

    fn icon(&mut self) -> Option<String> {
        self.r
            .at(Token::Icon)
            .then(|| inner(self.r.take()).to_string())
    }

    fn label(&mut self) -> Option<String> {
        self.r.at(Token::Label).then(|| label_value(self.r.take()))
    }

    fn group(&mut self) -> Result<GroupAst, Error> {
        self.r.bump();
        let id = self.r.expect(Token::Id)?.to_string();
        Ok(GroupAst {
            id,
            icon: self.icon(),
            title: self.label(),
            r#in: self.parent()?,
        })
    }

    fn service(&mut self) -> Result<ServiceAst, Error> {
        self.r.bump();
        let id = self.r.expect(Token::Id)?.to_string();
        let (icon, icon_text) = if self.r.at(Token::String) {
            (None, Some(string_value(self.r.take())))
        } else {
            (self.icon(), None)
        };
        Ok(ServiceAst {
            id,
            icon,
            icon_text,
            title: self.label(),
            r#in: self.parent()?,
        })
    }

    fn junction(&mut self) -> Result<JunctionAst, Error> {
        self.r.bump();
        let id = self.r.expect(Token::Id)?.to_string();
        Ok(JunctionAst {
            id,
            r#in: self.parent()?,
        })
    }

    fn edge(&mut self) -> Result<EdgeAst, Error> {
        let lhs_id = self.r.take().to_string();
        let lhs_group = self.r.accept(Token::ArrowGroup);
        self.r.expect(Token::Colon)?;
        let lhs_dir = side(self.r.expect(Token::Direction)?);
        let lhs_into = self.r.accept(Token::Into);
        let mut title = None;
        if !self.r.accept(Token::Dashes) {
            if !self.r.accept(Token::Dash) {
                return Err(self.r.fail("'--' or '-'"));
            }
            title = Some(label_value(self.r.expect(Token::Label)?));
            self.r.expect(Token::Dash)?;
        }
        let rhs_into = self.r.accept(Token::Into);
        let rhs_dir = side(self.r.expect(Token::Direction)?);
        self.r.expect(Token::Colon)?;
        let rhs_id = self.r.expect(Token::Id)?.to_string();
        let rhs_group = self.r.accept(Token::ArrowGroup);
        Ok(EdgeAst {
            lhs_id,
            lhs_group,
            lhs_dir,
            lhs_into,
            title,
            rhs_into,
            rhs_dir,
            rhs_id,
            rhs_group,
        })
    }

    fn alignment(&mut self) -> Result<AlignmentAst, Error> {
        self.r.bump();
        let direction = if self.r.accept(Token::Row) {
            Alignment::Row
        } else if self.r.accept(Token::Column) {
            Alignment::Column
        } else {
            return Err(self.r.fail("'row' or 'column'"));
        };
        let mut members = vec![
            self.r.expect(Token::Id)?.to_string(),
            self.r.expect(Token::Id)?.to_string(),
        ];
        while self.r.at(Token::Id) {
            members.push(self.r.take().to_string());
        }
        Ok(AlignmentAst { direction, members })
    }
}

pub fn parse_architecture(src: &str) -> Result<ArchitectureAst, Error> {
    let mut p = Parser {
        r: Reader::new(src, Kind::Architecture)?,
    };
    let mut ast = ArchitectureAst::default();

    p.r.skip(Token::Newline);
    p.r.expect(Token::Header)?;
    while let Some(kind) = p.r.kind() {
        if kind == Token::Newline {
            p.r.bump();
            continue;
        }
        if !p.r.heading(&mut ast.header) {
            match kind {
                Token::Group => ast.groups.push(p.group()?),
                Token::Service => ast.services.push(p.service()?),
                Token::Junction => ast.junctions.push(p.junction()?),
                Token::Id => ast.edges.push(p.edge()?),
                Token::Align => ast.alignments.push(p.alignment()?),
                _ => {
                    return Err(p.r.fail(
                        "a group, a service, a junction, an edge, an alignment, a title, or a line break",
                    ));
                }
            }
        }
        p.r.end_of_line(Token::Newline)?;
    }
    Ok(ast)
}
