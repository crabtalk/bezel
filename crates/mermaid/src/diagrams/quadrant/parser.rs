//! Pele's `diagrams/quadrant/parser.ts`.

use super::{
    db::{QuadrantModel, QuadrantText, TextKind},
    lexer::tokenize,
    tokens::{QUADRANTS, T},
};
use crate::diagrams::common::chart::ChartKind;
use crate::{detect::Kind, error::Error, js, token::Stream};

const TEXT_TOKENS: &str = "'STR', 'MD_STR', 'ALPHA', 'NUM', 'UNICODE_TEXT', 'PUNCTUATION'";

fn text(s: &mut Stream<'_, '_, T>) -> Result<QuadrantText, Error> {
    let first = s.kind();
    if !first.is_text() {
        return Err(s.fail(TEXT_TOKENS));
    }
    let mut text = s.take().to_string();
    while s.kind().is_word() || s.is_any(&[T::Space, T::Minus]) {
        text += s.take();
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

fn styles(s: &mut Stream<'_, '_, T>) -> Result<Vec<String>, Error> {
    let mut out = Vec::new();
    loop {
        if !s.kind().is_style() {
            return Err(s.fail("'ALPHA', 'NUM', 'COLON', 'SPACE', 'BRKT', 'MINUS'"));
        }
        let mut style = String::new();
        while s.kind().is_style() {
            style += s.take();
        }
        out.push(js::trim(&style).to_string());
        if !s.eat(T::Comma) {
            return Ok(out);
        }
    }
}

pub fn parse_quadrant(src: &str, db: &mut QuadrantModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::QuadrantChart, src, &tokens);

    while s.kind().is_eol() || s.is(T::Space) {
        s.bump();
    }
    s.expect(T::Quadrant, "")?;

    while !s.is(T::End) {
        s.eat_all(T::Space);
        let t = s.kind();
        match t {
            T::Classdef => {
                s.bump();
                s.expect(T::Space, "")?;
                if !s.kind().is_id() {
                    return Err(s.fail("'ALPHA', 'NUM', 'UNICODE_TEXT'"));
                }
                let mut id = String::new();
                while s.kind().is_id() {
                    id += s.take();
                }
                s.expect(T::Space, "")?;
                let list = styles(&mut s)?;
                db.add_class(id, &list)?;
            }
            T::XAxis | T::YAxis => {
                s.bump();
                let mut from = text(&mut s)?;
                let mut to = None;
                if s.eat(T::Delimiter) {
                    if s.kind().is_text() {
                        to = Some(text(&mut s)?);
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
                s.bump();
                let label = text(&mut s)?;
                let k = QUADRANTS.iter().position(|&q| q == t).unwrap_or_default();
                db.set_quadrant_text(k, label);
            }
            T::Title => {
                s.bump();
                db.titles.set_title(js::trim(s.expect(T::TitleValue, "")?));
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
            _ if t.is_text() => {
                let label = text(&mut s)?;
                let class_name = if s.is(T::ClassName) {
                    s.take().to_string()
                } else {
                    String::new()
                };
                s.expect(T::PointStart, "")?;
                let x = s.expect(T::PointX, "")?;
                let y = s.expect(T::PointY, "")?;
                let list = if s.kind().is_style() {
                    styles(&mut s)?
                } else {
                    Vec::new()
                };
                db.add_point(label, class_name, x, y, &list)?;
            }
            _ => {}
        }
        if !s.kind().is_eol() {
            return Err(s.fail("'NEWLINE', 'SEMI', 'EOF'"));
        }
        s.bump();
    }
    Ok(())
}
