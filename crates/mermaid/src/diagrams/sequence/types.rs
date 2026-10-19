//! Pele's `diagrams/sequence/types.ts`.

use serde::{Serialize, Serializer, ser::SerializeMap};

use crate::{config::Value, js};

/// The message kinds, numbered as in Mermaid.
pub mod line {
    pub const SOLID: i32 = 0;
    pub const DOTTED: i32 = 1;
    pub const NOTE: i32 = 2;
    pub const SOLID_CROSS: i32 = 3;
    pub const DOTTED_CROSS: i32 = 4;
    pub const SOLID_OPEN: i32 = 5;
    pub const DOTTED_OPEN: i32 = 6;
    pub const LOOP_START: i32 = 10;
    pub const LOOP_END: i32 = 11;
    pub const ALT_START: i32 = 12;
    pub const ALT_ELSE: i32 = 13;
    pub const ALT_END: i32 = 14;
    pub const OPT_START: i32 = 15;
    pub const OPT_END: i32 = 16;
    pub const ACTIVE_START: i32 = 17;
    pub const ACTIVE_END: i32 = 18;
    pub const PAR_START: i32 = 19;
    pub const PAR_AND: i32 = 20;
    pub const PAR_END: i32 = 21;
    pub const RECT_START: i32 = 22;
    pub const RECT_END: i32 = 23;
    pub const SOLID_POINT: i32 = 24;
    pub const DOTTED_POINT: i32 = 25;
    pub const AUTONUMBER: i32 = 26;
    pub const CRITICAL_START: i32 = 27;
    pub const CRITICAL_OPTION: i32 = 28;
    pub const CRITICAL_END: i32 = 29;
    pub const BREAK_START: i32 = 30;
    pub const BREAK_END: i32 = 31;
    pub const PAR_OVER_START: i32 = 32;
    pub const BIDIRECTIONAL_SOLID: i32 = 33;
    pub const BIDIRECTIONAL_DOTTED: i32 = 34;
    pub const SOLID_TOP: i32 = 41;
    pub const SOLID_BOTTOM: i32 = 42;
    pub const STICK_TOP: i32 = 43;
    pub const STICK_BOTTOM: i32 = 44;
    pub const SOLID_ARROW_TOP_REVERSE: i32 = 45;
    pub const SOLID_ARROW_BOTTOM_REVERSE: i32 = 46;
    pub const STICK_ARROW_TOP_REVERSE: i32 = 47;
    pub const STICK_ARROW_BOTTOM_REVERSE: i32 = 48;
    pub const SOLID_TOP_DOTTED: i32 = 51;
    pub const SOLID_BOTTOM_DOTTED: i32 = 52;
    pub const STICK_TOP_DOTTED: i32 = 53;
    pub const STICK_BOTTOM_DOTTED: i32 = 54;
    pub const SOLID_ARROW_TOP_REVERSE_DOTTED: i32 = 55;
    pub const SOLID_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 56;
    pub const STICK_ARROW_TOP_REVERSE_DOTTED: i32 = 57;
    pub const STICK_ARROW_BOTTOM_REVERSE_DOTTED: i32 = 58;
    pub const CENTRAL_CONNECTION: i32 = 59;
    pub const CENTRAL_CONNECTION_REVERSE: i32 = 60;
    pub const CENTRAL_CONNECTION_DUAL: i32 = 61;
}

/// Where a note is put.
pub mod placement {
    pub const LEFTOF: i32 = 0;
    pub const RIGHTOF: i32 = 1;
    pub const OVER: i32 = 2;
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
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub central_connection: Option<i32>,
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
    pub placement: i32,
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
        signal_type: i32,
    },
    AddNote {
        actor: NoteActor,
        placement: i32,
        text: SeqText,
    },
    AddLinks(String, SeqText),
    AddALink(String, SeqText),
    AddProperties(String, SeqText),
    AddDetails(String, SeqText),
    AddMessage {
        from: String,
        to: String,
        signal_type: i32,
        msg: SeqText,
        activate: Option<bool>,
        central_connection: Option<i32>,
    },
    BoxStart(SeqBoxData),
    BoxEnd,
    /// The start of a block or of one of its sections, with its text.
    Section {
        text: SeqText,
        signal_type: i32,
    },
    /// The end of a block.
    End {
        signal_type: i32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub actor: String,
    pub draw: Option<&'static str>,
    pub description: Option<SeqText>,
    pub config: Option<String>,
}
