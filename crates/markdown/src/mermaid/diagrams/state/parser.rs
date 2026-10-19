//! Pele's `diagrams/state/parser.ts`: Mermaid's state grammar.

use super::{
    lexer::tokenize,
    tokens::*,
    types::{
        ClassDefStmt, ClickStmt, Descr, DirectionStmt, RelationStmt, StateModel, StateNote,
        StateStmt, Stmt, StyleStmt,
    },
};
use crate::mermaid::{
    detect::Kind,
    error::{Error, Place},
    js,
};

const DIRECTIONS: [&str; 4] = ["TB", "BT", "RL", "LR"];
const STATEMENT: &str = "'NL', 'ID', 'HIDE_EMPTY', 'scale', 'COMPOSIT_STATE', 'STATE_DESCR', 'FORK', 'JOIN', 'CHOICE', 'CONCURRENT', 'note', 'acc_title', 'acc_descr', 'acc_descr_multiline_value', 'CLICK', 'classDef', 'style', 'class', 'direction_tb', 'direction_bt', 'direction_rl', 'direction_lr', 'EDGE_STATE'";

struct Parser<'s> {
    src: &'s str,
    types: Vec<u8>,
    texts: Vec<String>,
    starts: Vec<usize>,
    error: Option<String>,
    i: usize,
}

impl Parser<'_> {
    fn fail(&self, expected: &str) -> Error {
        let t = self.types[self.i];
        let start = self.starts[self.i];
        if t == ERROR {
            if let Some(message) = &self.error {
                let place = Place::locate(self.src, start);
                let mut error = Error::syntax(message.clone()).of(Kind::State);
                error.line = place.line;
                error.column = place.column;
                error.snippet = place.snippet;
                return error;
            }
            return Error::at(Kind::State, self.src, start, "", true);
        }
        Error::at(
            Kind::State,
            self.src,
            start,
            &format!("Expecting {expected}, got '{}'", TOKEN_NAMES[t as usize]),
            false,
        )
    }

    fn expect(&mut self, kind: u8) -> Result<String, Error> {
        if self.types[self.i] != kind {
            return Err(self.fail(&format!("'{}'", TOKEN_NAMES[kind as usize])));
        }
        self.i += 1;
        Ok(self.texts[self.i - 1].clone())
    }

    fn id_statement(&mut self) -> Result<StateStmt, Error> {
        let t = self.types[self.i];
        if t != ID && t != EDGE_STATE {
            return Err(self.fail("'ID', 'EDGE_STATE'"));
        }
        let id = js::trim(&self.texts[self.i]).to_string();
        self.i += 1;
        let mut state = StateStmt::new(id, Some("default"));
        state.description = Some(Descr::One(String::new()));
        if self.types[self.i] != STYLE_SEPARATOR {
            return Ok(state);
        }
        self.i += 1;
        state.classes = Some(vec![js::trim(&self.expect(ID)?).to_string()]);
        Ok(state)
    }
}

/// Where a composite's statements go: the open composites, outermost first,
/// each with the document it sits in.
struct Open {
    parent: Vec<Stmt>,
    state: StateStmt,
}

pub fn parse_state(src: &str, db: &mut StateModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut ps = Parser {
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        error: tokens.error,
        i: 0,
    };

    while ps.types[ps.i] == NL {
        ps.i += 1;
    }
    if ps.types[ps.i] != SD {
        return Err(ps.fail("'NL', 'SD'"));
    }
    ps.i += 1;

    let mut doc: Vec<Stmt> = Vec::new();
    let mut outer: Vec<Open> = Vec::new();
    // The grammar drops a statement whose value is the string it uses for a
    // blank line.
    let add = |doc: &mut Vec<Stmt>, value: Stmt| {
        if !matches!(&value, Stmt::Text(text) if text == "nl") {
            doc.push(value);
        }
    };
    let open = |ps: &mut Parser,
                doc: &mut Vec<Stmt>,
                outer: &mut Vec<Open>,
                id: String,
                description: String| {
        ps.i += 1;
        let mut state = StateStmt::new(id, Some("default"));
        state.description = Some(Descr::One(description));
        outer.push(Open {
            parent: std::mem::take(doc),
            state,
        });
    };

    loop {
        let t = ps.types[ps.i];
        match t {
            NL => ps.i += 1,
            ID | EDGE_STATE => {
                let mut first = ps.id_statement()?;
                if ps.types[ps.i] == DESCR {
                    first.description = Some(Descr::One(db.trim_colon(&ps.texts[ps.i])));
                    ps.i += 1;
                    add(&mut doc, Stmt::State(Box::new(first)));
                } else if ps.types[ps.i] == ARROW {
                    ps.i += 1;
                    let second = ps.id_statement()?;
                    let description = if ps.types[ps.i] == DESCR {
                        ps.i += 1;
                        Some(db.trim_colon(&ps.texts[ps.i - 1]))
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
            HIDE_EMPTY => {
                add(&mut doc, Stmt::Text(ps.texts[ps.i].clone()));
                ps.i += 1;
            }
            SCALE => {
                ps.i += 1;
                ps.expect(WIDTH)?;
                add(&mut doc, Stmt::Text(ps.texts[ps.i - 2].clone()));
            }
            COMPOSIT_STATE => {
                let text = ps.texts[ps.i].clone();
                ps.i += 1;
                if ps.types[ps.i] == STRUCT_START {
                    open(&mut ps, &mut doc, &mut outer, text, String::new());
                } else {
                    add(&mut doc, Stmt::Text(text));
                }
            }
            STATE_DESCR => {
                let description = ps.texts[ps.i].clone();
                ps.i += 1;
                ps.expect(AS)?;
                let id = ps.expect(ID)?;
                if ps.types[ps.i] == STRUCT_START {
                    open(&mut ps, &mut doc, &mut outer, id, description);
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
            FORK | JOIN | CHOICE => {
                let kind = match t {
                    FORK => "fork",
                    JOIN => "join",
                    _ => "choice",
                };
                add(
                    &mut doc,
                    Stmt::State(Box::new(StateStmt::new(ps.texts[ps.i].clone(), Some(kind)))),
                );
                ps.i += 1;
            }
            CONCURRENT => {
                ps.i += 1;
                add(
                    &mut doc,
                    Stmt::State(Box::new(StateStmt::new(
                        db.get_divider_id(),
                        Some("divider"),
                    ))),
                );
            }
            NOTE => {
                let keyword = ps.texts[ps.i].clone();
                ps.i += 1;
                if ps.types[ps.i] == LEFT_OF || ps.types[ps.i] == RIGHT_OF {
                    let position = js::trim(&ps.texts[ps.i]).to_string();
                    ps.i += 1;
                    let id = js::trim(&ps.expect(ID)?).to_string();
                    let text = js::trim(&ps.expect(NOTE_TEXT)?).to_string();
                    let mut state = StateStmt::new(id, None);
                    state.note = Some(StateNote {
                        position: Some(position),
                        text,
                    });
                    add(&mut doc, Stmt::State(Box::new(state)));
                } else {
                    if ps.types[ps.i] != NOTE_TEXT {
                        return Err(ps.fail("'NOTE_TEXT', 'left_of', 'right_of'"));
                    }
                    ps.i += 1;
                    ps.expect(AS)?;
                    ps.expect(ID)?;
                    add(&mut doc, Stmt::Text(keyword));
                }
            }
            DIRECTION_TB | DIRECTION_BT | DIRECTION_RL | DIRECTION_LR => {
                let value = DIRECTIONS[(t - DIRECTION_TB) as usize].to_string();
                ps.i += 1;
                // The grammar's setDirection lands in the root document the
                // parser has not handed over yet, which setRootDoc replaces.
                add(&mut doc, Stmt::Dir(DirectionStmt { value }));
            }
            ACC_TITLE => {
                ps.i += 1;
                let value = js::trim(&ps.expect(ACC_TITLE_VALUE)?).to_string();
                db.set_acc_title(&value);
                add(&mut doc, Stmt::Text(value));
            }
            ACC_DESCR => {
                ps.i += 1;
                let value = js::trim(&ps.expect(ACC_DESCR_VALUE)?).to_string();
                db.set_acc_description(&value);
                add(&mut doc, Stmt::Text(value));
            }
            ACC_DESCR_MULTILINE_VALUE => {
                let value = js::trim(&ps.texts[ps.i]).to_string();
                ps.i += 1;
                db.set_acc_description(&value);
                add(&mut doc, Stmt::Text(value));
            }
            CLICK => {
                ps.i += 1;
                let id = ps.id_statement()?;
                let url;
                let mut tooltip = String::new();
                if ps.types[ps.i] == HREF {
                    ps.i += 1;
                    url = ps.expect(STRING)?;
                } else {
                    if ps.types[ps.i] != STRING {
                        return Err(ps.fail("'STRING', 'HREF'"));
                    }
                    url = ps.texts[ps.i].clone();
                    ps.i += 1;
                    tooltip = ps.expect(STRING)?;
                }
                ps.expect(NL)?;
                add(
                    &mut doc,
                    Stmt::Click(Box::new(ClickStmt { id, url, tooltip })),
                );
            }
            CLASSDEF => {
                ps.i += 1;
                let id = js::trim(&ps.expect(CLASSDEF_ID)?).to_string();
                let classes = js::trim(&ps.expect(CLASSDEF_STYLEOPTS)?).to_string();
                add(&mut doc, Stmt::ClassDef(ClassDefStmt { id, classes }));
            }
            STYLE => {
                ps.i += 1;
                let id = js::trim(&ps.expect(STYLE_IDS)?).to_string();
                let style_class = js::trim(&ps.expect(STYLEDEF_STYLEOPTS)?).to_string();
                add(&mut doc, Stmt::Style(StyleStmt { id, style_class }));
            }
            CLASS => {
                ps.i += 1;
                let id = js::trim(&ps.expect(CLASSENTITY_IDS)?).to_string();
                let style_class = js::trim(&ps.expect(STYLECLASS)?).to_string();
                add(&mut doc, Stmt::ApplyClass(StyleStmt { id, style_class }));
            }
            STRUCT_STOP => {
                let Some(Open { parent, mut state }) = outer.pop() else {
                    return Err(ps.fail(&format!("'$end', {STATEMENT}")));
                };
                ps.i += 1;
                state.doc = Some(std::mem::replace(&mut doc, parent));
                doc.push(Stmt::State(Box::new(state)));
            }
            END => {
                if !outer.is_empty() {
                    return Err(ps.fail(&format!("'STRUCT_STOP', {STATEMENT}")));
                }
                db.set_root_doc(doc)?;
                return Ok(());
            }
            _ => {
                return Err(ps.fail(&if outer.is_empty() {
                    format!("'$end', {STATEMENT}")
                } else {
                    format!("'STRUCT_STOP', {STATEMENT}")
                }));
            }
        }
    }
}
