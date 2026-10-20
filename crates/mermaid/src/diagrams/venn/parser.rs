//! Ports Mermaid's venn grammar. A token is read only when the grammar has to
//! look at one to decide, because the lexer asks the model whether an
//! indented `text` continues a set, and the statements are what tell the
//! model so. Pele's `diagrams/venn/parser.ts`.

use std::borrow::Cow;

use super::{
    db::VennDb,
    lexer::{IndentHost, T, VennLexer},
};
use crate::{
    detect::Kind,
    error::Error,
    js,
    token::{Grammar, Token, after_keyword},
};

const STATEMENT: &str = "'EOF', 'NEWLINE', 'TITLE', 'SET', 'UNION', 'TEXT', 'INDENT_TEXT', 'STYLE'";
const VALUE: &str = "'NUMERIC', 'IDENTIFIER', 'STRING', 'HEXCOLOR', 'RGBCOLOR', 'RGBACOLOR'";
const STATEMENTS: [T; 7] = [
    T::Newline,
    T::Title,
    T::Set,
    T::Union,
    T::Text,
    T::IndentText,
    T::Style,
];
const VALUE_PARTS: [T; 5] = [
    T::Identifier,
    T::Numeric,
    T::Hexcolor,
    T::Rgbcolor,
    T::Rgbacolor,
];

struct Parser<'s, 'd> {
    src: &'s str,
    lexer: VennLexer<'s>,
    db: &'d mut VennDb,
    /// The token read ahead and not yet taken.
    peeked: Option<Token<'s, T>>,
}

impl<'s> Parser<'s, '_> {
    fn token(&mut self) -> &Token<'s, T> {
        self.peeked
            .get_or_insert_with(|| self.lexer.next(&mut *self.db))
    }

    fn peek(&mut self) -> T {
        self.token().kind
    }

    fn fail(&mut self, expected: &str) -> Error {
        let src = self.src;
        self.token().error(Kind::Venn, src, expected, None)
    }

    fn take(&mut self) -> Cow<'s, str> {
        let token = match self.peeked.take() {
            Some(token) => token,
            None => self.lexer.next(&mut *self.db),
        };
        token.text
    }

    fn expect(&mut self, kind: T) -> Result<Cow<'s, str>, Error> {
        if self.peek() != kind {
            return Err(self.fail(&format!("'{}'", kind.name())));
        }
        Ok(self.take())
    }

    fn eat(&mut self, kind: T) -> bool {
        let found = self.peek() == kind;
        if found {
            self.peeked = None;
        }
        found
    }

    fn identifier(&mut self) -> Result<String, Error> {
        if !matches!(self.peek(), T::Identifier | T::String) {
            return Err(self.fail("'IDENTIFIER', 'STRING'"));
        }
        Ok(self.take().into_owned())
    }

    fn identifiers(&mut self) -> Result<Vec<String>, Error> {
        let mut list = vec![self.identifier()?];
        while self.eat(T::Comma) {
            list.push(self.identifier()?);
        }
        Ok(list)
    }

    fn subset(&mut self, ids: &[String], union: bool) -> Result<(), Error> {
        let label = if self.peek() == T::BracketLabel {
            Some(self.take())
        } else {
            None
        };
        let mut size = None;
        if self.eat(T::Colon) {
            size = Some(js::parse_float(&self.expect(T::Numeric)?));
        }
        if union {
            if ids.len() < 2 {
                return Err(Error::semantic("union requires multiple identifiers").of(Kind::Venn));
            }
            self.db.validate_union_identifiers(ids)?;
        }
        self.db.add_subset_data(ids, label.as_deref(), size);
        self.db.set_indent_mode(true);
        Ok(())
    }

    /// The id of a text, and the label in brackets that an id which is not a
    /// number may carry. `sets` is `None` for an indented text.
    fn text(&mut self, sets: Option<Vec<String>>) -> Result<(), Error> {
        let kind = self.peek();
        if !matches!(kind, T::Identifier | T::String | T::Numeric) {
            return Err(self.fail("'NUMERIC', 'IDENTIFIER', 'STRING'"));
        }
        let id = self.take();
        let label = if kind != T::Numeric && self.peek() == T::BracketLabel {
            Some(self.take())
        } else {
            None
        };
        let sets = match sets {
            Some(sets) => sets,
            None => match self.db.current_sets() {
                Some(current) => current.to_vec(),
                None => return Err(Error::semantic("text requires set").of(Kind::Venn)),
            },
        };
        self.db.add_text_data(&sets, &id, label.as_deref());
        Ok(())
    }

    fn style(&mut self) -> Result<(), Error> {
        let ids = self.identifiers()?;
        let mut styles: Vec<(String, String)> = Vec::new();
        loop {
            let key = self.expect(T::Identifier)?.into_owned();
            self.expect(T::Colon)?;
            let value = if self.peek() == T::String {
                self.take().into_owned()
            } else {
                let mut parts: Vec<Cow<str>> = Vec::new();
                while VALUE_PARTS.contains(&self.peek()) {
                    parts.push(self.take());
                }
                if parts.is_empty() {
                    return Err(self.fail(VALUE));
                }
                parts.join(" ")
            };
            styles.push((key, value));
            if !self.eat(T::Comma) {
                break;
            }
        }
        self.db.add_style_data(&ids, styles);
        Ok(())
    }
}

pub fn parse_venn(src: &str, db: &mut VennDb) -> Result<(), Error> {
    let mut p = Parser {
        src,
        lexer: VennLexer::new(src),
        db,
        peeked: None,
    };
    while p.eat(T::Newline) {}
    p.expect(T::Venn)?;
    loop {
        let statement = p.peek();
        if statement == T::Eof {
            return Ok(());
        }
        if !STATEMENTS.contains(&statement) {
            return Err(p.fail(STATEMENT));
        }
        let text = p.take();
        match statement {
            T::Title => {
                p.db.title = Some(after_keyword(&text, 5).to_string());
            }
            T::Set => {
                let id = p.identifier()?;
                p.subset(&[id], false)?;
            }
            T::Union => {
                let ids = p.identifiers()?;
                p.subset(&ids, true)?;
            }
            T::Text => {
                let ids = p.identifiers()?;
                p.text(Some(ids))?;
            }
            T::IndentText => p.text(None)?,
            T::Style => p.style()?,
            _ => {}
        }
    }
}
