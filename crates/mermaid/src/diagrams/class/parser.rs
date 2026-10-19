//! Pele's `diagrams/class/parser.ts`.

use std::sync::LazyLock;

use super::{
    db::ClassModel,
    lexer::{Tokens, tokenize},
    tokens::*,
    types::{ClassRelation, Relation, RelationEnd},
};
use crate::{detect::Kind, error::Error, js};

const NAME_TOKENS: &str = "'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS', 'BQUOTE_STR'";

static ALPHANUM: LazyLock<[u8; COUNT]> = LazyLock::new(|| table(&[UNICODE_TEXT, ALPHA, MINUS]));
static STYLE_PART: LazyLock<[u8; COUNT]> =
    LazyLock::new(|| table(&[ALPHA, COLON, BRKT, STYLE, PCT, LABEL]));
/// Mermaid's relationType numbers, plus one so that zero means "not an end
/// marker".
static END_TYPE: LazyLock<[u8; COUNT]> = LazyLock::new(|| {
    let mut t = [0; COUNT];
    t[AGGREGATION as usize] = 1;
    t[EXTENSION as usize] = 2;
    t[COMPOSITION as usize] = 3;
    t[DEPENDENCY as usize] = 4;
    t[LOLLIPOP as usize] = 5;
    t
});

fn table(kinds: &[u8]) -> [u8; COUNT] {
    let mut t = [0; COUNT];
    for &kind in kinds {
        t[kind as usize] = 1;
    }
    t
}

struct Frame {
    id: String,
    classes: Vec<String>,
    notes: Vec<String>,
}

struct Parser<'t, 's> {
    tokens: &'t Tokens<'s>,
    src: &'s str,
    db: &'t mut ClassModel,
    i: usize,
}

type Parsed<T> = Result<T, Error>;

impl<'s> Parser<'_, 's> {
    fn ty(&self, i: usize) -> u8 {
        self.tokens.types[i]
    }

    fn t(&self) -> u8 {
        self.ty(self.i)
    }

    fn text(&self) -> &'s str {
        self.tokens.texts[self.i]
    }

    /// The text of the current token, and the parser past it.
    fn take(&mut self) -> &'s str {
        let text = self.text();
        self.i += 1;
        text
    }

    fn fail(&self, expected: &str) -> Error {
        let t = self.t();
        let start = self.tokens.starts[self.i];
        if t == ERROR {
            return Error::at(Kind::Class, self.src, start, "", true);
        }
        Error::at(
            Kind::Class,
            self.src,
            start,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            false,
        )
    }

    fn expect(&mut self, kind: u8) -> Parsed<&'s str> {
        if self.t() != kind {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[kind as usize])));
        }
        Ok(self.take())
    }

    fn alpha_num(&mut self) -> Parsed<&'s str> {
        if ALPHANUM[self.t() as usize] == 0 {
            return Err(self.fail("'UNICODE_TEXT', 'NUM', 'ALPHA', 'MINUS'"));
        }
        Ok(self.take())
    }

    fn name(&mut self, generic: bool) -> Parsed<String> {
        let mut out = String::new();
        loop {
            if self.t() == BQUOTE_STR {
                out += self.take();
                break;
            }
            if ALPHANUM[self.t() as usize] == 0 {
                return Err(self.fail(NAME_TOKENS));
            }
            out += self.take();
            let t = self.t();
            if t == DOT {
                out.push('.');
                self.i += 1;
            } else if ALPHANUM[t as usize] == 0 && t != BQUOTE_STR {
                break;
            }
        }
        if generic && self.t() == GENERICTYPE {
            out.push('~');
            out += self.take();
            out.push('~');
        }
        Ok(out)
    }

    fn class_label(&mut self) -> Parsed<&'s str> {
        self.i += 1;
        let label = self.expect(STR)?;
        self.expect(SQE)?;
        Ok(label)
    }

    fn members(&mut self) -> Parsed<Vec<&'s str>> {
        if self.t() != MEMBER {
            return Err(self.fail("'STRUCT_STOP', 'MEMBER'"));
        }
        let mut list = Vec::new();
        while self.t() == MEMBER {
            list.push(self.take());
        }
        self.expect(STRUCT_STOP)?;
        // The grammar collects members last to first, and add_members turns
        // them back.
        list.reverse();
        Ok(list)
    }

    fn class_statement(&mut self) -> Parsed<String> {
        self.i += 1;
        let id = self.name(true)?;
        self.db.add_class(&id);
        if self.t() == SQS {
            let label = self.class_label()?;
            self.db.set_class_label(&id, label);
        }
        match self.t() {
            STYLE_SEPARATOR => {
                self.i += 1;
                let css = self.alpha_num()?;
                self.db.set_css_class(&id, css);
                if self.t() == STRUCT_START {
                    self.i += 1;
                    let members = self.members()?;
                    self.db.add_members(&id, members);
                }
            }
            STRUCT_START => {
                self.i += 1;
                if self.t() == STRUCT_STOP {
                    self.i += 1;
                } else {
                    let members = self.members()?;
                    self.db.add_members(&id, members);
                }
            }
            ANNOTATION_START => {
                self.i += 1;
                let annotation = self.alpha_num()?;
                self.expect(ANNOTATION_END)?;
                self.db.add_annotation(&id, annotation);
                if self.t() == STRUCT_START {
                    self.i += 1;
                    if self.t() == STRUCT_STOP {
                        self.i += 1;
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
        let t = self.t();
        self.i += 1;
        if t == NOTE {
            let text = self.expect(STR)?;
            return Ok(self.db.add_note(text, None));
        }
        let id = self.name(true)?;
        let text = self.expect(STR)?;
        Ok(self.db.add_note(text, Some(&id)))
    }

    /// Namespaces nest, so they are parsed with a stack of open ones.
    fn namespace_statement(&mut self) -> Parsed<()> {
        let mut open: Vec<Frame> = Vec::new();
        loop {
            self.i += 1;
            let ns = self.name(false)?;
            let id = if self.t() == SQS {
                let label = self.class_label()?;
                self.db.add_namespace(&ns, Some(label))
            } else {
                self.db.add_namespace(&ns, None)
            };
            self.expect(STRUCT_START)?;
            if self.t() == NEWLINE {
                self.i += 1;
            }
            open.push(Frame {
                id,
                classes: Vec::new(),
                notes: Vec::new(),
            });
            let mut item = true;
            while item {
                let t = self.t();
                if t == NAMESPACE {
                    break;
                }
                if t == CLASS {
                    let id = self.class_statement()?;
                    open.last_mut().expect("a frame").classes.push(id);
                } else if t == NOTE || t == NOTE_FOR {
                    let id = self.note_statement()?;
                    open.last_mut().expect("a frame").notes.push(id);
                } else {
                    return Err(self.fail("'NAMESPACE', 'CLASS', 'NOTE_FOR', 'NOTE'"));
                }
                loop {
                    if self.t() == NEWLINE {
                        let next = self.ty(self.i + 1);
                        if next == NAMESPACE || next == CLASS || next == NOTE || next == NOTE_FOR {
                            self.i += 1;
                            break;
                        }
                        self.i += 1;
                    }
                    self.expect(STRUCT_STOP)?;
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
        let kind = END_TYPE[self.t() as usize];
        if kind == 0 {
            return RelationEnd::None;
        }
        self.i += 1;
        RelationEnd::Type(kind - 1)
    }

    fn class_or_relation(&mut self) -> Parsed<()> {
        let id1 = self.name(true)?;
        let t = self.t();
        if t == LABEL {
            let text = self.take();
            let label = self.db.cleanup_label(text);
            self.db.add_member(&id1, &label);
            return Ok(());
        }
        let mut title1 = "none";
        if t == STR {
            title1 = self.take();
        } else if END_TYPE[t as usize] == 0 && t != LINE && t != DOTTED_LINE {
            return Ok(());
        }
        let type1 = self.relation_end();
        let t = self.t();
        if t != LINE && t != DOTTED_LINE {
            return Err(self.fail(
                "'AGGREGATION', 'EXTENSION', 'COMPOSITION', 'DEPENDENCY', 'LOLLIPOP', 'LINE', 'DOTTED_LINE'",
            ));
        }
        self.i += 1;
        let type2 = self.relation_end();
        let title2 = if self.t() == STR { self.take() } else { "none" };
        let id2 = self.name(true)?;
        let mut relation = ClassRelation {
            id1,
            id2,
            relation: Relation {
                type1,
                type2,
                line_type: if t == LINE { 0 } else { 1 },
            },
            relation_title1: title1.to_string(),
            relation_title2: title2.to_string(),
            title: None,
        };
        if self.t() == LABEL {
            let text = self.take();
            relation.title = Some(self.db.cleanup_label(text));
        }
        self.db.add_relation(relation);
        Ok(())
    }

    fn styles(&mut self) -> Parsed<Vec<String>> {
        let mut out = Vec::new();
        loop {
            if STYLE_PART[self.t() as usize] == 0 {
                return Err(self.fail(
                    "'ALPHA', 'NUM', 'COLON', 'UNIT', 'SPACE', 'BRKT', 'STYLE', 'PCT', 'LABEL'",
                ));
            }
            let mut s = self.take().to_string();
            while STYLE_PART[self.t() as usize] == 1 {
                s += self.take();
            }
            out.push(s);
            if self.t() != COMMA {
                return Ok(out);
            }
            self.i += 1;
        }
    }

    fn tooltip(&mut self, id: &str) {
        if self.t() == STR {
            let tip = self.take();
            self.db.set_tooltip(id, Some(tip));
        }
    }

    fn link_statement(&mut self, id: &str) -> Parsed<()> {
        let href = self.expect(STR)?;
        let tip = if self.t() == STR {
            Some(self.take())
        } else {
            None
        };
        if self.t() == LINK_TARGET {
            let target = self.take();
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
        let t = self.t();
        match t {
            UNICODE_TEXT | ALPHA | MINUS | BQUOTE_STR => return self.class_or_relation(),
            NAMESPACE => return self.namespace_statement(),
            CLASS => return self.class_statement().map(|_| ()),
            MEMBER => {
                self.i += 1;
                return Ok(());
            }
            ANNOTATION_START => {
                self.i += 1;
                let annotation = self.alpha_num()?;
                self.expect(ANNOTATION_END)?;
                let name = self.name(true)?;
                self.db.add_annotation(&name, annotation);
                return Ok(());
            }
            CALLBACK => {
                self.i += 1;
                let id = self.name(true)?;
                self.expect(STR)?;
                self.db.set_click_event(&id);
                self.tooltip(&id);
                return Ok(());
            }
            LINK => {
                self.i += 1;
                let id = self.name(true)?;
                return self.link_statement(&id);
            }
            CLICK => {
                self.i += 1;
                let id = self.name(true)?;
                if self.t() == HREF {
                    self.i += 1;
                    return self.link_statement(&id);
                }
                self.expect(CALLBACK_NAME)?;
                if self.t() == CALLBACK_ARGS {
                    self.i += 1;
                }
                self.db.set_click_event(&id);
                self.tooltip(&id);
                return Ok(());
            }
            STYLE => {
                self.i += 1;
                let id = self.expect(ALPHA)?;
                let styles = self.styles()?;
                self.db.set_css_style(id, &styles);
                return Ok(());
            }
            CSSCLASS => {
                self.i += 1;
                let ids = self.expect(STR)?;
                let name = self.expect(ALPHA)?;
                self.db.set_css_class(ids, name);
                return Ok(());
            }
            NOTE | NOTE_FOR => return self.note_statement().map(|_| ()),
            CLASSDEF => {
                self.i += 1;
                let mut ids = vec![self.expect(ALPHA)?];
                while self.t() == COMMA {
                    self.i += 1;
                    ids.push(self.expect(ALPHA)?);
                }
                let styles = self.styles()?;
                self.db.define_class(&ids, &styles);
                return Ok(());
            }
            DIRECTION_TB | DIRECTION_BT | DIRECTION_RL | DIRECTION_LR => {
                self.i += 1;
                self.db.set_direction(match t {
                    DIRECTION_TB => "TB",
                    DIRECTION_BT => "BT",
                    DIRECTION_RL => "RL",
                    _ => "LR",
                });
                return Ok(());
            }
            ACC_TITLE => {
                self.i += 1;
                let text = self.expect(ACC_TITLE_VALUE)?;
                self.db.set_acc_title(js::trim(text));
                return Ok(());
            }
            ACC_DESCR => {
                self.i += 1;
                let text = self.expect(ACC_DESCR_VALUE)?;
                self.db.set_acc_description(js::trim(text));
                return Ok(());
            }
            ACC_DESCR_MULTILINE_VALUE => {
                let text = self.take();
                self.db.set_acc_description(js::trim(text));
                return Ok(());
            }
            _ => {}
        }
        Err(self.fail(
            "'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'NAMESPACE', 'ANNOTATION_START', 'CLASS', 'MEMBER', 'NOTE_FOR', 'NOTE', 'CLASSDEF', 'direction_tb', 'CALLBACK', 'LINK', 'CLICK', 'STYLE', 'CSSCLASS', 'ALPHA', 'MINUS', 'BQUOTE_STR'",
        ))
    }
}

pub fn parse_class_diagram(src: &str, db: &mut ClassModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        tokens: &tokens,
        src,
        db,
        i: 0,
    };

    // Mermaid's grammar also accepts statements with no `classDiagram` line
    // before them.
    let header = parser.t() == CLASS_DIAGRAM;
    if header {
        parser.i += 1;
        parser.expect(NEWLINE)?;
    }
    loop {
        parser.statement()?;
        if parser.t() != NEWLINE {
            break;
        }
        parser.i += 1;
        if parser.t() == EOF || parser.t() == END {
            break;
        }
    }
    if header {
        parser.expect(EOF)?;
    }
    parser.expect(END)?;
    Ok(())
}
