//! Pele's `diagrams/state/parser.ts`: Mermaid's state grammar.

use super::{
    lexer::tokenize,
    tokens::T,
    types::{
        ClassDefStmt, ClickStmt, Descr, DirectionStmt, RelationStmt, StateKind, StateModel,
        StateNote, StateStmt, Stmt, StyleStmt,
    },
};
use crate::{detect::Kind, error::Error, geom::Direction, js, token::Stream};

const STATEMENT: &str = "'NL', 'ID', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STATE_DESCR', 'FORK', 'JOIN', 'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', 'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE'";

/// A description without the colon before it.
fn trim_colon(text: &str) -> String {
    js::trim(text.strip_prefix(':').unwrap_or(text)).to_string()
}

/// A composite being read, and the document it sits in.
struct Open {
    parent: Vec<Stmt>,
    state: StateStmt,
}

struct Parser<'t, 's> {
    s: Stream<'t, 's, T>,
    db: &'t mut StateModel,
    /// The document being read.
    doc: Vec<Stmt>,
    /// The composites around it, outermost first.
    outer: Vec<Open>,
}

impl Parser<'_, '_> {
    /// Adds a statement to the document. The grammar drops a statement whose
    /// value is the string it uses for a blank line.
    fn add(&mut self, value: Stmt) {
        if !matches!(&value, Stmt::Text(text) if text == "nl") {
            self.doc.push(value);
        }
    }

    fn add_state(&mut self, state: StateStmt) {
        self.add(Stmt::State(Box::new(state)));
    }

    /// Steps past `{` and starts the document of the composite `id`.
    fn open(&mut self, id: String, description: String) {
        self.s.bump();
        let mut state = StateStmt::new(id, Some(StateKind::Default));
        state.description = Some(Descr::One(description));
        let parent = std::mem::take(&mut self.doc);
        self.outer.push(Open { parent, state });
    }

    fn id_statement(&mut self) -> Result<StateStmt, Error> {
        if !self.s.is_any(&[T::Id, T::EdgeState]) {
            return Err(self.s.fail("'ID', 'EDGE_STATE'"));
        }
        let id = js::trim(self.s.take()).to_string();
        let mut state = StateStmt::new(id, Some(StateKind::Default));
        state.description = Some(Descr::One(String::new()));
        if !self.s.eat(T::StyleSeparator) {
            return Ok(state);
        }
        state.classes = Some(vec![js::trim(self.s.expect(T::Id, "")?).to_string()]);
        Ok(state)
    }

    /// A state, or a transition from it.
    fn state_or_relation(&mut self) -> Result<(), Error> {
        let mut first = self.id_statement()?;
        if self.s.is(T::Descr) {
            first.description = Some(Descr::One(trim_colon(self.s.take())));
            self.add_state(first);
        } else if self.s.eat(T::Arrow) {
            let second = self.id_statement()?;
            let description = self.s.is(T::Descr).then(|| trim_colon(self.s.take()));
            self.add(Stmt::Relation(Box::new(RelationStmt {
                state1: first,
                state2: second,
                description,
            })));
        } else {
            self.add_state(first);
        }
        Ok(())
    }

    /// `state "description" as id`, opening a composite or not.
    fn described_state(&mut self) -> Result<(), Error> {
        let description = self.s.take().to_string();
        self.s.expect(T::As, "")?;
        let id = self.s.expect(T::Id, "")?;
        if self.s.is(T::StructStart) {
            self.open(id.to_string(), description);
            return Ok(());
        }
        let mut state;
        if let Some((id, second)) = id.split_once(':') {
            // Mermaid keeps the part after the first colon up to the next.
            let second = second.split(':').next().unwrap_or("");
            state = StateStmt::new(id, Some(StateKind::Default));
            state.description = Some(Descr::Many(vec![
                js::trim(&description).to_string(),
                second.to_string(),
            ]));
        } else {
            state = StateStmt::new(id, Some(StateKind::Default));
            state.description = Some(Descr::One(js::trim(&description).to_string()));
        }
        self.add_state(state);
        Ok(())
    }

    fn note(&mut self) -> Result<(), Error> {
        let keyword = self.s.take().to_string();
        if self.s.is_any(&[T::LeftOf, T::RightOf]) {
            let position = js::trim(self.s.take()).to_string();
            let id = js::trim(self.s.expect(T::Id, "")?).to_string();
            let text = js::trim(self.s.expect(T::NoteText, "")?).to_string();
            let mut state = StateStmt::new(id, None);
            state.note = Some(StateNote {
                position: Some(position),
                text,
            });
            self.add_state(state);
            return Ok(());
        }
        if !self.s.eat(T::NoteText) {
            return Err(self.s.fail("'NOTE_TEXT', 'left_of', 'right_of'"));
        }
        self.s.expect(T::As, "")?;
        self.s.expect(T::Id, "")?;
        self.add(Stmt::Text(keyword));
        Ok(())
    }

    fn click(&mut self) -> Result<(), Error> {
        self.s.bump();
        let id = self.id_statement()?;
        let url;
        let mut tooltip = "";
        if self.s.eat(T::Href) {
            url = self.s.expect(T::String, "")?;
        } else {
            if !self.s.is(T::String) {
                return Err(self.s.fail("'STRING', 'HREF'"));
            }
            url = self.s.take();
            tooltip = self.s.expect(T::String, "")?;
        }
        self.s.expect(T::Nl, "")?;
        self.add(Stmt::Click(Box::new(ClickStmt {
            id,
            url: url.to_string(),
            tooltip: tooltip.to_string(),
        })));
        Ok(())
    }

    fn acc_descr(&mut self, value: &str) {
        let value = js::trim(value).to_string();
        self.db.titles.set_acc_descr(&value);
        self.add(Stmt::Text(value));
    }

    /// A keyword's two values, trimmed.
    fn pair(&mut self, first: T, second: T) -> Result<(String, String), Error> {
        self.s.bump();
        let first = js::trim(self.s.expect(first, "")?).to_string();
        let second = js::trim(self.s.expect(second, "")?).to_string();
        Ok((first, second))
    }

    /// What the parser expects where no statement starts.
    fn fail_statement(&self) -> Error {
        self.s.fail(&if self.outer.is_empty() {
            format!("'$end', {STATEMENT}")
        } else {
            format!("'STRUCT_STOP', {STATEMENT}")
        })
    }

    fn document(&mut self) -> Result<Vec<Stmt>, Error> {
        loop {
            let t = self.s.kind();
            match t {
                T::Nl => self.s.bump(),
                T::Id | T::EdgeState => self.state_or_relation()?,
                T::HideEmpty => {
                    let text = self.s.take().to_string();
                    self.add(Stmt::Text(text));
                }
                T::Scale => {
                    let text = self.s.take().to_string();
                    self.s.expect(T::Width, "")?;
                    self.add(Stmt::Text(text));
                }
                T::CompositState => {
                    let text = self.s.take().to_string();
                    if self.s.is(T::StructStart) {
                        self.open(text, String::new());
                    } else {
                        self.add(Stmt::Text(text));
                    }
                }
                T::StateDescr => self.described_state()?,
                T::Fork | T::Join | T::Choice => {
                    let kind = match t {
                        T::Fork => StateKind::Fork,
                        T::Join => StateKind::Join,
                        _ => StateKind::Choice,
                    };
                    let id = self.s.take().to_string();
                    self.add_state(StateStmt::new(id, Some(kind)));
                }
                T::Concurrent => {
                    self.s.bump();
                    let id = self.db.next_divider_id();
                    self.add_state(StateStmt::new(id, Some(StateKind::Divider)));
                }
                T::Note => self.note()?,
                T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                    let value = match t {
                        T::DirectionTb => Direction::TopToBottom,
                        T::DirectionBt => Direction::BottomToTop,
                        T::DirectionRl => Direction::RightToLeft,
                        _ => Direction::LeftToRight,
                    };
                    self.s.bump();
                    // The grammar's setDirection lands in the root document
                    // the parser has not handed over yet, which setRootDoc
                    // replaces.
                    self.add(Stmt::Dir(DirectionStmt { value }));
                }
                T::AccTitle => {
                    self.s.bump();
                    let value = js::trim(self.s.expect(T::AccTitleValue, "")?).to_string();
                    self.db.titles.set_acc_title(&value);
                    self.add(Stmt::Text(value));
                }
                T::AccDescr => {
                    self.s.bump();
                    let value = self.s.expect(T::AccDescrValue, "")?;
                    self.acc_descr(value);
                }
                T::AccDescrMultilineValue => {
                    let value = self.s.take();
                    self.acc_descr(value);
                }
                T::Click => self.click()?,
                T::Classdef => {
                    let (id, classes) = self.pair(T::ClassdefId, T::ClassdefStyleopts)?;
                    self.add(Stmt::ClassDef(ClassDefStmt { id, classes }));
                }
                T::Style => {
                    let (id, style_class) = self.pair(T::StyleIds, T::StyledefStyleopts)?;
                    self.add(Stmt::Style(StyleStmt { id, style_class }));
                }
                T::Class => {
                    let (id, style_class) = self.pair(T::ClassentityIds, T::Styleclass)?;
                    self.add(Stmt::ApplyClass(StyleStmt { id, style_class }));
                }
                T::StructStop if !self.outer.is_empty() => {
                    self.s.bump();
                    let Open { parent, mut state } = self.outer.pop().expect("an open composite");
                    state.doc = Some(std::mem::replace(&mut self.doc, parent));
                    self.doc.push(Stmt::State(Box::new(state)));
                }
                T::End if self.outer.is_empty() => return Ok(std::mem::take(&mut self.doc)),
                _ => return Err(self.fail_statement()),
            }
        }
    }
}

pub fn parse_state(src: &str, db: &mut StateModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        s: Stream::new(Kind::State, src, &tokens),
        db,
        doc: Vec::new(),
        outer: Vec::new(),
    };
    parser.s.eat_all(T::Nl);
    if !parser.s.eat(T::Sd) {
        return Err(parser.s.fail("'NL', 'SD'"));
    }
    let doc = parser.document()?;
    parser.db.set_root_doc(doc)
}
