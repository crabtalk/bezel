//! Pele's `diagrams/pie/parser.ts`.

use std::sync::LazyLock;

use crate::mermaid::{
    detect::Kind,
    diagrams::common::tokens::{
        Reader, Scanner, Table, TokenType, acc_descr, acc_descr_value, acc_title, acc_title_value,
        directive, keyword, newline, single_line_comment, string, string_value, title, title_value,
        tokenize, whitespace, yaml,
    },
    error::Error,
    js,
    util::chars::code_at,
};

const SHOW_DATA: i32 = 0;
const PIE: i32 = 1;
const COLON: i32 = 2;
const NUMBER: i32 = 3;
const ACC_DESCR: i32 = 4;
const ACC_TITLE: i32 = 5;
const TITLE: i32 = 6;
const STRING: i32 = 7;
const NEWLINE: i32 = 8;

pub static PIE_TOKENS: LazyLock<Table> = LazyLock::new(|| {
    Table::new(vec![
        keyword("showData"),
        keyword("pie"),
        TokenType::literal(":", ":"),
        TokenType::regex(
            "NUMBER_PIE",
            r"(?:-?[0-9]+\.[0-9]+(?!\.))|(?:-?(0|[1-9][0-9]*)(?!\.))",
            Some("-0123456789"),
        )
        .scanner(number_scanner),
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

fn digits(src: &str, from: usize) -> usize {
    let mut i = from;
    while (48..=57).contains(&code_at(src, i)) {
        i += 1;
    }
    i
}

/// Answers as NUMBER_PIE's pattern does. The pattern reads a whole run of
/// digits before it finds there is no fraction, and would do so again from
/// every digit of a long run of zeros.
fn number_scanner() -> Scanner {
    let mut from = 0;
    let mut end = 0;
    let mut fraction: Option<usize> = None;
    Box::new(move |src, p| {
        let at = if code_at(src, p) == 45 { p + 1 } else { p };
        if at < from || at >= end {
            from = at;
            end = digits(src, at);
            fraction = None;
            if end > at && code_at(src, end) == 46 {
                let stop = digits(src, end + 1);
                let length = stop - end - 1;
                if length > 0 && code_at(src, stop) != 46 {
                    fraction = Some(stop);
                } else if length > 1 {
                    fraction = Some(stop - 1);
                }
            }
        }
        if end == at {
            return None;
        }
        if let Some(fraction) = fraction {
            return Some(fraction);
        }
        if code_at(src, at) == 48 {
            return if code_at(src, at + 1) == 46 {
                None
            } else {
                Some(at + 1)
            };
        }
        if code_at(src, end) != 46 {
            return Some(end);
        }
        if end - 1 > at { Some(end - 1) } else { None }
    })
}

pub struct PieSection {
    pub label: String,
    pub value: f64,
}

#[derive(Default)]
pub struct PieAst {
    pub show_data: bool,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<PieSection>,
}

pub fn parse_pie(src: &str) -> Result<PieAst, Error> {
    let tokens = tokenize(src, &PIE_TOKENS, Kind::Pie)?;
    let mut r = Reader::new(&tokens, &PIE_TOKENS, Kind::Pie);
    let mut ast = PieAst::default();

    let end_of_line = |r: &mut Reader| -> Result<(), Error> {
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
    };

    while r.kind() == NEWLINE {
        r.i += 1;
    }
    r.expect(PIE)?;
    ast.show_data = r.accept(SHOW_DATA);
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
            STRING => {
                let label = string_value(r.take());
                r.expect(COLON)?;
                let value = js::number(r.expect(NUMBER)?);
                ast.sections.push(PieSection { label, value });
                end_of_line(&mut r)?;
            }
            _ => return Err(r.fail("a section, a title, or a line break")),
        }
    }
    Ok(ast)
}
