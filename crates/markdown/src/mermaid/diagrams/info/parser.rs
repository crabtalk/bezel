//! Pele's `diagrams/info/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Table, acc_descr, acc_descr_value, acc_title, acc_title_value, directive, keyword,
        newline, single_line_comment, title, title_value, tokenize, whitespace, yaml,
    },
    error::Error,
};

const SHOW_INFO: i32 = 0;
const INFO: i32 = 1;
const ACC_DESCR: i32 = 2;
const ACC_TITLE: i32 = 3;
const TITLE: i32 = 4;
const NEWLINE: i32 = 5;

static TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        keyword("showInfo"),
        keyword("info"),
        acc_descr(),
        acc_title(),
        title(),
        newline(),
        whitespace(),
        yaml(),
        directive(),
        single_line_comment(),
    ])
});

#[derive(Default)]
pub struct InfoAst {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

pub fn parse_info(src: &str) -> Result<InfoAst, Error> {
    let tokens = tokenize(src, &TOKENS, Kind::Info)?;
    let mut r = Reader::new(&tokens, &TOKENS, Kind::Info);
    let mut ast = InfoAst::default();

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    r.expect(INFO)?;
    while r.kind() == NEWLINE {
        r.i += 1;
    }
    if r.accept(SHOW_INFO) {
        while r.kind() == NEWLINE {
            r.i += 1;
        }
    }
    while r.kind() != -1 {
        match r.kind() {
            ACC_DESCR => ast.acc_descr = Some(acc_descr_value(r.take())),
            ACC_TITLE => ast.acc_title = Some(acc_title_value(r.take())),
            TITLE => ast.title = Some(title_value(r.take())),
            _ => return Err(r.fail("a title or the end of input")),
        }
        if r.kind() == -1 {
            break;
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
