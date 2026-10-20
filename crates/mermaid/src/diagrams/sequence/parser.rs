//! Pele's `diagrams/sequence/parser.ts`.
//!
//! Mermaid's grammar nests the statements of a block in lists the model
//! builder walks in order. The statements here are kept in that order in one
//! list, each block's start entry in a slot reserved where the block opens.

use super::{
    actors::ActorKind,
    db::SequenceModel,
    lexer::tokenize,
    tokens::T,
    types::{
        Block, CentralConnection, LineType, NoteActor, Participant, Placement, SeqBoxData,
        SeqStatement, SeqText,
    },
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
    fn signal(self) -> Option<LineType> {
        Some(match self {
            T::SolidOpenArrow => LineType::SolidOpen,
            T::DottedOpenArrow => LineType::DottedOpen,
            T::SolidArrow => LineType::Solid,
            T::DottedArrow => LineType::Dotted,
            T::BidirectionalSolidArrow => LineType::BidirectionalSolid,
            T::BidirectionalDottedArrow => LineType::BidirectionalDotted,
            T::SolidCross => LineType::SolidCross,
            T::DottedCross => LineType::DottedCross,
            T::SolidPoint => LineType::SolidPoint,
            T::DottedPoint => LineType::DottedPoint,
            T::SolidArrowTop => LineType::SolidTop,
            T::SolidArrowBottom => LineType::SolidBottom,
            T::StickArrowTop => LineType::StickTop,
            T::StickArrowBottom => LineType::StickBottom,
            T::SolidArrowTopDotted => LineType::SolidTopDotted,
            T::SolidArrowBottomDotted => LineType::SolidBottomDotted,
            T::StickArrowTopDotted => LineType::StickTopDotted,
            T::StickArrowBottomDotted => LineType::StickBottomDotted,
            T::SolidArrowTopReverse => LineType::SolidArrowTopReverse,
            T::SolidArrowBottomReverse => LineType::SolidArrowBottomReverse,
            T::StickArrowTopReverse => LineType::StickArrowTopReverse,
            T::StickArrowBottomReverse => LineType::StickArrowBottomReverse,
            T::SolidArrowTopReverseDotted => LineType::SolidArrowTopReverseDotted,
            T::SolidArrowBottomReverseDotted => LineType::SolidArrowBottomReverseDotted,
            T::StickArrowTopReverseDotted => LineType::StickArrowTopReverseDotted,
            T::StickArrowBottomReverseDotted => LineType::StickArrowBottomReverseDotted,
            _ => return None,
        })
    }

    /// The block a keyword opens.
    fn block(self) -> Option<K> {
        Some(match self {
            T::Loop => K::Block(Block::Loop),
            T::Rect => K::Block(Block::Rect),
            T::Opt => K::Block(Block::Opt),
            T::Break => K::Block(Block::Break),
            T::Alt => K::Block(Block::Alt),
            T::Par | T::ParOver => K::Block(Block::Par),
            T::Critical => K::Block(Block::Critical),
            T::Box => K::Box,
            _ => return None,
        })
    }
}

/// What a frame of the parser is reading.
#[derive(Clone, Copy, PartialEq)]
enum K {
    Top,
    Box,
    Block(Block),
}

struct Frame {
    kind: K,
    text: String,
    /// The kind of a block's start entry.
    start_type: LineType,
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
            ActorKind::Participant
        } else {
            ActorKind::Actor
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
                CentralConnection::Both
            } else {
                CentralConnection::From
            });
            if dual {
                marks.push((to.clone(), LineType::CentralConnection));
            }
            marks.push((from.clone(), LineType::CentralConnectionReverse));
        } else if mark == T::Plus {
            activate = Some(true);
            marks.push((to.clone(), LineType::ActiveStart));
        } else if mark == T::Minus {
            marks.push((from.clone(), LineType::ActiveEnd));
        } else if mark == T::Central {
            activate = Some(true);
            central_connection = Some(CentralConnection::To);
            marks.push((to.clone(), LineType::CentralConnection));
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
            K::Block(block) => (
                section(SeqText::parse(&text), frame.start_type),
                ending(block.end()),
            ),
            K::Top => (
                section(SeqText::parse(&text), LineType::CriticalStart),
                ending(LineType::CriticalEnd),
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
            start_type: LineType::CriticalStart,
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
                            LineType::ActiveStart
                        } else {
                            LineType::ActiveEnd
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
                    let kind = t.block().unwrap_or(K::Top);
                    let start_type = match kind {
                        K::Block(Block::Par) if t == T::ParOver => LineType::ParOverStart,
                        K::Block(block) => block.start(),
                        K::Top | K::Box => LineType::CriticalStart,
                    };
                    let inner = Frame {
                        kind,
                        text: text.to_string(),
                        start_type,
                        slot,
                    };
                    stack.push(std::mem::replace(&mut frame, inner));
                }
                T::Else | T::And | T::Option => {
                    let (expected, signal_type) = match t {
                        T::Else => (Block::Alt, LineType::AltElse),
                        T::And => (Block::Par, LineType::ParAnd),
                        _ => (Block::Critical, LineType::CriticalOption),
                    };
                    let expected = K::Block(expected);
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

fn section(text: SeqText, signal_type: LineType) -> SeqStatement {
    SeqStatement::Section { text, signal_type }
}

fn ending(signal_type: LineType) -> SeqStatement {
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
