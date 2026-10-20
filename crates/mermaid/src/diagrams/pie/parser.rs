//! Pele's `diagrams/pie/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, decimal, directive, keyword, newline,
        single_line_comment, string, string_value, title, token_kinds, whitespace, yaml,
    },
    error::Error,
};

token_kinds! {
    enum Token {
        ShowData = keyword("showData"),
        Pie = keyword("pie"),
        Colon = TokenType::literal(":"),
        Number = TokenType::regex(
            "NUMBER_PIE",
            r"(?:-?[0-9]+\.[0-9]+(?!\.))|(?:-?(0|[1-9][0-9]*)(?!\.))",
            Some("-0123456789"),
        )
        .scanner(|| decimal(true, false)),
        AccDescr = acc_descr(),
        AccTitle = acc_title(),
        Title = title(),
        String = string(),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
    }
}

pub struct PieSection {
    pub label: String,
    pub value: f64,
}

#[derive(Default)]
pub struct PieAst {
    pub show_data: bool,
    pub header: Header,
    pub sections: Vec<PieSection>,
}

pub fn parse_pie(src: &str) -> Result<PieAst, Error> {
    let mut r = Reader::<Token>::new(src, Kind::Pie)?;
    let mut ast = PieAst::default();

    r.skip(Token::Newline);
    r.expect(Token::Pie)?;
    ast.show_data = r.accept(Token::ShowData);
    while let Some(kind) = r.kind() {
        if r.heading(&mut ast.header) {
            r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => r.bump(),
            Token::String => {
                let label = string_value(r.take());
                r.expect(Token::Colon)?;
                let value = r.number(Token::Number)?;
                ast.sections.push(PieSection { label, value });
                r.end_of_line(Token::Newline)?;
            }
            _ => return Err(r.fail("a section, a title, or a line break")),
        }
    }
    Ok(ast)
}
