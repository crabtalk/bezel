//! Pele's `diagrams/info/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, acc_descr, acc_title, directive, keyword, newline, single_line_comment,
        title, token_kinds, whitespace, yaml,
    },
    error::Error,
};

token_kinds! {
    enum Token {
        ShowInfo = keyword("showInfo"),
        Info = keyword("info"),
        AccDescr = acc_descr(),
        AccTitle = acc_title(),
        Title = title(),
        Newline = newline(),
        Whitespace = whitespace(),
        Yaml = yaml(),
        Directive = directive(),
        Comment = single_line_comment(),
    }
}

pub fn parse_info(src: &str) -> Result<Header, Error> {
    let mut r = Reader::<Token>::new(src, Kind::Info)?;
    let mut header = Header::default();

    r.skip(Token::Newline);
    r.expect(Token::Info)?;
    r.skip(Token::Newline);
    if r.accept(Token::ShowInfo) {
        r.skip(Token::Newline);
    }
    while !r.at_end() {
        if !r.heading(&mut header) {
            return Err(r.fail("a title or the end of input"));
        }
        r.end_of_line(Token::Newline)?;
    }
    Ok(header)
}
