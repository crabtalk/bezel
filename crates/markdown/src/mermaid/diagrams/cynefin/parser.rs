//! Pele's `diagrams/cynefin/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Table, TokenType, acc_descr, acc_descr_value, acc_title, acc_title_value,
        directive, keyword, newline, single_line_comment, string, string_value, title, title_value,
        tokenize, whitespace, yaml,
    },
    error::Error,
};

const KEYWORD_COLON: i32 = 0;
const KEYWORD: i32 = 1;
const ARROW: i32 = 2;
const COLON: i32 = 3;
const DOMAIN: i32 = 4;
const ACC_DESCR: i32 = 5;
const ACC_TITLE: i32 = 6;
const TITLE: i32 = 7;
const STRING: i32 = 8;
const NEWLINE: i32 = 9;

pub static CYNEFIN_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        TokenType::literal("cynefin-beta:", "cynefin-beta:"),
        keyword("cynefin-beta"),
        TokenType::literal("-->", "-->"),
        TokenType::literal(":", ":"),
        TokenType::regex(
            "DOMAIN_NAME",
            r"complex|complicated|clear|chaotic|confusion",
            None,
        ),
        acc_descr(),
        acc_title(),
        title(),
        string(),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
    ])
});

pub struct DomainBlockAst {
    pub domain: String,
    pub items: Vec<String>,
}

pub struct TransitionAst {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

#[derive(Default)]
pub struct CynefinAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub domains: Vec<DomainBlockAst>,
    pub transitions: Vec<TransitionAst>,
}

fn skip_lines(r: &mut Reader) {
    while r.kind() == NEWLINE {
        r.i += 1;
    }
}

fn end_of_line(r: &mut Reader) -> Result<(), Error> {
    if r.kind() == -1 {
        return Ok(());
    }
    if r.kind() != NEWLINE {
        return Err(r.fail("a line break or the end of input"));
    }
    skip_lines(r);
    Ok(())
}

pub fn parse_cynefin(src: &str) -> Result<CynefinAst, Error> {
    let tokens = tokenize(src, &CYNEFIN_TOKENS, Kind::Cynefin)?;
    let mut r = Reader::new(&tokens, &CYNEFIN_TOKENS, Kind::Cynefin);
    let mut ast = CynefinAst::default();

    skip_lines(&mut r);
    if r.kind() != KEYWORD && r.kind() != KEYWORD_COLON {
        return Err(r.fail("'cynefin-beta'"));
    }
    r.i += 1;
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
            DOMAIN => {
                let name = r.take().to_string();
                if r.accept(ARROW) {
                    let to = r.expect(DOMAIN)?.to_string();
                    let mut transition = TransitionAst {
                        from: name,
                        to,
                        label: None,
                    };
                    if r.accept(COLON) {
                        transition.label = Some(string_value(r.expect(STRING)?));
                    }
                    end_of_line(&mut r)?;
                    ast.transitions.push(transition);
                    continue;
                }
                let mut block = DomainBlockAst {
                    domain: name,
                    items: Vec::new(),
                };
                skip_lines(&mut r);
                while r.kind() == STRING {
                    block.items.push(string_value(r.take()));
                    skip_lines(&mut r);
                }
                ast.domains.push(block);
            }
            _ => return Err(r.fail("a domain, a transition, a title, or a line break")),
        }
    }
    Ok(ast)
}
