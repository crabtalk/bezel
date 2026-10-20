//! Pele's `diagrams/packet/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, directive, keyword, newline,
        single_line_comment, string, string_value, title, token_kinds, whitespace, yaml,
    },
    error::Error,
};

token_kinds! {
    enum Token {
        PacketBeta = TokenType::literal("packet-beta"),
        Packet = keyword("packet"),
        // A front matter block also starts with dashes, so Langium lets it
        // win over a dash.
        Dash = TokenType::literal("-").longer(&[Token::Yaml]),
        Plus = TokenType::literal("+"),
        Colon = TokenType::literal(":"),
        AccDescr = acc_descr(),
        AccTitle = acc_title(),
        Title = title(),
        Int = TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", None),
        String = string(),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
    }
}

pub struct PacketBlockAst {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub bits: Option<f64>,
    pub label: String,
}

#[derive(Default)]
pub struct PacketAst {
    pub header: Header,
    pub blocks: Vec<PacketBlockAst>,
}

pub fn parse_packet(src: &str) -> Result<PacketAst, Error> {
    let mut r = Reader::<Token>::new(src, Kind::Packet)?;
    let mut ast = PacketAst::default();

    r.skip(Token::Newline);
    if !r.accept(Token::PacketBeta) {
        r.expect(Token::Packet)?;
    }
    while let Some(kind) = r.kind() {
        if r.heading(&mut ast.header) {
            r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => r.bump(),
            Token::Int | Token::Plus => {
                let mut block = PacketBlockAst {
                    start: None,
                    end: None,
                    bits: None,
                    label: String::new(),
                };
                if r.accept(Token::Plus) {
                    block.bits = Some(r.number(Token::Int)?);
                } else {
                    block.start = Some(r.number(Token::Int)?);
                    if r.accept(Token::Dash) {
                        block.end = Some(r.number(Token::Int)?);
                    }
                }
                r.expect(Token::Colon)?;
                block.label = string_value(r.expect(Token::String)?);
                ast.blocks.push(block);
                r.end_of_line(Token::Newline)?;
            }
            _ => return Err(r.fail("a field, a title, or a line break")),
        }
    }
    Ok(ast)
}
