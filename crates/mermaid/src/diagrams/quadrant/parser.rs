//! Pele's `diagrams/quadrant/parser.ts`.

use super::{
    db::{QuadrantModel, QuadrantText, TextKind},
    lexer::tokenize,
    tokens::{QUADRANTS, T},
};
use crate::diagrams::common::chart::ChartKind;
use crate::{detect::Kind, error::Error, js, token::Stream};

const TEXT_TOKENS: &str = "'STR', 'MD_STR', 'ALPHA', 'NUM', 'UNICODE_TEXT', 'PUNCTUATION'";

pub fn parse_quadrant(src: &str, db: &mut QuadrantModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut p = Parser {
        s: Stream::new(Kind::QuadrantChart, src, &tokens),
    };

    while p.s.kind().is_eol() || p.s.is(T::Space) {
        p.s.bump();
    }
    p.s.expect(T::Quadrant, "")?;

    while !p.s.is(T::End) {
        p.s.eat_all(T::Space);
        let t = p.s.kind();
        match t {
            T::Classdef => {
                p.s.bump();
                p.s.expect(T::Space, "")?;
                if !p.s.kind().is_id() {
                    return Err(p.s.fail("'ALPHA', 'NUM', 'UNICODE_TEXT'"));
                }
                let mut id = String::new();
                while p.s.kind().is_id() {
                    id += p.s.take();
                }
                p.s.expect(T::Space, "")?;
                let list = p.styles()?;
                db.add_class(id, &list)?;
            }
            T::XAxis | T::YAxis => {
                p.s.bump();
                let mut from = p.text()?;
                let mut to = None;
                if p.s.eat(T::Delimiter) {
                    if p.s.kind().is_text() {
                        to = Some(p.text()?);
                    } else {
                        from.text.push_str(" ⟶ ");
                    }
                }
                if t == T::XAxis {
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
            T::Quadrant1 | T::Quadrant2 | T::Quadrant3 | T::Quadrant4 => {
                p.s.bump();
                let label = p.text()?;
                let k = QUADRANTS.iter().position(|&q| q == t).unwrap_or_default();
                db.set_quadrant_text(k, label);
            }
            T::Title => {
                p.s.bump();
                db.titles
                    .set_title(js::trim(p.s.expect(T::TitleValue, "")?));
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
            _ if t.is_text() => {
                let label = p.text()?;
                let class_name = if p.s.is(T::ClassName) {
                    p.s.take().to_string()
                } else {
                    String::new()
                };
                p.s.expect(T::PointStart, "")?;
                let x = p.s.expect(T::PointX, "")?;
                let y = p.s.expect(T::PointY, "")?;
                let list = if p.s.kind().is_style() {
                    p.styles()?
                } else {
                    Vec::new()
                };
                db.add_point(label, class_name, x, y, &list)?;
            }
            _ => {}
        }
        if !p.s.kind().is_eol() {
            return Err(p.s.fail("'NEWLINE', 'SEMI', 'EOF'"));
        }
        p.s.bump();
    }
    Ok(())
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
}

impl<'t> Parser<'t, '_> {
    fn text(&mut self) -> Result<QuadrantText, Error> {
        let first = self.s.kind();
        if !first.is_text() {
            return Err(self.s.fail(TEXT_TOKENS));
        }
        let mut text = self.s.take().to_string();
        while self.s.kind().is_word() || self.s.is_any(&[T::Space, T::Minus]) {
            text += self.s.take();
        }
        Ok(QuadrantText {
            text,
            kind: if first == T::MdStr {
                TextKind::Markdown
            } else {
                TextKind::Text
            },
        })
    }

    fn styles(&mut self) -> Result<Vec<String>, Error> {
        let mut out = Vec::new();
        loop {
            if !self.s.kind().is_style() {
                return Err(self
                    .s
                    .fail("'ALPHA', 'NUM', 'COLON', 'SPACE', 'BRKT', 'MINUS'"));
            }
            let mut style = String::new();
            while self.s.kind().is_style() {
                style += self.s.take();
            }
            out.push(js::trim(&style).to_string());
            if !self.s.eat(T::Comma) {
                return Ok(out);
            }
        }
    }
}
