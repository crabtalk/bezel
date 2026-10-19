//! Pele's `diagrams/quadrant/parser.ts`.

use super::{
    db::{QuadrantModel, QuadrantText},
    lexer::tokenize,
    tokens::*,
};
use crate::{detect::Kind, error::Error, js};

const TEXT_TOKENS: &str = "'STR', 'MD_STR', 'ALPHA', 'NUM', 'UNICODE_TEXT', 'PUNCTUATION'";

fn is_word_token(t: u8) -> bool {
    matches!(
        t,
        PUNCTUATION
            | AMP
            | NUM
            | ALPHA
            | COMMA
            | PLUS
            | EQUALS
            | MULT
            | DOT
            | BRKT
            | UNDERSCORE
            | UNICODE_TEXT
    )
}

fn is_id(t: u8) -> bool {
    matches!(
        t,
        ALPHA | NUM | MINUS | COMMA | COLON | AMP | BRKT | MULT | UNICODE_TEXT
    )
}

fn is_style(t: u8) -> bool {
    matches!(t, ALPHA | NUM | COLON | SPACE | BRKT | MINUS)
}

fn is_eol(t: u8) -> bool {
    matches!(t, NEWLINE | SEMI | EOF)
}

fn is_text(t: u8) -> bool {
    t == STR || t == MD_STR || is_word_token(t)
}

pub fn parse_quadrant(src: &str, db: &mut QuadrantModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let (types, texts, starts) = (&tokens.types, &tokens.texts, &tokens.starts);
    let mut i = 0;

    let fail = |i: usize, expected: &str| -> Error {
        let t = types[i];
        if t == ERROR {
            return Error::at(Kind::QuadrantChart, src, starts[i], "", true);
        }
        Error::at(
            Kind::QuadrantChart,
            src,
            starts[i],
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            false,
        )
    };

    let expect = |i: &mut usize, kind: u8| -> Result<&str, Error> {
        if types[*i] != kind {
            return Err(fail(*i, &format!("'{}'", TOKEN_NAMES[kind as usize])));
        }
        *i += 1;
        Ok(texts[*i - 1])
    };

    let text = |i: &mut usize| -> Result<QuadrantText, Error> {
        let first = types[*i];
        if !is_text(first) {
            return Err(fail(*i, TEXT_TOKENS));
        }
        let mut s = texts[*i].to_string();
        *i += 1;
        while is_word_token(types[*i]) || types[*i] == SPACE || types[*i] == MINUS {
            s.push_str(texts[*i]);
            *i += 1;
        }
        Ok(QuadrantText {
            text: s,
            kind: if first == MD_STR { "markdown" } else { "text" },
        })
    };

    let styles = |i: &mut usize| -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        loop {
            if !is_style(types[*i]) {
                return Err(fail(
                    *i,
                    "'ALPHA', 'NUM', 'COLON', 'SPACE', 'BRKT', 'MINUS'",
                ));
            }
            let mut s = texts[*i].to_string();
            *i += 1;
            while is_style(types[*i]) {
                s.push_str(texts[*i]);
                *i += 1;
            }
            out.push(js::trim(&s).to_string());
            if types[*i] != COMMA {
                return Ok(out);
            }
            *i += 1;
        }
    };

    while is_eol(types[i]) || types[i] == SPACE {
        i += 1;
    }
    expect(&mut i, QUADRANT)?;

    while types[i] != END {
        while types[i] == SPACE {
            i += 1;
        }
        let t = types[i];
        match t {
            CLASSDEF => {
                i += 1;
                expect(&mut i, SPACE)?;
                if !is_id(types[i]) {
                    return Err(fail(i, "'ALPHA', 'NUM', 'UNICODE_TEXT'"));
                }
                let mut id = texts[i].to_string();
                i += 1;
                while is_id(types[i]) {
                    id.push_str(texts[i]);
                    i += 1;
                }
                expect(&mut i, SPACE)?;
                let list = styles(&mut i)?;
                db.add_class(id, &list)?;
            }
            X_AXIS | Y_AXIS => {
                i += 1;
                let mut from = text(&mut i)?;
                let mut to = None;
                if types[i] == DELIMITER {
                    i += 1;
                    if is_text(types[i]) {
                        to = Some(text(&mut i)?);
                    } else {
                        from.text.push_str(" ⟶ ");
                    }
                }
                if t == X_AXIS {
                    db.set_x_axis_left_text(from);
                    if let Some(to) = to {
                        db.set_x_axis_right_text(to);
                    }
                } else {
                    db.set_y_axis_bottom_text(from);
                    if let Some(to) = to {
                        db.set_y_axis_top_text(to);
                    }
                }
            }
            QUADRANT_1 | QUADRANT_2 | QUADRANT_3 | QUADRANT_4 => {
                i += 1;
                let label = text(&mut i)?;
                db.set_quadrant_text((t - QUADRANT_1) as usize, label);
            }
            TITLE => {
                i += 1;
                db.set_diagram_title(js::trim(expect(&mut i, TITLE_VALUE)?));
            }
            ACC_TITLE => {
                i += 1;
                db.set_acc_title(js::trim(expect(&mut i, ACC_TITLE_VALUE)?));
            }
            ACC_DESCR => {
                i += 1;
                db.set_acc_description(js::trim(expect(&mut i, ACC_DESCR_VALUE)?));
            }
            ACC_DESCR_MULTILINE_VALUE => {
                db.set_acc_description(js::trim(texts[i]));
                i += 1;
            }
            _ => {
                if is_text(t) {
                    let label = text(&mut i)?;
                    let class_name = if types[i] == CLASS_NAME {
                        i += 1;
                        texts[i - 1].to_string()
                    } else {
                        String::new()
                    };
                    expect(&mut i, POINT_START)?;
                    let x = expect(&mut i, POINT_X)?;
                    let y = expect(&mut i, POINT_Y)?;
                    let list = if is_style(types[i]) {
                        styles(&mut i)?
                    } else {
                        Vec::new()
                    };
                    db.add_point(label, class_name, x, y, &list)?;
                }
            }
        }
        if !is_eol(types[i]) {
            return Err(fail(i, "'NEWLINE', 'SEMI', 'EOF'"));
        }
        i += 1;
    }
    Ok(())
}
