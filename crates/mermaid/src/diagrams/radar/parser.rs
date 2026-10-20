//! Pele's `diagrams/radar/parser.ts`.

use super::model::Graticule;
use crate::{
    detect::Kind,
    diagrams::common::{
        acc_descr::acc_descr,
        tokens::{
            Header, Reader, TokenType, acc_title, decimal, directive, keyword, newline,
            single_line_comment, string, string_value, title, token_kinds, whitespace, yaml,
        },
    },
    error::Error,
};

token_kinds! {
    enum Token {
        RadarColon = TokenType::literal("radar-beta:"),
        Radar = keyword("radar-beta").longer(&[Token::Id]),
        ShowLegend = TokenType::literal("showLegend").longer(&[Token::Id]),
        Graticule = TokenType::literal("graticule").longer(&[Token::Id]),
        Curve = TokenType::literal("curve").longer(&[Token::Id]),
        Ticks = TokenType::literal("ticks").longer(&[Token::Id]),
        Axis = TokenType::literal("axis").longer(&[Token::Id]),
        Max = TokenType::literal("max").longer(&[Token::Id]),
        Min = TokenType::literal("min").longer(&[Token::Id]),
        Colon = TokenType::literal(":"),
        Comma = TokenType::literal(","),
        Open = TokenType::literal("["),
        Close = TokenType::literal("]"),
        OpenBrace = TokenType::literal("{"),
        CloseBrace = TokenType::literal("}"),
        GraticuleValue = TokenType::regex("GRATICULE", r"circle|polygon", None),
        Boolean = TokenType::regex("BOOLEAN", r"true|false", None),
        AccDescr = acc_descr("ACC_DESCR"),
        AccTitle = acc_title(),
        Title = title(),
        Number = TokenType::regex(
            "NUMBER",
            r"(?:[0-9]+\.[0-9]+(?!\.))|(?:0|[1-9][0-9]*(?!\.))",
            None,
        )
        .scanner(|| decimal(false, true)),
        String = string(),
        Id = TokenType::regex("ID", r"[\w]([-\w]*\w)?", None),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
    }
}

impl Token {
    fn is_option(self) -> bool {
        matches!(
            self,
            Self::ShowLegend | Self::Ticks | Self::Max | Self::Min | Self::Graticule
        )
    }
}

pub struct AxisAst {
    pub name: String,
    pub label: Option<String>,
}

pub struct EntryAst {
    /// Langium leaves the reference unresolved; Mermaid reads only its text.
    pub axis: Option<String>,
    pub value: f64,
}

pub struct CurveAst {
    pub name: String,
    pub label: Option<String>,
    pub entries: Vec<EntryAst>,
}

pub enum OptionAst {
    ShowLegend(bool),
    Graticule(Graticule),
    Ticks(f64),
    Max(f64),
    Min(f64),
}

#[derive(Default)]
pub struct RadarAst {
    pub header: Header,
    pub axes: Vec<AxisAst>,
    pub curves: Vec<CurveAst>,
    pub options: Vec<OptionAst>,
}

struct Parser<'s> {
    r: Reader<'s, Token>,
}

impl Parser<'_> {
    fn label(&mut self) -> Result<Option<String>, Error> {
        if !self.r.accept(Token::Open) {
            return Ok(None);
        }
        let text = string_value(self.r.expect(Token::String)?);
        self.r.expect(Token::Close)?;
        Ok(Some(text))
    }

    fn entry(&mut self, detailed: bool) -> Result<EntryAst, Error> {
        let axis = if detailed {
            let axis = self.r.expect(Token::Id)?.to_string();
            self.r.accept(Token::Colon);
            Some(axis)
        } else {
            None
        };
        Ok(EntryAst {
            axis,
            value: self.r.number(Token::Number)?,
        })
    }

    fn curve(&mut self) -> Result<CurveAst, Error> {
        let name = self.r.expect(Token::Id)?.to_string();
        let label = self.label()?;
        let mut node = CurveAst {
            name,
            label,
            entries: Vec::new(),
        };
        self.r.expect(Token::OpenBrace)?;
        self.r.skip(Token::Newline);
        let detailed = match self.r.kind() {
            Some(Token::Id) => true,
            Some(Token::Number) => false,
            _ => return Err(self.r.fail("a number or an axis name")),
        };
        node.entries.push(self.entry(detailed)?);
        while self.r.accept(Token::Comma) {
            self.r.skip(Token::Newline);
            node.entries.push(self.entry(detailed)?);
        }
        self.r.skip(Token::Newline);
        self.r.expect(Token::CloseBrace)?;
        Ok(node)
    }

    fn axis(&mut self) -> Result<AxisAst, Error> {
        let name = self.r.expect(Token::Id)?.to_string();
        let label = self.label()?;
        Ok(AxisAst { name, label })
    }

    /// An option, whose name is the token here.
    fn option(&mut self, kind: Token) -> Result<OptionAst, Error> {
        self.r.bump();
        Ok(match kind {
            Token::ShowLegend => OptionAst::ShowLegend(self.r.expect(Token::Boolean)? == "true"),
            Token::Graticule => {
                OptionAst::Graticule(match self.r.expect(Token::GraticuleValue)? {
                    "polygon" => Graticule::Polygon,
                    _ => Graticule::Circle,
                })
            }
            Token::Ticks => OptionAst::Ticks(self.r.number(Token::Number)?),
            Token::Max => OptionAst::Max(self.r.number(Token::Number)?),
            _ => OptionAst::Min(self.r.number(Token::Number)?),
        })
    }
}

pub fn parse_radar(src: &str) -> Result<RadarAst, Error> {
    let mut p = Parser {
        r: Reader::new(src, Kind::Radar)?,
    };
    let mut ast = RadarAst::default();

    p.r.skip(Token::Newline);
    if !p.r.accept(Token::RadarColon) {
        p.r.expect(Token::Radar)?;
        p.r.accept(Token::Colon);
    }
    p.r.skip(Token::Newline);
    while let Some(kind) = p.r.kind() {
        if p.r.heading(&mut ast.header) {
            p.r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => p.r.bump(),
            Token::Axis => {
                p.r.bump();
                loop {
                    ast.axes.push(p.axis()?);
                    if !p.r.accept(Token::Comma) {
                        break;
                    }
                }
            }
            Token::Curve => {
                p.r.bump();
                loop {
                    ast.curves.push(p.curve()?);
                    if !p.r.accept(Token::Comma) {
                        break;
                    }
                }
            }
            kind if kind.is_option() => {
                ast.options.push(p.option(kind)?);
                while p.r.accept(Token::Comma) {
                    match p.r.kind() {
                        Some(kind) if kind.is_option() => ast.options.push(p.option(kind)?),
                        _ => return Err(p.r.fail("an option")),
                    }
                }
            }
            _ => {
                return Err(p
                    .r
                    .fail("an axis, a curve, an option, a title, or a line break"));
            }
        }
    }
    Ok(ast)
}
