//! Pele's `diagrams/xychart/parser.ts`.

use super::{
    db::{DataPoint, PlotKind, XyChartModel},
    lexer::tokenize,
    tokens::T,
};
use crate::diagrams::common::chart::ChartKind;
use crate::{detect::Kind, error::Error, js, token::Stream};

fn text(s: &mut Stream<'_, '_, T>) -> Result<String, Error> {
    let first = s.kind();
    if !first.is_text() {
        return Err(s.fail("'STR', 'ALPHA', 'NUM'"));
    }
    let mut text = s.take().to_string();
    if first != T::Str {
        while s.kind().is_word() {
            text += s.take();
        }
    }
    Ok(text)
}

fn number(s: &mut Stream<'_, '_, T>) -> Result<f64, Error> {
    Ok(js::number(s.expect(T::NumberWithDecimal, "")?))
}

fn range(s: &mut Stream<'_, '_, T>) -> Result<(f64, f64), Error> {
    let min = number(s)?;
    s.expect(T::ArrowDelimiter, "")?;
    Ok((min, number(s)?))
}

fn points(s: &mut Stream<'_, '_, T>) -> Result<Vec<DataPoint>, Error> {
    let mut out = Vec::new();
    s.expect(T::SquareBracesStart, "")?;
    loop {
        let value = number(s)?;
        let label = if s.is(T::Str) {
            s.take().to_string()
        } else {
            String::new()
        };
        out.push(DataPoint { value, label });
        if !s.eat(T::Comma) {
            break;
        }
    }
    s.expect(T::SquareBracesEnd, "")?;
    Ok(out)
}

pub fn parse_xy_chart(src: &str, db: &mut XyChartModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::XyChart, src, &tokens);

    loop {
        if s.kind().is_eol() {
            s.bump();
        } else if s.eat(T::Xychart) {
            if s.is(T::ChartOrientation) {
                db.set_orientation(s.take());
            }
        } else {
            break;
        }
    }

    while !s.is(T::End) {
        let t = s.kind();
        match t {
            T::Title => {
                s.bump();
                let title = text(&mut s)?;
                db.titles.set_title(js::trim(&title));
            }
            T::XAxis | T::YAxis => {
                s.bump();
                let title = if s.kind().is_text() {
                    Some(text(&mut s)?)
                } else {
                    None
                };
                if t == T::XAxis && s.is(T::SquareBracesStart) {
                    let mut categories = Vec::new();
                    loop {
                        s.bump();
                        categories.push(text(&mut s)?);
                        if !s.is(T::Comma) {
                            break;
                        }
                    }
                    s.expect(T::SquareBracesEnd, "")?;
                    db.set_x_axis_band(&categories);
                } else if s.is(T::NumberWithDecimal) || title.is_none() {
                    let (min, max) = range(&mut s)?;
                    if t == T::XAxis {
                        db.set_x_axis_range_data(min, max);
                    } else {
                        db.set_y_axis_range_data(min, max);
                    }
                }
                let title = title.unwrap_or_default();
                if t == T::XAxis {
                    db.set_x_axis_title(&title);
                } else {
                    db.set_y_axis_title(&title);
                }
            }
            T::Line | T::Bar => {
                s.bump();
                let title = if s.is(T::SquareBracesStart) {
                    String::new()
                } else {
                    text(&mut s)?
                };
                let data = points(&mut s)?;
                let kind = if t == T::Line {
                    PlotKind::Line
                } else {
                    PlotKind::Bar
                };
                db.add_plot(kind, &title, &data);
            }
            T::AccTitle => {
                s.bump();
                db.titles
                    .set_acc_title(js::trim(s.expect(T::AccTitleValue, "")?));
            }
            T::AccDescr => {
                s.bump();
                db.titles
                    .set_acc_descr(js::trim(s.expect(T::AccDescrValue, "")?));
            }
            T::AccDescrMultilineValue => db.titles.set_acc_descr(js::trim(s.take())),
            _ => {
                return Err(s.fail(
                    "'title', 'X_AXIS', 'Y_AXIS', 'LINE', 'BAR', 'acc_title', 'acc_descr', 'acc_descr_multiline_value'",
                ));
            }
        }
        while s.kind().is_eol() {
            s.bump();
        }
    }
    Ok(())
}
