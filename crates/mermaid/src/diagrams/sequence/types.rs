//! Pele's `diagrams/sequence/types.ts`.

use serde::{Serialize, Serializer, ser::SerializeMap};
use serde_repr::Serialize_repr;

use super::actors::ActorKind;
use crate::{config::Value, js};

/// A message's kind, numbered as in Mermaid. Besides lines it covers notes,
/// numbering, activations and the starts, sections and ends of blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum LineType {
    Solid = 0,
    Dotted = 1,
    Note = 2,
    SolidCross = 3,
    DottedCross = 4,
    SolidOpen = 5,
    DottedOpen = 6,
    LoopStart = 10,
    LoopEnd = 11,
    AltStart = 12,
    AltElse = 13,
    AltEnd = 14,
    OptStart = 15,
    OptEnd = 16,
    ActiveStart = 17,
    ActiveEnd = 18,
    ParStart = 19,
    ParAnd = 20,
    ParEnd = 21,
    RectStart = 22,
    RectEnd = 23,
    SolidPoint = 24,
    DottedPoint = 25,
    Autonumber = 26,
    CriticalStart = 27,
    CriticalOption = 28,
    CriticalEnd = 29,
    BreakStart = 30,
    BreakEnd = 31,
    ParOverStart = 32,
    BidirectionalSolid = 33,
    BidirectionalDotted = 34,
    SolidTop = 41,
    SolidBottom = 42,
    StickTop = 43,
    StickBottom = 44,
    SolidArrowTopReverse = 45,
    SolidArrowBottomReverse = 46,
    StickArrowTopReverse = 47,
    StickArrowBottomReverse = 48,
    SolidTopDotted = 51,
    SolidBottomDotted = 52,
    StickTopDotted = 53,
    StickBottomDotted = 54,
    SolidArrowTopReverseDotted = 55,
    SolidArrowBottomReverseDotted = 56,
    StickArrowTopReverseDotted = 57,
    StickArrowBottomReverseDotted = 58,
    CentralConnection = 59,
    CentralConnectionReverse = 60,
    CentralConnectionDual = 61,
}

impl LineType {
    /// The block a message of this kind opens.
    pub fn opens(self) -> Option<Block> {
        Some(match self {
            LineType::LoopStart => Block::Loop,
            LineType::AltStart => Block::Alt,
            LineType::OptStart => Block::Opt,
            LineType::ParStart | LineType::ParOverStart => Block::Par,
            LineType::CriticalStart => Block::Critical,
            LineType::BreakStart => Block::Break,
            LineType::RectStart => Block::Rect,
            _ => return None,
        })
    }

    /// Whether a message of this kind starts a further section of a block.
    pub fn divides(self) -> bool {
        matches!(
            self,
            LineType::AltElse | LineType::ParAnd | LineType::CriticalOption
        )
    }

    /// Whether a message of this kind ends a block.
    pub fn closes(self) -> bool {
        matches!(
            self,
            LineType::LoopEnd
                | LineType::AltEnd
                | LineType::OptEnd
                | LineType::ParEnd
                | LineType::CriticalEnd
                | LineType::BreakEnd
                | LineType::RectEnd
        )
    }
}

/// A block of messages, by its keyword.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    Rect,
}

impl Block {
    pub fn name(self) -> &'static str {
        match self {
            Block::Loop => "loop",
            Block::Alt => "alt",
            Block::Opt => "opt",
            Block::Par => "par",
            Block::Critical => "critical",
            Block::Break => "break",
            Block::Rect => "rect",
        }
    }

    /// The kind of the message that opens the block; `par over` opens a
    /// `par` with [`LineType::ParOverStart`] instead.
    pub fn start(self) -> LineType {
        match self {
            Block::Loop => LineType::LoopStart,
            Block::Alt => LineType::AltStart,
            Block::Opt => LineType::OptStart,
            Block::Par => LineType::ParStart,
            Block::Critical => LineType::CriticalStart,
            Block::Break => LineType::BreakStart,
            Block::Rect => LineType::RectStart,
        }
    }

    /// The kind of the message that ends the block.
    pub fn end(self) -> LineType {
        match self {
            Block::Loop => LineType::LoopEnd,
            Block::Alt => LineType::AltEnd,
            Block::Opt => LineType::OptEnd,
            Block::Par => LineType::ParEnd,
            Block::Critical => LineType::CriticalEnd,
            Block::Break => LineType::BreakEnd,
            Block::Rect => LineType::RectEnd,
        }
    }
}

/// Which ends of a message carry a central-connection circle, numbered as
/// the line types Mermaid gives them, 0 for neither.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum CentralConnection {
    #[default]
    Neither = 0,
    /// At the receiver.
    To = 59,
    /// At the sender.
    From = 60,
    Both = 61,
}

impl CentralConnection {
    pub fn at_to(self) -> bool {
        matches!(self, CentralConnection::To | CentralConnection::Both)
    }

    pub fn at_from(self) -> bool {
        matches!(self, CentralConnection::From | CentralConnection::Both)
    }
}

/// Where a note is put, numbered as in Mermaid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize_repr)]
#[repr(u8)]
pub enum Placement {
    LeftOf = 0,
    RightOf = 1,
    Over = 2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqText {
    pub text: String,
    pub wrap: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeqBoxData {
    pub text: Option<String>,
    pub color: String,
    pub wrap: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeqBox {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub wrap: bool,
    pub fill: String,
    pub actor_keys: Vec<String>,
}

/// A participant. `box_` is the index of its box in the model's `boxes`,
/// serialized as the box itself.
#[derive(Clone, Debug, PartialEq)]
pub struct SeqActor {
    pub box_: Option<usize>,
    pub name: String,
    pub description: String,
    pub wrap: bool,
    pub prev_actor: Option<String>,
    pub next_actor: Option<String>,
    pub links: indexmap::IndexMap<String, Value>,
    pub properties: indexmap::IndexMap<String, Value>,
    /// The id of the page element named by a `details` statement. Pele has
    /// no page to read it from.
    pub details: Option<String>,
    pub kind: String,
}

impl SeqActor {
    /// The actor as `JSON.stringify` writes it, its box looked up in `boxes`.
    pub(crate) fn serialize_in<S: Serializer>(
        &self,
        boxes: &[SeqBox],
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        if let Some(index) = self.box_ {
            map.serialize_entry("box", &boxes[index])?;
        }
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("description", &self.description)?;
        map.serialize_entry("wrap", &self.wrap)?;
        if let Some(prev) = &self.prev_actor {
            map.serialize_entry("prevActor", prev)?;
        }
        map.serialize_entry("links", &self.links)?;
        map.serialize_entry("properties", &self.properties)?;
        map.serialize_entry("type", &self.kind)?;
        if let Some(next) = &self.next_actor {
            map.serialize_entry("nextActor", next)?;
        }
        if let Some(details) = &self.details {
            map.serialize_entry("details", details)?;
        }
        map.end()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeqNumbering {
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub start: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "js::json_number_opt"
    )]
    pub step: Option<f64>,
    pub visible: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SeqMessageText {
    Text(String),
    Numbering(SeqNumbering),
}

impl SeqMessageText {
    /// The text, when the message has one.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Numbering(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeqMessage {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    pub message: SeqMessageText,
    pub wrap: bool,
    #[serde(rename = "type")]
    pub kind: LineType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub central_connection: Option<CentralConnection>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum NoteActor {
    One(String),
    Two([String; 2]),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SeqNote {
    pub actor: NoteActor,
    pub placement: Placement,
    pub message: String,
    pub wrap: bool,
}

/// One statement the parser hands to the model builder, as Mermaid's grammar
/// builds it.
#[derive(Clone, Debug, PartialEq)]
pub enum SeqStatement {
    SequenceIndex {
        index: Option<f64>,
        step: Option<f64>,
        visible: bool,
    },
    AddParticipant(Participant),
    CreateParticipant(Participant),
    DestroyParticipant(String),
    /// `activeStart`, `activeEnd`, `centralConnection` and
    /// `centralConnectionReverse`: a signal on one participant.
    Mark {
        actor: String,
        signal_type: LineType,
    },
    AddNote {
        actor: NoteActor,
        placement: Placement,
        text: SeqText,
    },
    AddLinks(String, SeqText),
    AddALink(String, SeqText),
    AddProperties(String, SeqText),
    AddDetails(String, SeqText),
    AddMessage {
        from: String,
        to: String,
        signal_type: LineType,
        msg: SeqText,
        activate: Option<bool>,
        central_connection: Option<CentralConnection>,
    },
    BoxStart(SeqBoxData),
    BoxEnd,
    /// The start of a block or of one of its sections, with its text.
    Section {
        text: SeqText,
        signal_type: LineType,
    },
    /// The end of a block.
    End {
        signal_type: LineType,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub actor: String,
    pub draw: Option<ActorKind>,
    pub description: Option<SeqText>,
    pub config: Option<String>,
}
