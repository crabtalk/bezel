//! Pele's `diagrams/class/parser.ts`.

use super::{
    db::ClassModel,
    lexer::tokenize,
    tokens::T,
    types::{ClassRelation, Relation, RelationEnd},
};
use crate::{detect::Kind, error::Error, js, token::Stream};

const NAME_TOKENS: &str = "'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS', 'BQUOTE_STR'";

const ALPHANUM: [T; 3] = [T::UnicodeText, T::Alpha, T::Minus];
const STYLE_PART: [T; 6] = [T::Alpha, T::Colon, T::Brkt, T::Style, T::Pct, T::Label];

/// Mermaid's relationType number for an end marker.
fn end_type(t: T) -> Option<u8> {
    match t {
        T::Aggregation => Some(0),
        T::Extension => Some(1),
        T::Composition => Some(2),
        T::Dependency => Some(3),
        T::Lollipop => Some(4),
        _ => None,
    }
}

struct Frame {
    id: String,
    classes: Vec<String>,
    notes: Vec<String>,
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
    db: &'t mut ClassModel,
}

type Parsed<V> = Result<V, Error>;

impl<'t> Parser<'t, '_> {
    fn alpha_num(&mut self) -> Parsed<&'t str> {
        if !self.s.is_any(&ALPHANUM) {
            return Err(self.s.fail("'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS'"));
        }
        Ok(self.s.take())
    }

    fn name(&mut self, generic: bool) -> Parsed<String> {
        let mut out = String::new();
        loop {
            if self.s.is(T::BquoteStr) {
                out += self.s.take();
                break;
            }
            if !self.s.is_any(&ALPHANUM) {
                return Err(self.s.fail(NAME_TOKENS));
            }
            out += self.s.take();
            let t = self.s.kind();
            if t == T::Dot {
                out.push('.');
                self.s.bump();
            } else if !ALPHANUM.contains(&t) && t != T::BquoteStr {
                break;
            }
        }
        if generic && self.s.is(T::Generictype) {
            out.push('~');
            out += self.s.take();
            out.push('~');
        }
        Ok(out)
    }

    fn class_label(&mut self) -> Parsed<&'t str> {
        self.s.bump();
        let label = self.s.expect(T::Str, "")?;
        self.s.expect(T::Sqe, "")?;
        Ok(label)
    }

    fn members(&mut self) -> Parsed<Vec<&'t str>> {
        if !self.s.is(T::Member) {
            return Err(self.s.fail("'STRUCT_STOP', 'MEMBER'"));
        }
        let mut list = Vec::new();
        while self.s.is(T::Member) {
            list.push(self.s.take());
        }
        self.s.expect(T::StructStop, "")?;
        // The grammar collects members last to first, and add_members turns
        // them back.
        list.reverse();
        Ok(list)
    }

    fn class_statement(&mut self) -> Parsed<String> {
        self.s.bump();
        let id = self.name(true)?;
        self.db.add_class(&id);
        if self.s.is(T::Sqs) {
            let label = self.class_label()?;
            self.db.set_class_label(&id, label);
        }
        match self.s.kind() {
            T::StyleSeparator => {
                self.s.bump();
                let css = self.alpha_num()?;
                self.db.set_css_class(&id, css);
                if self.s.eat(T::StructStart) {
                    let members = self.members()?;
                    self.db.add_members(&id, members);
                }
            }
            T::StructStart => {
                self.s.bump();
                if self.s.eat(T::StructStop) {
                } else {
                    let members = self.members()?;
                    self.db.add_members(&id, members);
                }
            }
            T::AnnotationStart => {
                self.s.bump();
                let annotation = self.alpha_num()?;
                self.s.expect(T::AnnotationEnd, "")?;
                self.db.add_annotation(&id, annotation);
                if self.s.eat(T::StructStart) {
                    if self.s.eat(T::StructStop) {
                    } else {
                        let members = self.members()?;
                        self.db.add_members(&id, members);
                    }
                }
            }
            _ => {}
        }
        Ok(id)
    }

    fn note_statement(&mut self) -> Parsed<String> {
        let t = self.s.kind();
        self.s.bump();
        if t == T::Note {
            let text = self.s.expect(T::Str, "")?;
            return Ok(self.db.add_note(text, None));
        }
        let id = self.name(true)?;
        let text = self.s.expect(T::Str, "")?;
        Ok(self.db.add_note(text, Some(&id)))
    }

    /// Namespaces nest, so they are parsed with a stack of open ones.
    fn namespace_statement(&mut self) -> Parsed<()> {
        let mut open: Vec<Frame> = Vec::new();
        loop {
            self.s.bump();
            let ns = self.name(false)?;
            let id = if self.s.is(T::Sqs) {
                let label = self.class_label()?;
                self.db.add_namespace(&ns, Some(label))
            } else {
                self.db.add_namespace(&ns, None)
            };
            self.s.expect(T::StructStart, "")?;
            self.s.eat(T::Newline);
            open.push(Frame {
                id,
                classes: Vec::new(),
                notes: Vec::new(),
            });
            let mut item = true;
            while item {
                let t = self.s.kind();
                if t == T::Namespace {
                    break;
                }
                if t == T::Class {
                    let id = self.class_statement()?;
                    open.last_mut().expect("a frame").classes.push(id);
                } else if t == T::Note || t == T::NoteFor {
                    let id = self.note_statement()?;
                    open.last_mut().expect("a frame").notes.push(id);
                } else {
                    return Err(self.s.fail("'NAMESPACE', 'CLASS', 'NOTE_FOR', 'NOTE'"));
                }
                loop {
                    if self.s.is(T::Newline) {
                        let next = self.s.peek(1);
                        self.s.bump();
                        if matches!(next, T::Namespace | T::Class | T::Note | T::NoteFor) {
                            break;
                        }
                    }
                    self.s.expect(T::StructStop, "")?;
                    let done = open.pop().expect("a frame");
                    self.db
                        .add_classes_to_namespace(&done.id, &done.classes, &done.notes);
                    self.db.pop_namespace();
                    if open.is_empty() {
                        item = false;
                        break;
                    }
                }
            }
            if open.is_empty() {
                return Ok(());
            }
        }
    }

    fn relation_end(&mut self) -> RelationEnd {
        let Some(kind) = end_type(self.s.kind()) else {
            return RelationEnd::None;
        };
        self.s.bump();
        RelationEnd::Type(kind)
    }

    fn class_or_relation(&mut self) -> Parsed<()> {
        let id1 = self.name(true)?;
        let t = self.s.kind();
        if t == T::Label {
            let text = self.s.take();
            let label = self.db.cleanup_label(text);
            self.db.add_member(&id1, &label);
            return Ok(());
        }
        let mut title1 = "none";
        if t == T::Str {
            title1 = self.s.take();
        } else if end_type(t).is_none() && t != T::Line && t != T::DottedLine {
            return Ok(());
        }
        let type1 = self.relation_end();
        let t = self.s.kind();
        if t != T::Line && t != T::DottedLine {
            return Err(self.s.fail(
                "'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'LINE', 'DOTTED_LINE'",
            ));
        }
        self.s.bump();
        let type2 = self.relation_end();
        let title2 = if self.s.is(T::Str) {
            self.s.take()
        } else {
            "none"
        };
        let id2 = self.name(true)?;
        let mut relation = ClassRelation {
            id1,
            id2,
            relation: Relation {
                type1,
                type2,
                line_type: if t == T::Line { 0 } else { 1 },
            },
            relation_title1: title1.to_string(),
            relation_title2: title2.to_string(),
            title: None,
        };
        if self.s.is(T::Label) {
            let text = self.s.take();
            relation.title = Some(self.db.cleanup_label(text));
        }
        self.db.add_relation(relation);
        Ok(())
    }

    fn styles(&mut self) -> Parsed<Vec<String>> {
        let mut out = Vec::new();
        loop {
            if !self.s.is_any(&STYLE_PART) {
                return Err(self.s.fail(
                    "'ALPHA', 'NUM', 'COLON', 'UNIT', 'SPACE', 'BRKT', 'STYLE', 'PCT', 'LABEL'",
                ));
            }
            let mut s = self.s.take().to_string();
            while self.s.is_any(&STYLE_PART) {
                s += self.s.take();
            }
            out.push(s);
            if !self.s.eat(T::Comma) {
                return Ok(out);
            }
        }
    }

    fn tooltip(&mut self, id: &str) {
        if self.s.is(T::Str) {
            let tip = self.s.take();
            self.db.set_tooltip(id, Some(tip));
        }
    }

    fn link_statement(&mut self, id: &str) -> Parsed<()> {
        let href = self.s.expect(T::Str, "")?;
        let tip = if self.s.is(T::Str) {
            Some(self.s.take())
        } else {
            None
        };
        if self.s.is(T::LinkTarget) {
            let target = self.s.take();
            self.db.set_link(id, href, Some(target));
        } else {
            self.db.set_link(id, href, None);
        }
        if tip.is_some() {
            self.db.set_tooltip(id, tip);
        }
        Ok(())
    }

    fn statement(&mut self) -> Parsed<()> {
        let t = self.s.kind();
        match t {
            T::UnicodeText | T::Alpha | T::Minus | T::BquoteStr => return self.class_or_relation(),
            T::Namespace => return self.namespace_statement(),
            T::Class => return self.class_statement().map(|_| ()),
            T::Member => {
                self.s.bump();
                return Ok(());
            }
            T::AnnotationStart => {
                self.s.bump();
                let annotation = self.alpha_num()?;
                self.s.expect(T::AnnotationEnd, "")?;
                let name = self.name(true)?;
                self.db.add_annotation(&name, annotation);
                return Ok(());
            }
            T::Callback => {
                self.s.bump();
                let id = self.name(true)?;
                self.s.expect(T::Str, "")?;
                self.db.set_click_event(&id);
                self.tooltip(&id);
                return Ok(());
            }
            T::Link => {
                self.s.bump();
                let id = self.name(true)?;
                return self.link_statement(&id);
            }
            T::Click => {
                self.s.bump();
                let id = self.name(true)?;
                if self.s.eat(T::Href) {
                    return self.link_statement(&id);
                }
                self.s.expect(T::CallbackName, "")?;
                self.s.eat(T::CallbackArgs);
                self.db.set_click_event(&id);
                self.tooltip(&id);
                return Ok(());
            }
            T::Style => {
                self.s.bump();
                let id = self.s.expect(T::Alpha, "")?;
                let styles = self.styles()?;
                self.db.set_css_style(id, &styles);
                return Ok(());
            }
            T::Cssclass => {
                self.s.bump();
                let ids = self.s.expect(T::Str, "")?;
                let name = self.s.expect(T::Alpha, "")?;
                self.db.set_css_class(ids, name);
                return Ok(());
            }
            T::Note | T::NoteFor => return self.note_statement().map(|_| ()),
            T::Classdef => {
                self.s.bump();
                let mut ids = vec![self.s.expect(T::Alpha, "")?];
                while self.s.is(T::Comma) {
                    self.s.bump();
                    ids.push(self.s.expect(T::Alpha, "")?);
                }
                let styles = self.styles()?;
                self.db.define_class(&ids, &styles);
                return Ok(());
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                self.s.bump();
                self.db.set_direction(match t {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    _ => "LR",
                });
                return Ok(());
            }
            T::AccTitle => {
                self.s.bump();
                let text = self.s.expect(T::AccTitleValue, "")?;
                self.db.set_acc_title(js::trim(text));
                return Ok(());
            }
            T::AccDescr => {
                self.s.bump();
                let text = self.s.expect(T::AccDescrValue, "")?;
                self.db.set_acc_description(js::trim(text));
                return Ok(());
            }
            T::AccDescrMultilineValue => {
                let text = self.s.take();
                self.db.set_acc_description(js::trim(text));
                return Ok(());
            }
            _ => {}
        }
        Err(self.s.fail(
            "'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'NAMESPACE', 'ANNOTATION_START', 'CLASS', 'MEMBER', 'NOTE_FOR', 'NOTE', 'CLASSDEF', 'direction_tb', 'CALLBACK', 'LINK', 'CLICK', 'STYLE', 'CSSCLASS', 'ALPHA', 'MINUS', 'BQUOTE_STR'",
        ))
    }
}

pub fn parse_class_diagram(src: &str, db: &mut ClassModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        s: Stream::new(Kind::Class, src, &tokens),
        db,
    };

    // Mermaid's grammar also accepts statements with no `classDiagram` line
    // before them.
    let header = parser.s.eat(T::ClassDiagram);
    if header {
        parser.s.expect(T::Newline, "")?;
    }
    loop {
        parser.statement()?;
        if !parser.s.eat(T::Newline) || parser.s.is_any(&[T::Eof, T::End]) {
            break;
        }
    }
    if header {
        parser.s.expect(T::Eof, "")?;
    }
    parser.s.expect(T::End, "")?;
    Ok(())
}
