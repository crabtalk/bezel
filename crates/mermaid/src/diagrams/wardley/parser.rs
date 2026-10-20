//! Pele's `diagrams/wardley/parser.ts`.

use crate::util::chars::digits_end;

use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Header, Reader, Scanner, TokenType, acc_title, directive, newline, single_line_comment,
            string, string_value, title, token_kinds, whitespace, yaml,
        },
    },
    error::Error,
    js,
};

use super::builder::Strategy;
use serde::Serialize;

token_kinds! {
    enum Token {
        LBracket = TokenType::literal("["),
        Comma = TokenType::literal(","),
        RBracket = TokenType::literal("]"),
        At = TokenType::literal("@"),
        Slash = TokenType::literal("/"),
        LParen = TokenType::literal("("),
        RParen = TokenType::literal(")"),
        Minus = TokenType::literal("-").longer(&[Token::Arrow, Token::LinkArrow, Token::Yaml]),
        LBrace = TokenType::literal("{"),
        RBrace = TokenType::literal("}"),
        Number = TokenType::regex("WARDLEY_NUMBER", r"[0-9]+\.[0-9]+", Some("0123456789"))
            .scanner(number_scanner),
        Arrow = TokenType::regex("ARROW", r"->", Some("-")),
        LinkPort = TokenType::regex("LINK_PORT", r"\+<>|\+>|\+<", Some("+")),
        LinkArrow = TokenType::regex(
            "LINK_ARROW",
            r"-->|-\.->|>|\+'[^']*'<>|\+'[^']*'<|\+'[^']*'>",
            Some("->+"),
        ),
        LinkLabel = TokenType::regex("LINK_LABEL", r";[^\n\r]+", Some(";")),
        Strategy = TokenType::regex("STRATEGY", r"build|buy|outsource|market", Some("bom")),
        Wardley = word("KW_WARDLEY", "wardley-beta"),
        Size = word("KW_SIZE", "size"),
        Evolution = word("KW_EVOLUTION", "evolution"),
        Anchor = word("KW_ANCHOR", "anchor"),
        Component = word("KW_COMPONENT", "component"),
        Label = word("KW_LABEL", "label"),
        Inertia = word("KW_INERTIA", "inertia"),
        Evolve = word("KW_EVOLVE", "evolve"),
        Pipeline = word("KW_PIPELINE", "pipeline"),
        Note = word("KW_NOTE", "note"),
        Annotations = word("KW_ANNOTATIONS", "annotations"),
        Annotation = word("KW_ANNOTATION", "annotation"),
        Accelerator = word("KW_ACCELERATOR", "accelerator"),
        Deaccelerator = word("KW_DEACCELERATOR", "deaccelerator"),
        Name = TokenType::regex(
            "NAME_WITH_SPACES",
            r"(?!title\s|accTitle|accDescr)[A-Za-z](?:[A-Za-z0-9_()&]|-(?!>))*(?:[ \t]+[A-Za-z(](?:[A-Za-z0-9_()&]|-(?!>))*)*",
            Some("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"),
        ),
        Ws = TokenType::regex("WS", r"[ \t]+", Some("\t ")).hidden(),
        AccDescr = acc_descr("ACC_DESCR"),
        AccTitle = acc_title(),
        Title = title(),
        Int = TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", Some("0123456789")),
        String = string(),
        Id = TokenType::regex(
            "ID",
            r"[\w]([-\w]*\w)?",
            Some("_0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"),
        ),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
    }
}

fn word(name: &'static str, text: &'static str) -> TokenType<Token> {
    TokenType::regex(name, text, Some(&text[..1]))
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
            end = digits_end(src, p);
        }
        if end == p || src.as_bytes().get(end) != Some(&b'.') {
            return None;
        }
        let stop = digits_end(src, end + 1);
        (stop > end + 1).then_some(stop)
    })
}

pub struct LabelAst {
    pub neg_x: bool,
    pub offset_x: f64,
    pub neg_y: bool,
    pub offset_y: f64,
}

impl LabelAst {
    /// The signed offsets.
    pub fn offsets(&self) -> (f64, f64) {
        (
            if self.neg_x {
                -self.offset_x
            } else {
                self.offset_x
            },
            if self.neg_y {
                -self.offset_y
            } else {
                self.offset_y
            },
        )
    }
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
    pub strategy: Option<Strategy>,
    pub inertia: bool,
}

pub struct PipelineComponentAst {
    pub name: String,
    pub evolution: f64,
    pub label: Option<LabelAst>,
}

/// Which way a link's value flows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    Forward,
    Backward,
    Bidirectional,
}

impl Flow {
    /// Whether the value flows from the source to the target.
    pub fn forward(self) -> bool {
        matches!(self, Self::Forward | Self::Bidirectional)
    }

    /// Whether the value flows from the target to the source.
    pub fn backward(self) -> bool {
        matches!(self, Self::Backward | Self::Bidirectional)
    }

    /// A port's flow: `+>`, `+<` or `+<>`.
    fn of_port(port: &str) -> Self {
        match port {
            "+<>" => Self::Bidirectional,
            "+<" => Self::Backward,
            _ => Self::Forward,
        }
    }
}

pub struct LinkAst {
    pub from: String,
    pub from_port: Option<Flow>,
    pub arrow: Option<String>,
    pub to: String,
    pub to_port: Option<Flow>,
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
    pub header: Header,
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

struct Parser<'s> {
    r: Reader<'s, Token>,
}

impl Parser<'_> {
    fn is_name(&self) -> bool {
        matches!(self.r.kind(), Some(Token::String | Token::Id | Token::Name))
    }

    fn name(&mut self) -> Result<String, Error> {
        match self.r.kind() {
            Some(Token::String) => Ok(string_value(self.r.take())),
            Some(Token::Id | Token::Name) => Ok(self.r.take().to_string()),
            _ => Err(self.r.fail("a name")),
        }
    }

    fn number(&mut self) -> Result<f64, Error> {
        self.r.number(Token::Number)
    }

    fn int(&mut self) -> Result<f64, Error> {
        Ok(js::parse_int(self.r.expect(Token::Int)?, 10))
    }

    fn coordinate(&mut self) -> Result<f64, Error> {
        if self.r.at(Token::Number) {
            self.number()
        } else {
            self.int()
        }
    }

    fn pair(&mut self, read: fn(&mut Self) -> Result<f64, Error>) -> Result<(f64, f64), Error> {
        self.r.expect(Token::LBracket)?;
        let first = read(self)?;
        self.r.expect(Token::Comma)?;
        let second = read(self)?;
        self.r.expect(Token::RBracket)?;
        Ok((first, second))
    }

    fn label(&mut self) -> Result<Option<LabelAst>, Error> {
        if !self.r.accept(Token::Label) {
            return Ok(None);
        }
        self.r.expect(Token::LBracket)?;
        let neg_x = self.r.accept(Token::Minus);
        let offset_x = self.int()?;
        self.r.expect(Token::Comma)?;
        let neg_y = self.r.accept(Token::Minus);
        let offset_y = self.int()?;
        self.r.expect(Token::RBracket)?;
        Ok(Some(LabelAst {
            neg_x,
            offset_x,
            neg_y,
            offset_y,
        }))
    }

    fn stage(&mut self) -> Result<StageAst, Error> {
        let mut out = StageAst {
            name: self.name()?,
            boundary: None,
            second_name: None,
        };
        if self.r.accept(Token::At) {
            out.boundary = Some(self.number()?);
        }
        if self.r.accept(Token::Slash) {
            out.second_name = Some(self.name()?);
        }
        Ok(out)
    }

    fn port(&mut self) -> Option<Flow> {
        self.r
            .at(Token::LinkPort)
            .then(|| Flow::of_port(self.r.take()))
    }

    fn evolution(&mut self) -> Result<Vec<StageAst>, Error> {
        let mut stages = vec![self.stage()?];
        self.r.expect(Token::Arrow)?;
        stages.push(self.stage()?);
        while self.r.accept(Token::Arrow) {
            stages.push(self.stage()?);
        }
        Ok(stages)
    }

    fn component(&mut self) -> Result<ComponentAst, Error> {
        let name = self.name()?;
        let (visibility, evolution) = self.pair(Self::number)?;
        let mut component = ComponentAst {
            name,
            visibility,
            evolution,
            label: self.label()?,
            strategy: None,
            inertia: false,
        };
        if self.r.at(Token::LParen) && self.r.peek(1) == Some(Token::Strategy) {
            self.r.bump();
            component.strategy = Strategy::from_name(self.r.take());
            self.r.expect(Token::RParen)?;
        }
        if self.r.accept(Token::Inertia) {
            component.inertia = true;
        } else if self.r.accept(Token::LParen) {
            self.r.expect(Token::Inertia)?;
            self.r.expect(Token::RParen)?;
            component.inertia = true;
        }
        Ok(component)
    }

    fn pipeline(&mut self) -> Result<PipelineAst, Error> {
        let parent = self.name()?;
        self.r.expect(Token::LBrace)?;
        self.r.expect(Token::Newline)?;
        self.r.skip(Token::Newline);
        let mut components = Vec::new();
        loop {
            self.r.expect(Token::Component)?;
            let name = self.name()?;
            self.r.expect(Token::LBracket)?;
            let evolution = self.number()?;
            self.r.expect(Token::RBracket)?;
            components.push(PipelineComponentAst {
                name,
                evolution,
                label: self.label()?,
            });
            self.r.end_of_line(Token::Newline)?;
            if !self.r.at(Token::Component) {
                break;
            }
        }
        self.r.expect(Token::RBrace)?;
        Ok(PipelineAst { parent, components })
    }

    fn link(&mut self) -> Result<LinkAst, Error> {
        let from = self.name()?;
        let from_port = self.port();
        let arrow = matches!(self.r.kind(), Some(Token::LinkArrow | Token::Arrow))
            .then(|| self.r.take().to_string());
        let to = self.name()?;
        let to_port = self.port();
        let link_label = self
            .r
            .at(Token::LinkLabel)
            .then(|| js::trim(&self.r.take()[1..]).to_string());
        Ok(LinkAst {
            from,
            from_port,
            arrow,
            to,
            to_port,
            link_label,
        })
    }

    /// A statement after its keyword `kind`, into `ast`.
    fn statement(&mut self, kind: Token, ast: &mut WardleyAst) -> Result<(), Error> {
        match kind {
            Token::Size => ast.size = Some(self.pair(Self::int)?),
            Token::Evolution => ast.evolution = Some(self.evolution()?),
            Token::Anchor => {
                let name = self.name()?;
                let (visibility, evolution) = self.pair(Self::number)?;
                ast.anchors.push(AnchorAst {
                    name,
                    visibility,
                    evolution,
                });
            }
            Token::Component => ast.components.push(self.component()?),
            Token::Evolve => {
                let component = self.name()?;
                let target = self.number()?;
                ast.evolves.push(EvolveAst { component, target });
            }
            Token::Pipeline => ast.pipelines.push(self.pipeline()?),
            Token::Note => {
                let text = string_value(self.r.expect(Token::String)?);
                let (visibility, evolution) = self.pair(Self::number)?;
                ast.notes.push(NoteAst {
                    text,
                    visibility,
                    evolution,
                });
            }
            Token::Annotations => {
                let (x, y) = self.pair(Self::coordinate)?;
                ast.annotations.push(AnnotationsAst { x, y });
            }
            Token::Annotation => {
                let number = self.int()?;
                self.r.expect(Token::Comma)?;
                let (x, y) = self.pair(Self::coordinate)?;
                let text = string_value(self.r.expect(Token::String)?);
                ast.annotation.push(AnnotationAst { number, x, y, text });
            }
            _ => {
                let name = self.name()?;
                let (x, y) = self.pair(Self::number)?;
                let force = PlacedAst { name, x, y };
                if kind == Token::Deaccelerator {
                    ast.deaccelerators.push(force);
                } else {
                    ast.accelerators.push(force);
                }
            }
        }
        Ok(())
    }
}

pub fn parse_wardley(src: &str) -> Result<WardleyAst, Error> {
    let mut p = Parser {
        r: Reader::new(src, Kind::Wardley)?,
    };
    let mut ast = WardleyAst::default();

    p.r.skip(Token::Newline);
    p.r.expect(Token::Wardley)?;
    while let Some(kind) = p.r.kind() {
        if p.r.heading(&mut ast.header) {
            p.r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => {
                p.r.bump();
                continue;
            }
            Token::Size
            | Token::Evolution
            | Token::Anchor
            | Token::Component
            | Token::Evolve
            | Token::Pipeline
            | Token::Note
            | Token::Annotations
            | Token::Annotation
            | Token::Accelerator
            | Token::Deaccelerator => {
                p.r.bump();
                p.statement(kind, &mut ast)?;
            }
            _ if p.is_name() => ast.links.push(p.link()?),
            _ => return Err(p.r.fail("a statement")),
        }
        p.r.end_of_line(Token::Newline)?;
    }
    Ok(ast)
}
