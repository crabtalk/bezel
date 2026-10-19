//! Pele's `diagrams/wardley/parser.ts`.

use std::sync::LazyLock;

use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Reader, Scanner, Table, TokenType, acc_descr_value, acc_title, acc_title_value,
            directive, newline, single_line_comment, string, string_value, title, title_value,
            tokenize, whitespace, yaml,
        },
    },
    error::Error,
    js,
};

const LBRACKET: i32 = 0;
const COMMA: i32 = 1;
const RBRACKET: i32 = 2;
const AT: i32 = 3;
const SLASH: i32 = 4;
const LPAREN: i32 = 5;
const RPAREN: i32 = 6;
const MINUS: i32 = 7;
const LBRACE: i32 = 8;
const RBRACE: i32 = 9;
const NUMBER: i32 = 10;
const ARROW: i32 = 11;
const LINK_PORT: i32 = 12;
const LINK_ARROW: i32 = 13;
const LINK_LABEL: i32 = 14;
const STRATEGY: i32 = 15;
const WARDLEY: i32 = 16;
const SIZE: i32 = 17;
const EVOLUTION: i32 = 18;
const ANCHOR: i32 = 19;
const COMPONENT: i32 = 20;
const LABEL: i32 = 21;
const INERTIA: i32 = 22;
const EVOLVE: i32 = 23;
const PIPELINE: i32 = 24;
const NOTE: i32 = 25;
const ANNOTATIONS: i32 = 26;
const ANNOTATION: i32 = 27;
const ACCELERATOR: i32 = 28;
const DEACCELERATOR: i32 = 29;
const NAME: i32 = 30;
const ACC_DESCR: i32 = 32;
const ACC_TITLE: i32 = 33;
const TITLE: i32 = 34;
const INT: i32 = 35;
const STRING: i32 = 36;
const ID: i32 = 37;
const NEWLINE: i32 = 38;

pub static WARDLEY_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        TokenType::literal("[", "["),
        TokenType::literal(",", ","),
        TokenType::literal("]", "]"),
        TokenType::literal("@", "@"),
        TokenType::literal("/", "/"),
        TokenType::literal("(", "("),
        TokenType::literal(")", ")"),
        TokenType::literal("-", "-").longer(&["ARROW", "LINK_ARROW", "YAML"]),
        TokenType::literal("{", "{"),
        TokenType::literal("}", "}"),
        TokenType::regex("WARDLEY_NUMBER", r"[0-9]+\.[0-9]+", Some("0123456789"))
            .scanner(number_scanner),
        arrow(),
        TokenType::regex("LINK_PORT", r"\+<>|\+>|\+<", Some("+")),
        TokenType::regex(
            "LINK_ARROW",
            r"-->|-\.->|>|\+'[^']*'<>|\+'[^']*'<|\+'[^']*'>",
            Some("->+"),
        ),
        TokenType::regex("LINK_LABEL", r";[^\n\r]+", Some(";")),
        TokenType::regex("STRATEGY", r"build|buy|outsource|market", Some("bom")),
        word("KW_WARDLEY", "wardley-beta"),
        word("KW_SIZE", "size"),
        word("KW_EVOLUTION", "evolution"),
        word("KW_ANCHOR", "anchor"),
        word("KW_COMPONENT", "component"),
        word("KW_LABEL", "label"),
        word("KW_INERTIA", "inertia"),
        word("KW_EVOLVE", "evolve"),
        word("KW_PIPELINE", "pipeline"),
        word("KW_NOTE", "note"),
        word("KW_ANNOTATIONS", "annotations"),
        word("KW_ANNOTATION", "annotation"),
        word("KW_ACCELERATOR", "accelerator"),
        word("KW_DEACCELERATOR", "deaccelerator"),
        TokenType::regex(
            "NAME_WITH_SPACES",
            r"(?!title\s|accTitle|accDescr)[A-Za-z](?:[A-Za-z0-9_()&]|-(?!>))*(?:[ \t]+[A-Za-z(](?:[A-Za-z0-9_()&]|-(?!>))*)*",
            Some("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"),
        ),
        TokenType::regex("WS", r"[ \t]+", Some("\t ")).hidden(),
        acc_descr("ACC_DESCR"),
        acc_title(),
        title(),
        TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", Some("0123456789")),
        string(),
        TokenType::regex(
            "ID",
            r"[\w]([-\w]*\w)?",
            Some("_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"),
        ),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
    ])
});

fn arrow() -> TokenType {
    TokenType::regex("ARROW", r"->", Some("-"))
}

fn word(name: &'static str, text: &'static str) -> TokenType {
    TokenType::regex(name, text, Some(&text[..1]))
}

fn digits(src: &str, from: usize) -> usize {
    let bytes = src.as_bytes();
    let mut i = from;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i
}

/// Answers as WARDLEY_NUMBER's pattern does. The pattern reads a whole run
/// of digits before it finds there is no fraction, and would do so again from
/// every digit of a long run.
fn number_scanner() -> Scanner {
    let mut from = 0;
    let mut end = 0;
    Box::new(move |src, p| {
        if p < from || p >= end {
            from = p;
            end = digits(src, p);
        }
        if end == p || src.as_bytes().get(end) != Some(&b'.') {
            return None;
        }
        let stop = digits(src, end + 1);
        (stop > end + 1).then_some(stop)
    })
}

pub struct LabelAst {
    pub neg_x: bool,
    pub offset_x: f64,
    pub neg_y: bool,
    pub offset_y: f64,
}

pub struct StageAst {
    pub name: String,
    pub boundary: Option<f64>,
    pub second_name: Option<String>,
}

pub struct ComponentAst {
    pub name: String,
    pub visibility: f64,
    pub evolution: f64,
    pub label: Option<LabelAst>,
    pub strategy: Option<String>,
    pub inertia: bool,
}

pub struct PipelineComponentAst {
    pub name: String,
    pub evolution: f64,
    pub label: Option<LabelAst>,
}

pub struct LinkAst {
    pub from: String,
    pub from_port: Option<String>,
    pub arrow: Option<String>,
    pub to: String,
    pub to_port: Option<String>,
    pub link_label: Option<String>,
}

pub struct AnchorAst {
    pub name: String,
    pub visibility: f64,
    pub evolution: f64,
}

pub struct EvolveAst {
    pub component: String,
    pub target: f64,
}

pub struct PipelineAst {
    pub parent: String,
    pub components: Vec<PipelineComponentAst>,
}

pub struct NoteAst {
    pub text: String,
    pub visibility: f64,
    pub evolution: f64,
}

pub struct AnnotationsAst {
    pub x: f64,
    pub y: f64,
}

pub struct AnnotationAst {
    pub number: f64,
    pub x: f64,
    pub y: f64,
    pub text: String,
}

pub struct PlacedAst {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Default)]
pub struct WardleyAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub size: Option<(f64, f64)>,
    pub evolution: Option<Vec<StageAst>>,
    pub anchors: Vec<AnchorAst>,
    pub components: Vec<ComponentAst>,
    pub links: Vec<LinkAst>,
    pub evolves: Vec<EvolveAst>,
    pub pipelines: Vec<PipelineAst>,
    pub notes: Vec<NoteAst>,
    pub annotations: Vec<AnnotationsAst>,
    pub annotation: Vec<AnnotationAst>,
    pub accelerators: Vec<PlacedAst>,
    pub deaccelerators: Vec<PlacedAst>,
}

struct Parser<'t, 's> {
    r: Reader<'t, 's>,
}

impl Parser<'_, '_> {
    fn at(&self, kind: i32) -> bool {
        self.r.kind() == kind
    }

    fn is_name(&self) -> bool {
        self.at(STRING) || self.at(ID) || self.at(NAME)
    }

    fn name(&mut self) -> Result<String, Error> {
        if self.at(STRING) {
            return Ok(string_value(self.r.take()));
        }
        if self.at(ID) || self.at(NAME) {
            return Ok(self.r.take().to_string());
        }
        Err(self.r.fail("a name"))
    }

    fn number(&mut self) -> Result<f64, Error> {
        Ok(js::number(self.r.expect(NUMBER)?))
    }

    fn int(&mut self) -> Result<f64, Error> {
        Ok(js::parse_int(self.r.expect(INT)?, 10))
    }

    fn coordinate(&mut self) -> Result<f64, Error> {
        if self.at(NUMBER) {
            Ok(js::number(self.r.take()))
        } else {
            self.int()
        }
    }

    fn end_of_line(&mut self) -> Result<(), Error> {
        if self.at(-1) {
            return Ok(());
        }
        if !self.at(NEWLINE) {
            return Err(self.r.fail("a line break or the end of input"));
        }
        while self.at(NEWLINE) {
            self.r.i += 1;
        }
        Ok(())
    }

    fn pair(&mut self, read: fn(&mut Self) -> Result<f64, Error>) -> Result<(f64, f64), Error> {
        self.r.expect(LBRACKET)?;
        let first = read(self)?;
        self.r.expect(COMMA)?;
        let second = read(self)?;
        self.r.expect(RBRACKET)?;
        Ok((first, second))
    }

    fn label(&mut self) -> Result<LabelAst, Error> {
        self.r.expect(LABEL)?;
        self.r.expect(LBRACKET)?;
        let neg_x = self.r.accept(MINUS);
        let offset_x = self.int()?;
        self.r.expect(COMMA)?;
        let neg_y = self.r.accept(MINUS);
        let offset_y = self.int()?;
        self.r.expect(RBRACKET)?;
        Ok(LabelAst {
            neg_x,
            offset_x,
            neg_y,
            offset_y,
        })
    }

    fn stage(&mut self) -> Result<StageAst, Error> {
        let mut out = StageAst {
            name: self.name()?,
            boundary: None,
            second_name: None,
        };
        if self.r.accept(AT) {
            out.boundary = Some(self.number()?);
        }
        if self.r.accept(SLASH) {
            out.second_name = Some(self.name()?);
        }
        Ok(out)
    }
}

pub fn parse_wardley(src: &str) -> Result<WardleyAst, Error> {
    let tokens = tokenize(src, &WARDLEY_TOKENS, Kind::Wardley)?;
    let mut p = Parser {
        r: Reader::new(&tokens, &WARDLEY_TOKENS, Kind::Wardley),
    };
    let mut ast = WardleyAst::default();

    while p.at(NEWLINE) {
        p.r.i += 1;
    }
    p.r.expect(WARDLEY)?;
    while !p.at(-1) {
        match p.r.kind() {
            NEWLINE => p.r.i += 1,
            ACC_DESCR => {
                ast.acc_descr = Some(acc_descr_value(p.r.take()));
                p.end_of_line()?;
            }
            ACC_TITLE => {
                ast.acc_title = Some(acc_title_value(p.r.take()));
                p.end_of_line()?;
            }
            TITLE => {
                ast.title = Some(title_value(p.r.take()));
                p.end_of_line()?;
            }
            SIZE => {
                p.r.i += 1;
                let (width, height) = p.pair(Parser::int)?;
                p.end_of_line()?;
                ast.size = Some((width, height));
            }
            EVOLUTION => {
                p.r.i += 1;
                let mut stages = vec![p.stage()?];
                p.r.expect(ARROW)?;
                stages.push(p.stage()?);
                while p.r.accept(ARROW) {
                    stages.push(p.stage()?);
                }
                p.end_of_line()?;
                ast.evolution = Some(stages);
            }
            ANCHOR => {
                p.r.i += 1;
                let name = p.name()?;
                let (visibility, evolution) = p.pair(Parser::number)?;
                p.end_of_line()?;
                ast.anchors.push(AnchorAst {
                    name,
                    visibility,
                    evolution,
                });
            }
            COMPONENT => {
                p.r.i += 1;
                let name = p.name()?;
                let (visibility, evolution) = p.pair(Parser::number)?;
                let mut component = ComponentAst {
                    name,
                    visibility,
                    evolution,
                    label: None,
                    strategy: None,
                    inertia: false,
                };
                if p.at(LABEL) {
                    component.label = Some(p.label()?);
                }
                if p.at(LPAREN) && p.r.tokens.kinds[p.r.i + 1] == STRATEGY {
                    p.r.i += 1;
                    component.strategy = Some(p.r.take().to_string());
                    p.r.expect(RPAREN)?;
                }
                if p.r.accept(INERTIA) {
                    component.inertia = true;
                } else if p.r.accept(LPAREN) {
                    p.r.expect(INERTIA)?;
                    p.r.expect(RPAREN)?;
                    component.inertia = true;
                }
                p.end_of_line()?;
                ast.components.push(component);
            }
            EVOLVE => {
                p.r.i += 1;
                let component = p.name()?;
                let target = p.number()?;
                p.end_of_line()?;
                ast.evolves.push(EvolveAst { component, target });
            }
            PIPELINE => {
                p.r.i += 1;
                let parent = p.name()?;
                p.r.expect(LBRACE)?;
                p.r.expect(NEWLINE)?;
                while p.at(NEWLINE) {
                    p.r.i += 1;
                }
                let mut components = Vec::new();
                loop {
                    p.r.expect(COMPONENT)?;
                    let name = p.name()?;
                    p.r.expect(LBRACKET)?;
                    let evolution = p.number()?;
                    p.r.expect(RBRACKET)?;
                    let mut component = PipelineComponentAst {
                        name,
                        evolution,
                        label: None,
                    };
                    if p.at(LABEL) {
                        component.label = Some(p.label()?);
                    }
                    p.end_of_line()?;
                    components.push(component);
                    if !p.at(COMPONENT) {
                        break;
                    }
                }
                p.r.expect(RBRACE)?;
                p.end_of_line()?;
                ast.pipelines.push(PipelineAst { parent, components });
            }
            NOTE => {
                p.r.i += 1;
                let text = string_value(p.r.expect(STRING)?);
                let (visibility, evolution) = p.pair(Parser::number)?;
                p.end_of_line()?;
                ast.notes.push(NoteAst {
                    text,
                    visibility,
                    evolution,
                });
            }
            ANNOTATIONS => {
                p.r.i += 1;
                let (x, y) = p.pair(Parser::coordinate)?;
                p.end_of_line()?;
                ast.annotations.push(AnnotationsAst { x, y });
            }
            ANNOTATION => {
                p.r.i += 1;
                let number = p.int()?;
                p.r.expect(COMMA)?;
                let (x, y) = p.pair(Parser::coordinate)?;
                let text = string_value(p.r.expect(STRING)?);
                p.end_of_line()?;
                ast.annotation.push(AnnotationAst { number, x, y, text });
            }
            kind @ (ACCELERATOR | DEACCELERATOR) => {
                let slow = kind == DEACCELERATOR;
                p.r.i += 1;
                let name = p.name()?;
                let (x, y) = p.pair(Parser::number)?;
                p.end_of_line()?;
                let force = PlacedAst { name, x, y };
                if slow {
                    ast.deaccelerators.push(force);
                } else {
                    ast.accelerators.push(force);
                }
            }
            _ => {
                if !p.is_name() {
                    return Err(p.r.fail("a statement"));
                }
                let from = p.name()?;
                let mut link = LinkAst {
                    from,
                    from_port: None,
                    arrow: None,
                    to: String::new(),
                    to_port: None,
                    link_label: None,
                };
                if p.at(LINK_PORT) {
                    link.from_port = Some(p.r.take().to_string());
                }
                if p.at(LINK_ARROW) || p.at(ARROW) {
                    link.arrow = Some(p.r.take().to_string());
                }
                link.to = p.name()?;
                if p.at(LINK_PORT) {
                    link.to_port = Some(p.r.take().to_string());
                }
                if p.at(LINK_LABEL) {
                    link.link_label = Some(js::trim(&p.r.take()[1..]).to_string());
                }
                p.end_of_line()?;
                ast.links.push(link);
            }
        }
    }
    Ok(ast)
}
