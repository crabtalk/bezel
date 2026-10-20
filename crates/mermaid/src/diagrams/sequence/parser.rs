//! Pele's `diagrams/sequence/parser.ts`.
//!
//! Mermaid's grammar nests the statements of a block in lists the model
//! builder walks in order. The statements here are kept in that order in one
//! list, each block's start entry in a slot reserved where the block opens.

use super::{
    db::SequenceModel,
    lexer::tokenize,
    tokens::T,
    types::{NoteActor, Participant, Placement, SeqBoxData, SeqStatement, SeqText, line},
};
use crate::{
    detect::Kind,
    error::Error,
    js,
    token::{Stream, after_keyword},
};

const SIGNAL_TOKENS: &str = "'SOLID_OPEN_ARROW', 'DOTTED_OPEN_ARROW', 'SOLID_ARROW', 'DOTTED_ARROW', 'SOLID_CROSS', 'DOTTED_CROSS', 'SOLID_POINT', 'DOTTED_POINT'";

impl T {
    /// The message kind of an arrow.
    fn signal(self) -> Option<i32> {
        Some(match self {
            T::SolidOpenArrow => line::SOLID_OPEN,
            T::DottedOpenArrow => line::DOTTED_OPEN,
            T::SolidArrow => line::SOLID,
            T::DottedArrow => line::DOTTED,
            T::BidirectionalSolidArrow => line::BIDIRECTIONAL_SOLID,
            T::BidirectionalDottedArrow => line::BIDIRECTIONAL_DOTTED,
            T::SolidCross => line::SOLID_CROSS,
            T::DottedCross => line::DOTTED_CROSS,
            T::SolidPoint => line::SOLID_POINT,
            T::DottedPoint => line::DOTTED_POINT,
            T::SolidArrowTop => line::SOLID_TOP,
            T::SolidArrowBottom => line::SOLID_BOTTOM,
            T::StickArrowTop => line::STICK_TOP,
            T::StickArrowBottom => line::STICK_BOTTOM,
            T::SolidArrowTopDotted => line::SOLID_TOP_DOTTED,
            T::SolidArrowBottomDotted => line::SOLID_BOTTOM_DOTTED,
            T::StickArrowTopDotted => line::STICK_TOP_DOTTED,
            T::StickArrowBottomDotted => line::STICK_BOTTOM_DOTTED,
            T::SolidArrowTopReverse => line::SOLID_ARROW_TOP_REVERSE,
            T::SolidArrowBottomReverse => line::SOLID_ARROW_BOTTOM_REVERSE,
            T::StickArrowTopReverse => line::STICK_ARROW_TOP_REVERSE,
            T::StickArrowBottomReverse => line::STICK_ARROW_BOTTOM_REVERSE,
            T::SolidArrowTopReverseDotted => line::SOLID_ARROW_TOP_REVERSE_DOTTED,
            T::SolidArrowBottomReverseDotted => line::SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
            T::StickArrowTopReverseDotted => line::STICK_ARROW_TOP_REVERSE_DOTTED,
            T::StickArrowBottomReverseDotted => line::STICK_ARROW_BOTTOM_REVERSE_DOTTED,
            _ => return None,
        })
    }

    /// The block a keyword opens.
    fn block(self) -> Option<K> {
        Some(match self {
            T::Loop => K::Loop,
            T::Rect => K::Rect,
            T::Opt => K::Opt,
            T::Break => K::Break,
            T::Alt => K::Alt,
            T::Par | T::ParOver => K::Par,
            T::Critical => K::Critical,
            T::Box => K::Box,
            _ => return None,
        })
    }
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

struct Frame {
    kind: K,
    text: String,
    start_type: i32,
    /// The slot of the block's start entry.
    slot: usize,
}

struct Parser<'t, 's, 'd> {
    s: Stream<'t, 's, T>,
    db: &'d mut SequenceModel,
    list: Vec<Option<SeqStatement>>,
}

impl Parser<'_, '_, '_> {
    fn actor(&mut self) -> Result<String, Error> {
        if !self.s.is_any(&[T::Actor, T::Link]) {
            return Err(self.s.fail("'ACTOR'"));
        }
        Ok(self.s.take().to_string())
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
        let text = js::trim(self.s.expect(T::Txt, "")?);
        Ok(SeqText::parse(&text[1.min(text.len())..]))
    }

    /// A `participant`, `actor` or `destroy` statement: the participant, and
    /// whether it was `destroy`.
    fn participant(&mut self) -> Result<(Participant, bool), Error> {
        let t = self.s.kind();
        self.s.bump();
        if t == T::Destroy {
            let actor = self.actor()?;
            self.s.expect(T::Newline, "")?;
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
        if self.s.is(T::ConfigStart) && self.s.previous() == Some(T::Actor) {
            self.s.bump();
            let content = self.s.expect(T::ConfigContent, "")?;
            self.s.expect(T::ConfigEnd, "")?;
            node.config = Some(js::trim(content).to_string());
        }
        node.draw = Some(if t == T::Participant {
            "participant"
        } else {
            "actor"
        });
        if self.s.eat(T::As) {
            let alias = self.s.expect(T::RestOfLine, "")?;
            self.s.expect(T::Newline, "")?;
            node.description = Some(SeqText::parse(alias));
        } else if self.s.eat(T::Newline) {
        } else {
            return Err(self.s.fail("'AS', 'NEWLINE'"));
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
        let central_from = self.s.eat(T::Central);
        let Some(signal_type) = self.s.kind().signal() else {
            return Err(self.s.fail(SIGNAL_TOKENS));
        };
        self.s.bump();
        let mark = self.s.kind();
        if mark == T::Central || (!central_from && (mark == T::Plus || mark == T::Minus)) {
            self.s.bump();
        }
        let to = self.actor()?;
        let msg = self.text2()?;
        self.list.push(Some(Self::mention(&from)));
        self.list.push(Some(Self::mention(&to)));
        let mut activate = None;
        let mut central_connection = None;
        let mut marks = Vec::new();
        if central_from {
            let dual = mark == T::Central;
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
        } else if mark == T::Plus {
            activate = Some(true);
            marks.push((to.clone(), line::ACTIVE_START));
        } else if mark == T::Minus {
            marks.push((from.clone(), line::ACTIVE_END));
        } else if mark == T::Central {
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
        self.s.bump();
        let who = self.actor()?;
        let text = self.text2()?;
        self.list.push(Some(Self::mention(&who)));
        self.list.push(Some(make(who, text)));
        Ok(())
    }

    fn note(&mut self) -> Result<(), Error> {
        self.s.bump();
        let t = self.s.kind();
        if t == T::Over {
            self.s.bump();
            let first = self.actor()?;
            let mut second = None;
            if self.s.eat(T::Comma) {
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
                placement: Placement::Over,
                text,
            }));
            return Ok(());
        }
        if t != T::LeftOf && t != T::RightOf {
            return Err(self.s.fail("'over', 'left_of', 'right_of'"));
        }
        self.s.bump();
        let who = self.actor()?;
        let text = self.text2()?;
        self.list.push(Some(Self::mention(&who)));
        self.list.push(Some(SeqStatement::AddNote {
            actor: NoteActor::One(who),
            placement: if t == T::LeftOf {
                Placement::LeftOf
            } else {
                Placement::RightOf
            },
            text,
        }));
        Ok(())
    }

    fn autonumber(&mut self) -> Result<(), Error> {
        self.s.bump();
        let mut index = None;
        let mut step = None;
        let mut visible = true;
        if self.s.is(T::Num) {
            index = Some(js::number(self.s.take()));
            step = Some(if self.s.is(T::Num) {
                js::number(self.s.take())
            } else {
                1.0
            });
        } else if self.s.eat(T::Off) {
            visible = false;
        }
        if !self.s.eat(T::Newline) {
            return Err(self.s.fail("'NEWLINE', 'NUM', 'off'"));
        }
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
                SeqStatement::BoxStart(SeqBoxData::parse(&text)),
                SeqStatement::BoxEnd,
            ),
            K::Loop => (
                section(SeqText::parse(&text), line::LOOP_START),
                ending(line::LOOP_END),
            ),
            K::Rect => (
                section(SeqText::parse(&text), line::RECT_START),
                ending(line::RECT_END),
            ),
            K::Opt => (
                section(SeqText::parse(&text), line::OPT_START),
                ending(line::OPT_END),
            ),
            K::Break => (
                section(SeqText::parse(&text), line::BREAK_START),
                ending(line::BREAK_END),
            ),
            K::Alt => (
                section(SeqText::parse(&text), line::ALT_START),
                ending(line::ALT_END),
            ),
            K::Par => (
                section(SeqText::parse(&text), frame.start_type),
                ending(line::PAR_END),
            ),
            K::Critical | K::Top => (
                section(SeqText::parse(&text), line::CRITICAL_START),
                ending(line::CRITICAL_END),
            ),
        };
        self.list[frame.slot] = Some(start);
        self.list.push(Some(end));
    }

    fn run(&mut self) -> Result<(), Error> {
        self.s.eat_all(T::Newline);
        self.s.expect(T::Sd, "")?;

        let mut stack: Vec<Frame> = Vec::new();
        let mut frame = Frame {
            kind: K::Top,
            text: String::new(),
            start_type: 0,
            slot: 0,
        };

        loop {
            let t = self.s.kind();
            if frame.kind == K::Box {
                if t == T::Participant || t == T::ParticipantActor || t == T::Destroy {
                    self.push_participant()?;
                } else if t == T::Newline {
                    self.s.bump();
                } else if t == T::EndKw {
                    self.s.bump();
                    let parent = stack.pop().expect("a box is inside a frame");
                    self.close(std::mem::replace(&mut frame, parent));
                } else {
                    return Err(self
                        .s
                        .fail("'end', 'NEWLINE', 'participant', 'participant_actor', 'destroy'"));
                }
                continue;
            }
            match t {
                T::Newline | T::Invalid => self.s.bump(),
                T::Participant | T::ParticipantActor | T::Destroy => self.push_participant()?,
                T::Create => {
                    self.s.bump();
                    let next = self.s.kind();
                    if next != T::Participant && next != T::ParticipantActor && next != T::Destroy {
                        return Err(self.s.fail("'participant', 'participant_actor', 'destroy'"));
                    }
                    let (node, _) = self.participant()?;
                    self.list.push(Some(SeqStatement::CreateParticipant(node)));
                }
                T::Actor => {
                    let from = self.actor()?;
                    self.signal(from)?;
                    self.s.expect(T::Newline, "")?;
                }
                T::Link => {
                    let next = self.s.peek(1);
                    if next == T::Actor || next == T::Link {
                        self.menu(SeqStatement::AddALink)?;
                    } else {
                        let from = self.actor()?;
                        self.signal(from)?;
                    }
                    self.s.expect(T::Newline, "")?;
                }
                T::Links => {
                    self.menu(SeqStatement::AddLinks)?;
                    self.s.expect(T::Newline, "")?;
                }
                T::Properties => {
                    self.menu(SeqStatement::AddProperties)?;
                    self.s.expect(T::Newline, "")?;
                }
                T::Details => {
                    self.menu(SeqStatement::AddDetails)?;
                    self.s.expect(T::Newline, "")?;
                }
                T::Note => {
                    self.note()?;
                    self.s.expect(T::Newline, "")?;
                }
                T::Autonumber => self.autonumber()?,
                T::Activate | T::Deactivate => {
                    self.s.bump();
                    let actor = self.actor()?;
                    self.s.expect(T::Newline, "")?;
                    self.list.push(Some(SeqStatement::Mark {
                        actor,
                        signal_type: if t == T::Activate {
                            line::ACTIVE_START
                        } else {
                            line::ACTIVE_END
                        },
                    }));
                }
                T::Title | T::LegacyTitle => {
                    let text = self.s.take();
                    let head = if t == T::Title { 5 } else { 6 };
                    self.db.titles.set_title(after_keyword(text, head));
                }
                T::AccTitle => {
                    self.s.bump();
                    let value = self.s.expect(T::AccTitleValue, "")?;
                    self.db.titles.set_acc_title(js::trim(value));
                }
                T::AccDescr => {
                    self.s.bump();
                    let value = self.s.expect(T::AccDescrValue, "")?;
                    self.db.titles.set_acc_descr(js::trim(value));
                }
                T::AccDescrMultilineValue => {
                    let value = js::trim(self.s.take());
                    self.db.titles.set_acc_descr(value);
                }
                T::Loop
                | T::Rect
                | T::Opt
                | T::Break
                | T::Alt
                | T::Par
                | T::ParOver
                | T::Critical
                | T::Box => {
                    self.s.bump();
                    let text = self.s.expect(T::RestOfLine, "")?;
                    let slot = self.list.len();
                    self.list.push(None);
                    let inner = Frame {
                        kind: t.block().unwrap_or(K::Top),
                        text: text.to_string(),
                        start_type: if t == T::ParOver {
                            line::PAR_OVER_START
                        } else {
                            line::PAR_START
                        },
                        slot,
                    };
                    stack.push(std::mem::replace(&mut frame, inner));
                }
                T::Else | T::And | T::Option => {
                    let (expected, signal_type) = match t {
                        T::Else => (K::Alt, line::ALT_ELSE),
                        T::And => (K::Par, line::PAR_AND),
                        _ => (K::Critical, line::CRITICAL_OPTION),
                    };
                    if frame.kind != expected {
                        return Err(self.s.fail("'end'"));
                    }
                    self.s.bump();
                    let text = self.s.expect(T::RestOfLine, "")?;
                    let text = SeqText::parse(text);
                    self.list.push(Some(section(text, signal_type)));
                }
                T::EndKw => {
                    if frame.kind == K::Top {
                        return Err(self.s.fail("'NEWLINE', 'EOF'"));
                    }
                    self.s.bump();
                    let parent = stack.pop().expect("a block is inside a frame");
                    self.close(std::mem::replace(&mut frame, parent));
                }
                T::End => {
                    if frame.kind != K::Top {
                        return Err(self.s.fail("'end'"));
                    }
                    return Ok(());
                }
                _ => {
                    return Err(self
                        .s
                        .fail("'NEWLINE', 'ACTOR', 'participant', 'note', 'end'"));
                }
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
        s: Stream::new(Kind::Sequence, src, &tokens),
        db,
        list: Vec::new(),
    };
    parser.run()?;
    let list = std::mem::take(&mut parser.list);
    parser.db.apply(list.into_iter().flatten())
}
