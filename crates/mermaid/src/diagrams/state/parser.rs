//! Pele's `diagrams/state/parser.ts`: Mermaid's state grammar.

use super::{
    lexer::tokenize,
    tokens::T,
    types::{
        ClassDefStmt, ClickStmt, Descr, DirectionStmt, RelationStmt, StateModel, StateNote,
        StateStmt, Stmt, StyleStmt,
    },
};
use crate::{detect::Kind, error::Error, js, token::Stream};

const STATEMENT: &str = "'NL', 'ID', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STATE_DESCR', 'FORK', 'JOIN', 'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', 'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE'";

fn id_statement(s: &mut Stream<'_, '_, T>) -> Result<StateStmt, Error> {
    if !s.is_any(&[T::Id, T::EdgeState]) {
        return Err(s.fail("'ID', 'EDGE_STATE'"));
    }
    let id = js::trim(s.take()).to_string();
    let mut state = StateStmt::new(id, Some("default"));
    state.description = Some(Descr::One(String::new()));
    if !s.eat(T::StyleSeparator) {
        return Ok(state);
    }
    state.classes = Some(vec![js::trim(s.expect(T::Id, "")?).to_string()]);
    Ok(state)
}

/// Where a composite's statements go: the open composites, outermost first,
/// each with the document it sits in.
struct Open {
    parent: Vec<Stmt>,
    state: StateStmt,
}

pub fn parse_state(src: &str, db: &mut StateModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut s = Stream::new(Kind::State, src, &tokens);

    s.eat_all(T::Nl);
    if !s.eat(T::Sd) {
        return Err(s.fail("'NL', 'SD'"));
    }

    let mut doc: Vec<Stmt> = Vec::new();
    let mut outer: Vec<Open> = Vec::new();
    // The grammar drops a statement whose value is the string it uses for a
    // blank line.
    let add = |doc: &mut Vec<Stmt>, value: Stmt| {
        if !matches!(&value, Stmt::Text(text) if text == "nl") {
            doc.push(value);
        }
    };
    let open = |s: &mut Stream<'_, '_, T>,
                doc: &mut Vec<Stmt>,
                outer: &mut Vec<Open>,
                id: String,
                description: String| {
        s.bump();
        let mut state = StateStmt::new(id, Some("default"));
        state.description = Some(Descr::One(description));
        outer.push(Open {
            parent: std::mem::take(doc),
            state,
        });
    };

    loop {
        let t = s.kind();
        match t {
            T::Nl => s.bump(),
            T::Id | T::EdgeState => {
                let mut first = id_statement(&mut s)?;
                if s.is(T::Descr) {
                    first.description = Some(Descr::One(db.trim_colon(s.take())));
                    add(&mut doc, Stmt::State(Box::new(first)));
                } else if s.eat(T::Arrow) {
                    let second = id_statement(&mut s)?;
                    let description = if s.is(T::Descr) {
                        Some(db.trim_colon(s.take()))
                    } else {
                        None
                    };
                    add(
                        &mut doc,
                        Stmt::Relation(Box::new(RelationStmt {
                            state1: first,
                            state2: second,
                            description,
                        })),
                    );
                } else {
                    add(&mut doc, Stmt::State(Box::new(first)));
                }
            }
            T::HideEmpty => {
                add(&mut doc, Stmt::Text(s.take().to_string()));
            }
            T::Scale => {
                let text = s.take().to_string();
                s.expect(T::Width, "")?;
                add(&mut doc, Stmt::Text(text));
            }
            T::CompositState => {
                let text = s.take().to_string();
                if s.is(T::StructStart) {
                    open(&mut s, &mut doc, &mut outer, text, String::new());
                } else {
                    add(&mut doc, Stmt::Text(text));
                }
            }
            T::StateDescr => {
                let description = s.take().to_string();
                s.expect(T::As, "")?;
                let id = s.expect(T::Id, "")?;
                if s.is(T::StructStart) {
                    open(&mut s, &mut doc, &mut outer, id.to_string(), description);
                } else if id.contains(':') {
                    let parts: Vec<&str> = id.split(':').collect();
                    let mut state = StateStmt::new(parts[0], Some("default"));
                    state.description = Some(Descr::Many(vec![
                        js::trim(&description).to_string(),
                        parts[1].to_string(),
                    ]));
                    add(&mut doc, Stmt::State(Box::new(state)));
                } else {
                    let mut state = StateStmt::new(id, Some("default"));
                    state.description = Some(Descr::One(js::trim(&description).to_string()));
                    add(&mut doc, Stmt::State(Box::new(state)));
                }
            }
            T::Fork | T::Join | T::Choice => {
                let kind = match t {
                    T::Fork => "fork",
                    T::Join => "join",
                    _ => "choice",
                };
                add(
                    &mut doc,
                    Stmt::State(Box::new(StateStmt::new(s.text().to_string(), Some(kind)))),
                );
                s.bump();
            }
            T::Concurrent => {
                s.bump();
                add(
                    &mut doc,
                    Stmt::State(Box::new(StateStmt::new(
                        db.get_divider_id(),
                        Some("divider"),
                    ))),
                );
            }
            T::Note => {
                let keyword = s.take().to_string();
                if s.is(T::LeftOf) || s.is(T::RightOf) {
                    let position = js::trim(s.take()).to_string();
                    let id = js::trim(s.expect(T::Id, "")?).to_string();
                    let text = js::trim(s.expect(T::NoteText, "")?).to_string();
                    let mut state = StateStmt::new(id, None);
                    state.note = Some(StateNote {
                        position: Some(position),
                        text,
                    });
                    add(&mut doc, Stmt::State(Box::new(state)));
                } else {
                    if !s.eat(T::NoteText) {
                        return Err(s.fail("'NOTE_TEXT', 'left_of', 'right_of'"));
                    }
                    s.expect(T::As, "")?;
                    s.expect(T::Id, "")?;
                    add(&mut doc, Stmt::Text(keyword));
                }
            }
            T::DirectionTb | T::DirectionBt | T::DirectionRl | T::DirectionLr => {
                let value = match t {
                    T::DirectionTb => "TB",
                    T::DirectionBt => "BT",
                    T::DirectionRl => "RL",
                    _ => "LR",
                }
                .to_string();
                s.bump();
                // The grammar's setDirection lands in the root document the
                // parser has not handed over yet, which setRootDoc replaces.
                add(&mut doc, Stmt::Dir(DirectionStmt { value }));
            }
            T::AccTitle => {
                s.bump();
                let value = js::trim(s.expect(T::AccTitleValue, "")?).to_string();
                db.set_acc_title(&value);
                add(&mut doc, Stmt::Text(value));
            }
            T::AccDescr => {
                s.bump();
                let value = js::trim(s.expect(T::AccDescrValue, "")?).to_string();
                db.set_acc_description(&value);
                add(&mut doc, Stmt::Text(value));
            }
            T::AccDescrMultilineValue => {
                let value = js::trim(s.take()).to_string();
                db.set_acc_description(&value);
                add(&mut doc, Stmt::Text(value));
            }
            T::Click => {
                s.bump();
                let id = id_statement(&mut s)?;
                let url;
                let mut tooltip = "";
                if s.eat(T::Href) {
                    url = s.expect(T::String, "")?;
                } else {
                    if !s.is(T::String) {
                        return Err(s.fail("'STRING', 'HREF'"));
                    }
                    url = s.take();
                    tooltip = s.expect(T::String, "")?;
                }
                s.expect(T::Nl, "")?;
                add(
                    &mut doc,
                    Stmt::Click(Box::new(ClickStmt {
                        id,
                        url: url.to_string(),
                        tooltip: tooltip.to_string(),
                    })),
                );
            }
            T::Classdef => {
                s.bump();
                let id = js::trim(s.expect(T::ClassdefId, "")?).to_string();
                let classes = js::trim(s.expect(T::ClassdefStyleopts, "")?).to_string();
                add(&mut doc, Stmt::ClassDef(ClassDefStmt { id, classes }));
            }
            T::Style => {
                s.bump();
                let id = js::trim(s.expect(T::StyleIds, "")?).to_string();
                let style_class = js::trim(s.expect(T::StyledefStyleopts, "")?).to_string();
                add(&mut doc, Stmt::Style(StyleStmt { id, style_class }));
            }
            T::Class => {
                s.bump();
                let id = js::trim(s.expect(T::ClassentityIds, "")?).to_string();
                let style_class = js::trim(s.expect(T::Styleclass, "")?).to_string();
                add(&mut doc, Stmt::ApplyClass(StyleStmt { id, style_class }));
            }
            T::StructStop => {
                let Some(Open { parent, mut state }) = outer.pop() else {
                    return Err(s.fail(&format!("'$end', {STATEMENT}")));
                };
                s.bump();
                state.doc = Some(std::mem::replace(&mut doc, parent));
                doc.push(Stmt::State(Box::new(state)));
            }
            T::End => {
                if !outer.is_empty() {
                    return Err(s.fail(&format!("'STRUCT_STOP', {STATEMENT}")));
                }
                db.set_root_doc(doc)?;
                return Ok(());
            }
            _ => {
                return Err(s.fail(&if outer.is_empty() {
                    format!("'$end', {STATEMENT}")
                } else {
                    format!("'STRUCT_STOP', {STATEMENT}")
                }));
            }
        }
    }
}
