//! Ports Mermaid's venn grammar. A token is read only when the grammar has to
//! look at one to decide, because the lexer asks the model whether an
//! indented `text` continues a set, and the statements are what tell the
//! model so. Pele's `diagrams/venn/parser.ts`.

use super::{
    db::VennDb,
    lexer::{
        BRACKET_LABEL, COLON, COMMA, EOF, ERROR, IDENTIFIER, INDENT_TEXT, IndentHost, NEWLINE,
        NUMERIC, RGBACOLOR, SET, STRING, STYLE, TEXT, TITLE, TOKEN_NAMES, UNION, VENN, VennLexer,
    },
};
use crate::{detect::Kind, error::Error, js, util::chars::width_at};

const STATEMENT: &str = "'EOF', 'NEWLINE', 'TITLE', 'SET', 'UNION', 'TEXT', 'INDENT_TEXT', 'STYLE'";
const VALUE: &str = "'NUMERIC', 'IDENTIFIER', 'STRING', 'HEXCOLOR', 'RGBCOLOR', 'RGBACOLOR'";

struct Parser<'s, 'd> {
    src: &'s str,
    lexer: VennLexer<'s>,
    db: &'d mut VennDb,
    kind: i32,
}

impl<'s> Parser<'s, '_> {
    fn peek(&mut self) -> i32 {
        if self.kind == -1 {
            self.kind = self.lexer.next(self.db);
        }
        self.kind
    }

    fn fail(&mut self, expected: &str) -> Error {
        if self.peek() == ERROR {
            return Error::at(Kind::Venn, self.src, self.lexer.start, "", true);
        }
        Error::at(
            Kind::Venn,
            self.src,
            self.lexer.start,
            &format!(
                "Expecting {expected}, got '{}'",
                TOKEN_NAMES[self.kind as usize]
            ),
            false,
        )
    }

    fn take(&mut self) -> &'s str {
        self.kind = -1;
        self.lexer.text
    }

    fn expect(&mut self, wanted: i32) -> Result<&'s str, Error> {
        if self.peek() != wanted {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[wanted as usize])));
        }
        Ok(self.take())
    }

    fn identifier(&mut self) -> Result<String, Error> {
        if self.peek() != IDENTIFIER && self.kind != STRING {
            return Err(self.fail("'IDENTIFIER', 'STRING'"));
        }
        Ok(self.take().to_string())
    }

    fn identifiers(&mut self) -> Result<Vec<String>, Error> {
        let mut list = vec![self.identifier()?];
        while self.peek() == COMMA {
            self.take();
            list.push(self.identifier()?);
        }
        Ok(list)
    }

    fn subset(&mut self, ids: &[String], union: bool) -> Result<(), Error> {
        let label = if self.peek() == BRACKET_LABEL {
            Some(self.take())
        } else {
            None
        };
        let mut size = None;
        if self.peek() == COLON {
            self.take();
            size = Some(js::parse_float(self.expect(NUMERIC)?));
        }
        if union {
            if ids.len() < 2 {
                return Err(Error::semantic("union requires multiple identifiers").of(Kind::Venn));
            }
            self.db.validate_union_identifiers(ids)?;
        }
        self.db.add_subset_data(ids, label, size);
        self.db.set_indent_mode(true);
        Ok(())
    }

    /// The id of a text, and the label in brackets that an id which is not a
    /// number may carry. `sets` is `None` for an indented text.
    fn text(&mut self, sets: Option<Vec<String>>) -> Result<(), Error> {
        let kind = self.peek();
        if kind != IDENTIFIER && kind != STRING && kind != NUMERIC {
            return Err(self.fail("'NUMERIC', 'IDENTIFIER', 'STRING'"));
        }
        let id = self.take();
        let label = if kind != NUMERIC && self.peek() == BRACKET_LABEL {
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
        self.db.add_text_data(&sets, id, label);
        Ok(())
    }

    fn style(&mut self) -> Result<(), Error> {
        let ids = self.identifiers()?;
        let mut styles: Vec<(String, String)> = Vec::new();
        loop {
            let key = self.expect(IDENTIFIER)?.to_string();
            self.expect(COLON)?;
            let value = if self.peek() == STRING {
                self.take().to_string()
            } else {
                let mut parts: Vec<&str> = Vec::new();
                while self.peek() == IDENTIFIER || (NUMERIC..=RGBACOLOR).contains(&self.kind) {
                    parts.push(self.take());
                }
                if parts.is_empty() {
                    return Err(self.fail(VALUE));
                }
                parts.join(" ")
            };
            styles.push((key, value));
            if self.peek() != COMMA {
                break;
            }
            self.take();
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
        kind: -1,
    };
    while p.peek() == NEWLINE {
        p.take();
    }
    p.expect(VENN)?;
    loop {
        let statement = p.peek();
        if statement == EOF {
            return Ok(());
        }
        if !(NEWLINE..=STYLE).contains(&statement) || statement == VENN {
            return Err(p.fail(STATEMENT));
        }
        p.take();
        match statement {
            TITLE => {
                let text = p.lexer.text;
                let title = &text[5 + width_at(text, 5)..];
                p.db.set_diagram_title(title.to_string());
            }
            SET => {
                let id = p.identifier()?;
                p.subset(&[id], false)?;
            }
            UNION => {
                let ids = p.identifiers()?;
                p.subset(&ids, true)?;
            }
            TEXT => {
                let ids = p.identifiers()?;
                p.text(Some(ids))?;
            }
            INDENT_TEXT => p.text(None)?,
            STYLE => p.style()?,
            _ => {}
        }
    }
}
