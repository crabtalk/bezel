//! Pele's `diagrams/sequence/parser.ts`.
//!
//! Mermaid's grammar nests the statements of a block in lists the model
//! builder walks in order. The statements here are kept in that order in one
//! list, each block's start entry in a slot reserved where the block opens.

use super::{
    db::SequenceModel,
    lexer::tokenize,
    tokens::*,
    types::{NoteActor, Participant, SeqStatement, SeqText, line, placement},
};
use crate::{detect::Kind, error::Error, js, util::chars::width_at};

const SIGNAL_TOKENS: &str = "'SOLID_OPEN_ARROW', 'DOTTED_OPEN_ARROW', 'SOLID_ARROW', 'DOTTED_ARROW', 'SOLID_CROSS', 'DOTTED_CROSS', 'SOLID_POINT', 'DOTTED_POINT'";

fn signal_of(token: u8) -> Option<i32> {
    Some(match token {
        SOLID_OPEN_ARROW => line::SOLID_OPEN,
        DOTTED_OPEN_ARROW => line::DOTTED_OPEN,
        SOLID_ARROW => line::SOLID,
        DOTTED_ARROW => line::DOTTED,
        BIDIRECTIONAL_SOLID_ARROW => line::BIDIRECTIONAL_SOLID,
        BIDIRECTIONAL_DOTTED_ARROW => line::BIDIRECTIONAL_DOTTED,
        SOLID_CROSS => line::SOLID_CROSS,
        DOTTED_CROSS => line::DOTTED_CROSS,
        SOLID_POINT => line::SOLID_POINT,
        DOTTED_POINT => line::DOTTED_POINT,
        SOLID_ARROW_TOP => line::SOLID_TOP,
        SOLID_ARROW_BOTTOM => line::SOLID_BOTTOM,
        STICK_ARROW_TOP => line::STICK_TOP,
        STICK_ARROW_BOTTOM => line::STICK_BOTTOM,
        SOLID_ARROW_TOP_DOTTED => line::SOLID_TOP_DOTTED,
        SOLID_ARROW_BOTTOM_DOTTED => line::SOLID_BOTTOM_DOTTED,
        STICK_ARROW_TOP_DOTTED => line::STICK_TOP_DOTTED,
        STICK_ARROW_BOTTOM_DOTTED => line::STICK_BOTTOM_DOTTED,
        SOLID_ARROW_TOP_REVERSE => line::SOLID_ARROW_TOP_REVERSE,
        SOLID_ARROW_BOTTOM_REVERSE => line::SOLID_ARROW_BOTTOM_REVERSE,
        STICK_ARROW_TOP_REVERSE => line::STICK_ARROW_TOP_REVERSE,
        STICK_ARROW_BOTTOM_REVERSE => line::STICK_ARROW_BOTTOM_REVERSE,
        SOLID_ARROW_TOP_REVERSE_DOTTED => line::SOLID_ARROW_TOP_REVERSE_DOTTED,
        SOLID_ARROW_BOTTOM_REVERSE_DOTTED => line::SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
        STICK_ARROW_TOP_REVERSE_DOTTED => line::STICK_ARROW_TOP_REVERSE_DOTTED,
        STICK_ARROW_BOTTOM_REVERSE_DOTTED => line::STICK_ARROW_BOTTOM_REVERSE_DOTTED,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum K {
    Top,
    Box,
    Loop,
    Rect,
    Opt,
    Break,
    Alt,
    Par,
    Critical,
}

fn block_of(token: u8) -> Option<K> {
    Some(match token {
        LOOP => K::Loop,
        RECT => K::Rect,
        OPT => K::Opt,
        BREAK => K::Break,
        ALT => K::Alt,
        PAR | PAR_OVER => K::Par,
        CRITICAL => K::Critical,
        BOX => K::Box,
        _ => return None,
    })
}

struct Frame {
    kind: K,
    text: String,
    start_type: i32,
    /// The slot of the block's start entry.
    slot: usize,
}

struct Parser<'s, 'd> {
    src: &'s str,
    types: Vec<u8>,
    texts: Vec<String>,
    starts: Vec<usize>,
    i: usize,
    db: &'d mut SequenceModel,
    list: Vec<Option<SeqStatement>>,
}

impl Parser<'_, '_> {
    fn fail(&self, expected: &str) -> Error {
        let t = self.types[self.i];
        if t == ERROR {
            return Error::at(Kind::Sequence, self.src, self.starts[self.i], "", true);
        }
        Error::at(
            Kind::Sequence,
            self.src,
            self.starts[self.i],
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

    fn actor(&mut self) -> Result<String, Error> {
        let t = self.types[self.i];
        if t != ACTOR && t != LINK {
            return Err(self.fail("'ACTOR'"));
        }
        self.i += 1;
        Ok(self.texts[self.i - 1].clone())
    }

    fn mention(actor: &str) -> SeqStatement {
        SeqStatement::AddParticipant(Participant {
            actor: actor.to_string(),
            draw: None,
            description: None,
            config: None,
        })
    }

    fn text2(&mut self) -> Result<SeqText, Error> {
        let text = self.expect(TXT)?;
        let text = js::trim(&text);
        Ok(self.db.parse_message(&text[1.min(text.len())..]))
    }

    /// A `participant`, `actor` or `destroy` statement: the participant, and
    /// whether it was `destroy`.
    fn participant(&mut self) -> Result<(Participant, bool), Error> {
        let t = self.types[self.i];
        self.i += 1;
        if t == DESTROY {
            let actor = self.actor()?;
            self.expect(NEWLINE)?;
            return Ok((
                Participant {
                    actor,
                    draw: None,
                    description: None,
                    config: None,
                },
                true,
            ));
        }
        let actor = self.actor()?;
        let mut node = Participant {
            actor,
            draw: None,
            description: None,
            config: None,
        };
        if self.types[self.i] == CONFIG_START && self.types[self.i - 1] == ACTOR {
            self.i += 1;
            let content = self.expect(CONFIG_CONTENT)?;
            self.expect(CONFIG_END)?;
            node.config = Some(js::trim(&content).to_string());
        }
        node.draw = Some(if t == PARTICIPANT {
            "participant"
        } else {
            "actor"
        });
        if self.types[self.i] == AS {
            self.i += 1;
            let alias = self.expect(REST_OF_LINE)?;
            self.expect(NEWLINE)?;
            node.description = Some(self.db.parse_message(&alias));
        } else if self.types[self.i] == NEWLINE {
            self.i += 1;
        } else {
            return Err(self.fail("'AS', 'NEWLINE'"));
        }
        Ok((node, false))
    }

    fn push_participant(&mut self) -> Result<(), Error> {
        let (node, destroyed) = self.participant()?;
        self.list.push(Some(if destroyed {
            SeqStatement::DestroyParticipant(node.actor)
        } else {
            SeqStatement::AddParticipant(node)
        }));
        Ok(())
    }

    fn signal(&mut self, from: String) -> Result<(), Error> {
        let mut central_from = false;
        if self.types[self.i] == CENTRAL {
            central_from = true;
            self.i += 1;
        }
        let Some(signal_type) = signal_of(self.types[self.i]) else {
            return Err(self.fail(SIGNAL_TOKENS));
        };
        self.i += 1;
        let mark = self.types[self.i];
        if mark == CENTRAL || (!central_from && (mark == PLUS || mark == MINUS)) {
            self.i += 1;
        }
        let to = self.actor()?;
        let msg = self.text2()?;
        self.list.push(Some(Self::mention(&from)));
        self.list.push(Some(Self::mention(&to)));
        let mut activate = None;
        let mut central_connection = None;
        let mut marks = Vec::new();
        if central_from {
            let dual = mark == CENTRAL;
            activate = Some(dual);
            central_connection = Some(if dual {
                line::CENTRAL_CONNECTION_DUAL
            } else {
                line::CENTRAL_CONNECTION_REVERSE
            });
            if dual {
                marks.push((to.clone(), line::CENTRAL_CONNECTION));
            }
            marks.push((from.clone(), line::CENTRAL_CONNECTION_REVERSE));
        } else if mark == PLUS {
            activate = Some(true);
            marks.push((to.clone(), line::ACTIVE_START));
        } else if mark == MINUS {
            marks.push((from.clone(), line::ACTIVE_END));
        } else if mark == CENTRAL {
            activate = Some(true);
            central_connection = Some(line::CENTRAL_CONNECTION);
            marks.push((to.clone(), line::CENTRAL_CONNECTION));
        }
        self.list.push(Some(SeqStatement::AddMessage {
            from,
            to,
            signal_type,
            msg,
            activate,
            central_connection,
        }));
        for (actor, signal_type) in marks {
            self.list
                .push(Some(SeqStatement::Mark { actor, signal_type }));
        }
        Ok(())
    }

    fn menu(&mut self, make: fn(String, SeqText) -> SeqStatement) -> Result<(), Error> {
        self.i += 1;
        let who = self.actor()?;
        let text = self.text2()?;
        self.list.push(Some(Self::mention(&who)));
        self.list.push(Some(make(who, text)));
        Ok(())
    }

    fn note(&mut self) -> Result<(), Error> {
        self.i += 1;
        let t = self.types[self.i];
        if t == OVER {
            self.i += 1;
            let first = self.actor()?;
            let mut second = None;
            if self.types[self.i] == COMMA {
                self.i += 1;
                second = Some(self.actor()?);
            }
            let text = self.text2()?;
            self.list.push(Some(Self::mention(&first)));
            if let Some(second) = &second {
                self.list.push(Some(Self::mention(second)));
            }
            let second = second.unwrap_or_else(|| first.clone());
            self.list.push(Some(SeqStatement::AddNote {
                actor: NoteActor::Two([first, second]),
                placement: placement::OVER,
                text,
            }));
            return Ok(());
        }
        if t != LEFT_OF && t != RIGHT_OF {
            return Err(self.fail("'over', 'left_of', 'right_of'"));
        }
        self.i += 1;
        let who = self.actor()?;
        let text = self.text2()?;
        self.list.push(Some(Self::mention(&who)));
        self.list.push(Some(SeqStatement::AddNote {
            actor: NoteActor::One(who),
            placement: if t == LEFT_OF {
                placement::LEFTOF
            } else {
                placement::RIGHTOF
            },
            text,
        }));
        Ok(())
    }

    fn autonumber(&mut self) -> Result<(), Error> {
        self.i += 1;
        let mut index = None;
        let mut step = None;
        let mut visible = true;
        if self.types[self.i] == NUM {
            index = Some(js::number(&self.texts[self.i]));
            self.i += 1;
            step = Some(if self.types[self.i] == NUM {
                self.i += 1;
                js::number(&self.texts[self.i - 1])
            } else {
                1.0
            });
        } else if self.types[self.i] == OFF {
            self.i += 1;
            visible = false;
        }
        if self.types[self.i] != NEWLINE {
            return Err(self.fail("'NEWLINE', 'NUM', 'off'"));
        }
        self.i += 1;
        self.list.push(Some(SeqStatement::SequenceIndex {
            index,
            step,
            visible,
        }));
        Ok(())
    }

    fn close(&mut self, frame: Frame) {
        let text = frame.text;
        let (start, end) = match frame.kind {
            K::Box => (
                SeqStatement::BoxStart(self.db.parse_box_data(&text)),
                SeqStatement::BoxEnd,
            ),
            K::Loop => (
                section(self.db.parse_message(&text), line::LOOP_START),
                ending(line::LOOP_END),
            ),
            K::Rect => (
                section(self.db.parse_message(&text), line::RECT_START),
                ending(line::RECT_END),
            ),
            K::Opt => (
                section(self.db.parse_message(&text), line::OPT_START),
                ending(line::OPT_END),
            ),
            K::Break => (
                section(self.db.parse_message(&text), line::BREAK_START),
                ending(line::BREAK_END),
            ),
            K::Alt => (
                section(self.db.parse_message(&text), line::ALT_START),
                ending(line::ALT_END),
            ),
            K::Par => (
                section(self.db.parse_message(&text), frame.start_type),
                ending(line::PAR_END),
            ),
            K::Critical | K::Top => (
                section(self.db.parse_message(&text), line::CRITICAL_START),
                ending(line::CRITICAL_END),
            ),
        };
        self.list[frame.slot] = Some(start);
        self.list.push(Some(end));
    }

    fn run(&mut self) -> Result<(), Error> {
        while self.types[self.i] == NEWLINE {
            self.i += 1;
        }
        self.expect(SD)?;

        let mut stack: Vec<Frame> = Vec::new();
        let mut frame = Frame {
            kind: K::Top,
            text: String::new(),
            start_type: 0,
            slot: 0,
        };

        loop {
            let t = self.types[self.i];
            if frame.kind == K::Box {
                if t == PARTICIPANT || t == PARTICIPANT_ACTOR || t == DESTROY {
                    self.push_participant()?;
                } else if t == NEWLINE {
                    self.i += 1;
                } else if t == END {
                    self.i += 1;
                    let parent = stack.pop().expect("a box is inside a frame");
                    self.close(std::mem::replace(&mut frame, parent));
                } else {
                    return Err(self
                        .fail("'end', 'NEWLINE', 'participant', 'participant_actor', 'destroy'"));
                }
                continue;
            }
            match t {
                NEWLINE | INVALID => self.i += 1,
                PARTICIPANT | PARTICIPANT_ACTOR | DESTROY => self.push_participant()?,
                CREATE => {
                    self.i += 1;
                    let next = self.types[self.i];
                    if next != PARTICIPANT && next != PARTICIPANT_ACTOR && next != DESTROY {
                        return Err(self.fail("'participant', 'participant_actor', 'destroy'"));
                    }
                    let (node, _) = self.participant()?;
                    self.list.push(Some(SeqStatement::CreateParticipant(node)));
                }
                ACTOR => {
                    let from = self.actor()?;
                    self.signal(from)?;
                    self.expect(NEWLINE)?;
                }
                LINK => {
                    let next = self.types[self.i + 1];
                    if next == ACTOR || next == LINK {
                        self.menu(SeqStatement::AddALink)?;
                    } else {
                        let from = self.actor()?;
                        self.signal(from)?;
                    }
                    self.expect(NEWLINE)?;
                }
                LINKS => {
                    self.menu(SeqStatement::AddLinks)?;
                    self.expect(NEWLINE)?;
                }
                PROPERTIES => {
                    self.menu(SeqStatement::AddProperties)?;
                    self.expect(NEWLINE)?;
                }
                DETAILS => {
                    self.menu(SeqStatement::AddDetails)?;
                    self.expect(NEWLINE)?;
                }
                NOTE => {
                    self.note()?;
                    self.expect(NEWLINE)?;
                }
                AUTONUMBER => self.autonumber()?,
                ACTIVATE | DEACTIVATE => {
                    self.i += 1;
                    let actor = self.actor()?;
                    self.expect(NEWLINE)?;
                    self.list.push(Some(SeqStatement::Mark {
                        actor,
                        signal_type: if t == ACTIVATE {
                            line::ACTIVE_START
                        } else {
                            line::ACTIVE_END
                        },
                    }));
                }
                TITLE | LEGACY_TITLE => {
                    let text = &self.texts[self.i];
                    self.i += 1;
                    let head = if t == TITLE { 5 } else { 6 };
                    let title = text[head + width_at(text, head)..].to_string();
                    self.db.set_diagram_title(&title);
                }
                ACC_TITLE => {
                    self.i += 1;
                    let value = self.expect(ACC_TITLE_VALUE)?;
                    self.db.set_acc_title(js::trim(&value));
                }
                ACC_DESCR => {
                    self.i += 1;
                    let value = self.expect(ACC_DESCR_VALUE)?;
                    self.db.set_acc_description(js::trim(&value));
                }
                ACC_DESCR_MULTILINE_VALUE => {
                    let value = js::trim(&self.texts[self.i]).to_string();
                    self.i += 1;
                    self.db.set_acc_description(&value);
                }
                LOOP | RECT | OPT | BREAK | ALT | PAR | PAR_OVER | CRITICAL | BOX => {
                    self.i += 1;
                    let text = self.expect(REST_OF_LINE)?;
                    let slot = self.list.len();
                    self.list.push(None);
                    let inner = Frame {
                        kind: block_of(t).unwrap_or(K::Top),
                        text,
                        start_type: if t == PAR_OVER {
                            line::PAR_OVER_START
                        } else {
                            line::PAR_START
                        },
                        slot,
                    };
                    stack.push(std::mem::replace(&mut frame, inner));
                }
                ELSE | AND | OPTION => {
                    let (expected, signal_type) = match t {
                        ELSE => (K::Alt, line::ALT_ELSE),
                        AND => (K::Par, line::PAR_AND),
                        _ => (K::Critical, line::CRITICAL_OPTION),
                    };
                    if frame.kind != expected {
                        return Err(self.fail("'end'"));
                    }
                    self.i += 1;
                    let text = self.expect(REST_OF_LINE)?;
                    let text = self.db.parse_message(&text);
                    self.list.push(Some(section(text, signal_type)));
                }
                END => {
                    if frame.kind == K::Top {
                        return Err(self.fail("'NEWLINE', 'EOF'"));
                    }
                    self.i += 1;
                    let parent = stack.pop().expect("a block is inside a frame");
                    self.close(std::mem::replace(&mut frame, parent));
                }
                EOF => {
                    if frame.kind != K::Top {
                        return Err(self.fail("'end'"));
                    }
                    return Ok(());
                }
                _ => return Err(self.fail("'NEWLINE', 'ACTOR', 'participant', 'note', 'end'")),
            }
        }
    }
}

fn section(text: SeqText, signal_type: i32) -> SeqStatement {
    SeqStatement::Section { text, signal_type }
}

fn ending(signal_type: i32) -> SeqStatement {
    SeqStatement::End { signal_type }
}

pub fn parse_sequence(src: &str, db: &mut SequenceModel) -> Result<(), Error> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        src,
        types: tokens.types,
        texts: tokens.texts,
        starts: tokens.starts,
        i: 0,
        db,
        list: Vec::new(),
    };
    parser.run()?;
    let list = std::mem::take(&mut parser.list);
    parser.db.apply(list.into_iter().flatten())
}
