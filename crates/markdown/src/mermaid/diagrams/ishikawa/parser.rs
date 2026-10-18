//! Follows the parser Jison builds from Mermaid's grammar: blank lines and
//! comments may come first, then the keyword and at most one line break, then
//! statements. A statement is a line of text, with or without indentation, or
//! a blank line; each is ended by a blank line, a line break or the end of the
//! text, and any number of line breaks after that. Pele's
//! `diagrams/ishikawa/parser.ts`.

use super::{
    db::IshikawaDb,
    lexer::{END, EOF, ISHIKAWA, NL, SPACELINE, SPACELIST, TEXT, TOKEN_NAMES, tokenize},
};
use crate::mermaid::{detect::Kind, error::Error, js};

const STOP: &str = "'SPACELINE', 'NL', 'EOF'";

pub fn parse_ishikawa(src: &str, db: &mut IshikawaDb) -> Result<(), Error> {
    let tokens = tokenize(src);
    let types = &tokens.types;
    let starts = &tokens.starts;
    let ends = &tokens.ends;
    let mut i = 0;

    let fail = |i: usize, expected: &str| -> Error {
        Error::at(
            Kind::Ishikawa,
            src,
            starts[i],
            &format!(
                "Expecting {expected}, got '{}'",
                TOKEN_NAMES[types[i] as usize]
            ),
            false,
        )
    };

    if types[i] == SPACELINE {
        i += 1;
        while types[i] == SPACELINE || types[i] == NL {
            i += 1;
        }
        if types[i] != ISHIKAWA {
            return Err(fail(i, "'SPACELINE', 'NL', 'ISHIKAWA'"));
        }
    } else if types[i] != ISHIKAWA {
        return Err(fail(i, "'SPACELINE', 'ISHIKAWA'"));
    }
    i += 1;
    if types[i] == NL {
        i += 1;
    }

    loop {
        let kind = types[i];
        if kind == TEXT || (kind == SPACELIST && types[i + 1] == TEXT) {
            let text = if kind == TEXT { i } else { i + 1 };
            i = text + 1;
            if types[i] != SPACELINE && types[i] != NL && types[i] != EOF {
                return Err(fail(i, STOP));
            }
            let level = if kind == TEXT {
                0
            } else {
                js::len(&src[starts[text - 1]..ends[text - 1]]) as i64
            };
            db.add_node(level, js::trim(&src[starts[text]..ends[text]]).to_string());
        } else if kind == SPACELINE || kind == SPACELIST {
            i += 1;
            if types[i] != SPACELINE && types[i] != NL && types[i] != EOF {
                let expected = if kind == SPACELIST {
                    format!("{STOP}, 'TEXT'")
                } else {
                    STOP.to_string()
                };
                return Err(fail(i, &expected));
            }
        } else {
            return Err(fail(i, "'SPACELINE', 'SPACELIST', 'TEXT'"));
        }
        i += 1;
        while types[i] == NL || types[i] == EOF {
            i += 1;
        }
        if types[i] == END {
            return Ok(());
        }
    }
}
