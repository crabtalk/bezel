//! Pele's `diagrams/cynefin/parser.ts`.

use crate::{
    detect::Kind,
    diagrams::common::tokens::{
        Header, Reader, TokenType, acc_descr, acc_title, directive, keyword, newline,
        single_line_comment, string, string_value, title, token_kinds, whitespace, yaml,
    },
    error::Error,
};

use super::model::Domain;

token_kinds! {
    enum Token {
        KeywordColon = TokenType::literal("cynefin-beta:"),
        Keyword = keyword("cynefin-beta"),
        Arrow = TokenType::literal("-->"),
        Colon = TokenType::literal(":"),
        Domain = TokenType::regex(
            "DOMAIN_NAME",
            r"complex|complicated|clear|chaotic|confusion",
            None,
        ),
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

pub struct DomainBlockAst {
    pub domain: Domain,
    pub items: Vec<String>,
}

pub struct TransitionAst {
    pub from: Domain,
    pub to: Domain,
    pub label: Option<String>,
}

#[derive(Default)]
pub struct CynefinAst {
    pub header: Header,
    pub domains: Vec<DomainBlockAst>,
    pub transitions: Vec<TransitionAst>,
}

pub fn parse_cynefin(src: &str) -> Result<CynefinAst, Error> {
    let mut r = Reader::<Token>::new(src, Kind::Cynefin)?;
    let mut ast = CynefinAst::default();

    r.skip(Token::Newline);
    if !r.accept(Token::Keyword) && !r.accept(Token::KeywordColon) {
        return Err(r.fail("'cynefin-beta'"));
    }
    while let Some(kind) = r.kind() {
        if r.heading(&mut ast.header) {
            r.end_of_line(Token::Newline)?;
            continue;
        }
        match kind {
            Token::Newline => r.bump(),
            Token::Domain => {
                let name = domain(r.take());
                if r.accept(Token::Arrow) {
                    let to = domain(r.expect(Token::Domain)?);
                    let mut transition = TransitionAst {
                        from: name,
                        to,
                        label: None,
                    };
                    if r.accept(Token::Colon) {
                        transition.label = Some(string_value(r.expect(Token::String)?));
                    }
                    r.end_of_line(Token::Newline)?;
                    ast.transitions.push(transition);
                    continue;
                }
                let mut block = DomainBlockAst {
                    domain: name,
                    items: Vec::new(),
                };
                r.skip(Token::Newline);
                while r.at(Token::String) {
                    block.items.push(string_value(r.take()));
                    r.skip(Token::Newline);
                }
                ast.domains.push(block);
            }
            _ => return Err(r.fail("a domain, a transition, a title, or a line break")),
        }
    }
    Ok(ast)
}

fn domain(text: &str) -> Domain {
    Domain::from_name(text).expect("a DOMAIN_NAME token is a domain's name")
}
