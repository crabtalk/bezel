//! Pele's `diagrams/xychart/parser.ts`.

use super::{
    db::{DataPoint, PlotKind, XyChartModel},
    lexer::tokenize,
    tokens::T,
};
use crate::diagrams::common::chart::ChartKind;
use crate::{detect::Kind, error::Error, js, token::Stream};

pub fn parse_xy_chart(src: &str, db: &mut XyChartModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut p = Parser {
        s: Stream::new(Kind::XyChart, src, &tokens),
    };

    loop {
        if p.s.kind().is_eol() {
            p.s.bump();
        } else if p.s.eat(T::Xychart) {
            if p.s.is(T::ChartOrientation) {
                db.set_orientation(p.s.take());
            }
        } else {
            break;
        }
    }

    while !p.s.is(T::End) {
        let t = p.s.kind();
        match t {
            T::Title => {
                p.s.bump();
                let title = p.text()?;
                db.titles.set_title(js::trim(&title));
            }
            T::XAxis | T::YAxis => {
                p.s.bump();
                let title = if p.s.kind().is_text() {
                    Some(p.text()?)
                } else {
                    None
                };
                if t == T::XAxis && p.s.is(T::SquareBracesStart) {
                    let mut categories = Vec::new();
                    loop {
                        p.s.bump();
                        categories.push(p.text()?);
                        if !p.s.is(T::Comma) {
                            break;
                        }
                    }
                    p.s.expect(T::SquareBracesEnd, "")?;
                    db.set_x_axis_band(&categories);
                } else if p.s.is(T::NumberWithDecimal) || title.is_none() {
                    let (min, max) = p.range()?;
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
                p.s.bump();
                let title = if p.s.is(T::SquareBracesStart) {
                    String::new()
                } else {
                    p.text()?
                };
                let data = p.points()?;
                let kind = if t == T::Line {
                    PlotKind::Line
                } else {
                    PlotKind::Bar
                };
                db.add_plot(kind, &title, &data);
            }
            T::AccTitle => {
                p.s.bump();
                db.titles
                    .set_acc_title(js::trim(p.s.expect(T::AccTitleValue, "")?));
            }
            T::AccDescr => {
                p.s.bump();
                db.titles
                    .set_acc_descr(js::trim(p.s.expect(T::AccDescrValue, "")?));
            }
            T::AccDescrMultilineValue => db.titles.set_acc_descr(js::trim(p.s.take())),
            _ => {
                return Err(p.s.fail(
                    "'title', 'X_AXIS', 'Y_AXIS', 'LINE', 'BAR', 'acc_title', 'acc_descr', 'acc_descr_multiline_value'",
                ));
            }
        }
        while p.s.kind().is_eol() {
            p.s.bump();
        }
    }
    Ok(())
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl<'t> Parser<'t, '_> {
    fn text(&mut self) -> Result<String, Error> {
        let first = self.s.kind();
        if !first.is_text() {
            return Err(self.s.fail("'STR', 'ALPHA', 'NUM'"));
        }
        let mut text = self.s.take().to_string();
        if first != T::Str {
            while self.s.kind().is_word() {
                text += self.s.take();
            }
        }
        Ok(text)
    }

    fn number(&mut self) -> Result<f64, Error> {
        Ok(js::number(self.s.expect(T::NumberWithDecimal, "")?))
    }

    fn range(&mut self) -> Result<(f64, f64), Error> {
        let min = self.number()?;
        self.s.expect(T::ArrowDelimiter, "")?;
        Ok((min, self.number()?))
    }

    fn points(&mut self) -> Result<Vec<DataPoint>, Error> {
        let mut out = Vec::new();
        self.s.expect(T::SquareBracesStart, "")?;
        loop {
            let value = self.number()?;
            let label = if self.s.is(T::Str) {
                self.s.take().to_string()
            } else {
                String::new()
            };
            out.push(DataPoint { value, label });
            if !self.s.eat(T::Comma) {
                break;
            }
        }
        self.s.expect(T::SquareBracesEnd, "")?;
        Ok(out)
    }
}
