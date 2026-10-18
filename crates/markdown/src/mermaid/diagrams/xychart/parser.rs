//! Pele's `diagrams/xychart/parser.ts`.

use super::{
    db::{DataPoint, XyChartModel, XyText},
    lexer::tokenize,
    tokens::*,
};
use crate::mermaid::{detect::Kind, error::Error, js};

fn is_word_token(t: u8) -> bool {
    matches!(
        t,
        AMP | NUM | ALPHA | PLUS | EQUALS | MULT | DOT | BRKT | MINUS | UNDERSCORE
    )
}

fn is_eol(t: u8) -> bool {
    matches!(t, NEWLINE | SEMI | EOF)
}

fn is_text(t: u8) -> bool {
    t == STR || is_word_token(t)
}

pub fn parse_xy_chart(src: &str, db: &mut XyChartModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let (types, texts, starts) = (&tokens.types, &tokens.texts, &tokens.starts);
    let mut i = 0;

    let fail = |i: usize, expected: &str| -> Error {
        let t = types[i];
        if t == ERROR {
            return Error::at(Kind::XyChart, src, starts[i], "", true);
        }
        Error::at(
            Kind::XyChart,
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

    let text = |i: &mut usize| -> Result<XyText, Error> {
        if !is_text(types[*i]) {
            return Err(fail(*i, "'STR', 'ALPHA', 'NUM'"));
        }
        let mut s = texts[*i].to_string();
        *i += 1;
        if types[*i - 1] != STR {
            while is_word_token(types[*i]) {
                s.push_str(texts[*i]);
                *i += 1;
            }
        }
        Ok(XyText { text: s })
    };

    let range = |i: &mut usize| -> Result<(f64, f64), Error> {
        let min = js::number(expect(i, NUMBER_WITH_DECIMAL)?);
        expect(i, ARROW_DELIMITER)?;
        Ok((min, js::number(expect(i, NUMBER_WITH_DECIMAL)?)))
    };

    let points = |i: &mut usize| -> Result<Vec<DataPoint>, Error> {
        let mut out = Vec::new();
        expect(i, SQUARE_BRACES_START)?;
        loop {
            let value = js::number(expect(i, NUMBER_WITH_DECIMAL)?);
            let label = if types[*i] == STR {
                *i += 1;
                texts[*i - 1].to_string()
            } else {
                String::new()
            };
            out.push(DataPoint { value, label });
            if types[*i] != COMMA {
                break;
            }
            *i += 1;
        }
        expect(i, SQUARE_BRACES_END)?;
        Ok(out)
    };

    loop {
        if is_eol(types[i]) {
            i += 1;
        } else if types[i] != XYCHART {
            break;
        } else {
            i += 1;
            if types[i] == CHART_ORIENTATION {
                db.set_orientation(texts[i]);
                i += 1;
            }
        }
    }

    while types[i] != END {
        let t = types[i];
        i += 1;
        match t {
            TITLE => {
                let title = text(&mut i)?;
                db.set_diagram_title(js::trim(&title.text));
            }
            X_AXIS | Y_AXIS => {
                let title = if is_text(types[i]) {
                    Some(text(&mut i)?)
                } else {
                    None
                };
                if t == X_AXIS && types[i] == SQUARE_BRACES_START {
                    let mut categories = Vec::new();
                    loop {
                        i += 1;
                        categories.push(text(&mut i)?);
                        if types[i] != COMMA {
                            break;
                        }
                    }
                    expect(&mut i, SQUARE_BRACES_END)?;
                    db.set_x_axis_band(categories);
                } else if types[i] == NUMBER_WITH_DECIMAL || title.is_none() {
                    let (min, max) = range(&mut i)?;
                    if t == X_AXIS {
                        db.set_x_axis_range_data(min, max);
                    } else {
                        db.set_y_axis_range_data(min, max);
                    }
                }
                let title = title.unwrap_or(XyText {
                    text: String::new(),
                });
                if t == X_AXIS {
                    db.set_x_axis_title(title);
                } else {
                    db.set_y_axis_title(title);
                }
            }
            LINE | BAR => {
                let title = if types[i] == SQUARE_BRACES_START {
                    XyText {
                        text: String::new(),
                    }
                } else {
                    text(&mut i)?
                };
                let data = points(&mut i)?;
                if t == LINE {
                    db.set_line_data(title, data);
                } else {
                    db.set_bar_data(title, data);
                }
            }
            ACC_TITLE => db.set_acc_title(js::trim(expect(&mut i, ACC_TITLE_VALUE)?)),
            ACC_DESCR => db.set_acc_description(js::trim(expect(&mut i, ACC_DESCR_VALUE)?)),
            ACC_DESCR_MULTILINE_VALUE => db.set_acc_description(js::trim(texts[i - 1])),
            _ => {
                return Err(fail(
                    i - 1,
                    "'title', 'X_AXIS', 'Y_AXIS', 'LINE', 'BAR', 'acc_title', 'acc_descr', 'acc_descr_multiline_value'",
                ));
            }
        }
        while is_eol(types[i]) {
            i += 1;
        }
    }
    Ok(())
}
