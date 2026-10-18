//! Pele's `diagrams/packet/parser.ts`.

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

const PACKET_BETA: i32 = 0;
const PACKET: i32 = 1;
const DASH: i32 = 2;
const PLUS: i32 = 3;
const COLON: i32 = 4;
const ACC_DESCR: i32 = 5;
const ACC_TITLE: i32 = 6;
const TITLE: i32 = 7;
const INT: i32 = 8;
const STRING: i32 = 9;
const NEWLINE: i32 = 10;

static TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        TokenType::literal("packet-beta", "packet-beta"),
        keyword("packet"),
        // A front matter block also starts with dashes, so Langium lets it
        // win over a dash.
        TokenType::literal("-", "-").longer(&["YAML"]),
        TokenType::literal("+", "+"),
        TokenType::literal(":", ":"),
        acc_descr(),
        acc_title(),
        title(),
        TokenType::regex("INT", r"0|[1-9][0-9]*(?!\.)", None),
        string(),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
    ])
});

pub struct PacketBlockAst {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub bits: Option<f64>,
    pub label: String,
}

#[derive(Default)]
pub struct PacketAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub blocks: Vec<PacketBlockAst>,
}

pub fn parse_packet(src: &str) -> Result<PacketAst, Error> {
    let tokens = tokenize(src, &TOKENS, Kind::Packet)?;
    let mut r = Reader::new(&tokens, &TOKENS, Kind::Packet);
    let mut ast = PacketAst::default();

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

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    if !r.accept(PACKET_BETA) {
        r.expect(PACKET)?;
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
            INT | PLUS => {
                let mut block = PacketBlockAst {
                    start: None,
                    end: None,
                    bits: None,
                    label: String::new(),
                };
                if r.accept(PLUS) {
                    block.bits = Some(js::number(r.expect(INT)?));
                } else {
                    block.start = Some(js::number(r.take()));
                    if r.accept(DASH) {
                        block.end = Some(js::number(r.expect(INT)?));
                    }
                }
                r.expect(COLON)?;
                block.label = string_value(r.expect(STRING)?);
                ast.blocks.push(block);
                end_of_line(&mut r)?;
            }
            _ => return Err(r.fail("a field, a title, or a line break")),
        }
    }
    Ok(ast)
}
