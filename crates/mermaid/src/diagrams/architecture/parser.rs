//! Pele's `diagrams/architecture/parser.ts`.

use std::sync::LazyLock;

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Table, TokenType, acc_descr, acc_descr_value, acc_title, acc_title_value,
        directive, newline, single_line_comment, string, string_value, title, title_value,
        tokenize, whitespace, yaml,
    },
    error::Error,
    js,
};

const HEADER: i32 = 0;
const JUNCTION: i32 = 1;
const SERVICE: i32 = 2;
const COLUMN: i32 = 3;
const GROUP: i32 = 4;
const ALIGN: i32 = 5;
const ROW: i32 = 6;
const DASHES: i32 = 7;
const IN: i32 = 8;
const COLON: i32 = 9;
const DASH: i32 = 10;
const DIRECTION: i32 = 11;
const ARROW_GROUP: i32 = 12;
const INTO: i32 = 13;
const ACC_DESCR: i32 = 14;
const ACC_TITLE: i32 = 15;
const TITLE: i32 = 16;
const STRING: i32 = 17;
const ID: i32 = 18;
const NEWLINE: i32 = 19;
const ICON: i32 = 24;
const LABEL: i32 = 25;

// The direction letters come before identifiers and have no longer
// alternative, so Mermaid reads an id that starts with L, R, T or B as a
// direction followed by the rest.
pub static ARCHITECTURE_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    let word = |text: &'static str| TokenType::literal(text, text).longer(&["ID"]);
    Table::new(vec![
        word("architecture-beta"),
        word("junction"),
        word("service"),
        word("column"),
        word("group"),
        word("align"),
        word("row"),
        TokenType::literal("--", "--").longer(&["YAML"]),
        word("in"),
        TokenType::literal(":", ":"),
        TokenType::literal("-", "-").longer(&["YAML"]),
        TokenType::regex("ARROW_DIRECTION", r"L|R|T|B", Some("LRTB")),
        TokenType::regex("ARROW_GROUP", r"\{group\}", Some("{")),
        TokenType::regex("ARROW_INTO", r"<|>", Some("<>")),
        acc_descr(),
        acc_title(),
        title(),
        string(),
        TokenType::regex("ID", r"[\w]([-\w]*\w)?", None),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
        TokenType::regex("ARCH_ICON", r"\([\w-:]+\)", Some("(")),
        TokenType::regex(
            "ARCH_TITLE",
            r#"\[(?:"([^"\\]|\\.)*"|'([^'\\]|\\.)*'|[^\[\]\r\n]+)\]"#,
            Some("["),
        ),
    ])
});

/// One of the four sides of a node: `L`, `R`, `T` or `B`.
pub type Side = String;

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
    pub direction: String,
    pub members: Vec<String>,
}

#[derive(Default)]
pub struct ArchitectureAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub groups: Vec<GroupAst>,
    pub services: Vec<ServiceAst>,
    pub junctions: Vec<JunctionAst>,
    pub edges: Vec<EdgeAst>,
    pub alignments: Vec<AlignmentAst>,
}

/// Mermaid's ArchitectureValueConverter for ARCH_TITLE.
fn label_value(text: &str) -> String {
    let mut result = js::trim(&text[1..text.len() - 1]).to_string();
    if (result.starts_with('"') && result.ends_with('"') && result.len() >= 2)
        || (result.starts_with('\'') && result.ends_with('\'') && result.len() >= 2)
    {
        result = result[1..result.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\'", "'");
    } else if result == "\"" || result == "'" {
        result = String::new();
    }
    js::trim(&result).to_string()
}

pub fn parse_architecture(src: &str) -> Result<ArchitectureAst, Error> {
    let table = &*ARCHITECTURE_TOKENS;
    let tokens = tokenize(src, table, Kind::Architecture)?;
    let mut r = Reader::new(&tokens, table, Kind::Architecture);
    let mut ast = ArchitectureAst::default();

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    r.expect(HEADER)?;
    while r.kind() != -1 {
        match r.kind() {
            NEWLINE => {
                r.i += 1;
                continue;
            }
            ACC_DESCR => ast.acc_descr = Some(acc_descr_value(r.take())),
            ACC_TITLE => ast.acc_title = Some(acc_title_value(r.take())),
            TITLE => ast.title = Some(title_value(r.take())),
            GROUP => {
                r.i += 1;
                let mut group = GroupAst {
                    id: r.expect(ID)?.to_string(),
                    icon: None,
                    title: None,
                    r#in: None,
                };
                if r.kind() == ICON {
                    let text = r.take();
                    group.icon = Some(text[1..text.len() - 1].to_string());
                }
                if r.kind() == LABEL {
                    group.title = Some(label_value(r.take()));
                }
                if r.accept(IN) {
                    group.r#in = Some(r.expect(ID)?.to_string());
                }
                ast.groups.push(group);
            }
            SERVICE => {
                r.i += 1;
                let mut service = ServiceAst {
                    id: r.expect(ID)?.to_string(),
                    icon: None,
                    icon_text: None,
                    title: None,
                    r#in: None,
                };
                if r.kind() == STRING {
                    service.icon_text = Some(string_value(r.take()));
                } else if r.kind() == ICON {
                    let text = r.take();
                    service.icon = Some(text[1..text.len() - 1].to_string());
                }
                if r.kind() == LABEL {
                    service.title = Some(label_value(r.take()));
                }
                if r.accept(IN) {
                    service.r#in = Some(r.expect(ID)?.to_string());
                }
                ast.services.push(service);
            }
            JUNCTION => {
                r.i += 1;
                let mut junction = JunctionAst {
                    id: r.expect(ID)?.to_string(),
                    r#in: None,
                };
                if r.accept(IN) {
                    junction.r#in = Some(r.expect(ID)?.to_string());
                }
                ast.junctions.push(junction);
            }
            ID => {
                let lhs_id = r.take().to_string();
                let lhs_group = r.accept(ARROW_GROUP);
                r.expect(COLON)?;
                let lhs_dir = r.expect(DIRECTION)?.to_string();
                let lhs_into = r.accept(INTO);
                let mut title = None;
                if !r.accept(DASHES) {
                    if r.kind() != DASH {
                        return Err(r.fail("'--' or '-'"));
                    }
                    r.i += 1;
                    title = Some(label_value(r.expect(LABEL)?));
                    r.expect(DASH)?;
                }
                let rhs_into = r.accept(INTO);
                let rhs_dir = r.expect(DIRECTION)?.to_string();
                r.expect(COLON)?;
                let rhs_id = r.expect(ID)?.to_string();
                let rhs_group = r.accept(ARROW_GROUP);
                ast.edges.push(EdgeAst {
                    lhs_id,
                    lhs_group,
                    lhs_dir,
                    lhs_into,
                    title,
                    rhs_into,
                    rhs_dir,
                    rhs_id,
                    rhs_group,
                });
            }
            ALIGN => {
                r.i += 1;
                if r.kind() != ROW && r.kind() != COLUMN {
                    return Err(r.fail("'row' or 'column'"));
                }
                let direction = r.take().to_string();
                let mut members = vec![r.expect(ID)?.to_string(), r.expect(ID)?.to_string()];
                while r.kind() == ID {
                    members.push(r.take().to_string());
                }
                ast.alignments.push(AlignmentAst { direction, members });
            }
            _ => {
                return Err(r.fail(
                    "a group, a service, a junction, an edge, an alignment, a title, or a line break",
                ));
            }
        }
        // End of a line.
        if r.kind() == -1 {
            continue;
        }
        if r.kind() != NEWLINE {
            return Err(r.fail("a line break or the end of input"));
        }
        while r.kind() == NEWLINE {
            r.i += 1;
        }
    }
    Ok(ast)
}
