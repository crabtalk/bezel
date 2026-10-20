//! Follows the parser Jison builds from Mermaid's grammar: blank lines and
//! comments may come first, then the keyword and at most one line break, then
//! statements. A statement is a line of text, with or without indentation, or
//! a blank line; each is ended by a blank line, a line break or the end of the
//! text, and any number of line breaks after that. Pele's
//! `diagrams/ishikawa/parser.ts`.

use super::{
    db::IshikawaDb,
    lexer::{T, tokenize},
};
use crate::{detect::Kind, error::Error, js, token::Stream};

const STOP: &str = "'SPACELINE', 'NL', 'EOF'";
const STOPS: [T; 3] = [T::Spaceline, T::Nl, T::Eof];

pub fn parse_ishikawa(src: &str, db: &mut IshikawaDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::Ishikawa, src, &tokens);

    if s.eat(T::Spaceline) {
        while s.is_any(&[T::Spaceline, T::Nl]) {
            s.bump();
        }
        if !s.is(T::Ishikawa) {
            return Err(s.fail("'SPACELINE', 'NL', 'ISHIKAWA'"));
        }
    } else if !s.is(T::Ishikawa) {
        return Err(s.fail("'SPACELINE', 'ISHIKAWA'"));
    }
    s.bump();
    s.eat(T::Nl);

    loop {
        let kind = s.kind();
        if kind == T::Text || (kind == T::Spacelist && s.peek(1) == T::Text) {
            let level = if kind == T::Text {
                0
            } else {
                s.take().chars().count() as i64
            };
            let text = s.take();
            if !s.is_any(&STOPS) {
                return Err(s.fail(STOP));
            }
            db.add_node(level, js::trim(text).to_string());
        } else if kind == T::Spaceline || kind == T::Spacelist {
            s.bump();
            if !s.is_any(&STOPS) {
                return Err(s.fail(&if kind == T::Spacelist {
                    format!("{STOP}, 'TEXT'")
                } else {
                    STOP.to_string()
                }));
            }
        } else {
            return Err(s.fail("'SPACELINE', 'SPACELIST', 'TEXT'"));
        }
        s.bump();
        while s.is_any(&[T::Nl, T::Eof]) {
            s.bump();
        }
        if s.is(T::End) {
            return Ok(());
        }
    }
}
