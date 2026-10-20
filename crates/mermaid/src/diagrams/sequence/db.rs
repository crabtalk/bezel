//! Pele's `diagrams/sequence/db.ts`.

use crate::diagrams::common::db::Titles;
use crate::js::JsValue;
use indexmap::IndexMap;
use serde::{Serialize, Serializer, ser::SerializeMap};

use super::{
    colors::is_css_color,
    types::{
        CentralConnection, LineType, NoteActor, Participant, Placement, SeqActor, SeqBox,
        SeqBoxData, SeqMessage, SeqMessageText, SeqNote, SeqNumbering, SeqStatement, SeqText,
    },
};
use crate::{
    config::Value,
    detect::Kind,
    error::Error,
    js::parse_json,
    js::{self, Lazy},
    yaml::parse_yaml,
};

static RE_WRAP: Lazy = Lazy::new(r"^:?wrap:", "");
static RE_NOWRAP: Lazy = Lazy::new(r"^:?nowrap:", "");
static RE_WRAP_PREFIX: Lazy = Lazy::new(r"^:?(?:no)?wrap:", "");
static RE_BOX_FUNCTION: Lazy = Lazy::new(r"^(?:rgba?|hsla?)\s*\(", "");
static RE_LINE_TERMINATOR: Lazy = Lazy::new(r"[\r  ]", "");
static RE_WORD: Lazy = Lazy::new(r"^\w*", "");

fn fail(message: impl Into<String>) -> Error {
    Error::semantic(message).of(Kind::Sequence)
}

/// Splits a box line into a leading color and the rest, as Mermaid's
/// `/^((?:rgba?|hsla?)\s*\(.*\)|\w*)(.*)$/` does. `.` stops at a line
/// terminator, so a line that holds one outside the gap before the
/// parenthesis does not match at all.
fn split_box(text: &str) -> Option<(&str, &str)> {
    if let Some(function) = RE_BOX_FUNCTION.exec(text) {
        let head = function.end();
        if let Some(close) = text.rfind(')').filter(|&close| close >= head) {
            if RE_LINE_TERMINATOR.test(&text[head..]) {
                return None;
            }
            return Some((&text[..close + 1], &text[close + 1..]));
        }
    }
    if RE_LINE_TERMINATOR.test(text) {
        return None;
    }
    let word = RE_WORD.exec(text).map_or(0, |m| m.end());
    Some((&text[..word], &text[word..]))
}

/// `Object.entries` of a parsed value, a string giving its code points.
fn entries(value: Value) -> Vec<(String, Value)> {
    match value {
        Value::String(text) => text
            .chars()
            .enumerate()
            .map(|(i, ch)| (i.to_string(), Value::String(ch.to_string())))
            .collect(),
        Value::Array(items) => items
            .into_iter()
            .enumerate()
            .map(|(i, item)| (i.to_string(), item))
            .collect(),
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| (key.to_string(), value.clone()))
            .collect(),
        _ => Vec::new(),
    }
}

/// `text.slice(0, to)` in UTF-16 units, a negative `to` counting from the
/// end.
fn slice_units(text: &str, to: i64) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    let len = units.len() as i64;
    let to = (if to < 0 { len + to } else { to }).clamp(0, len) as usize;
    String::from_utf16_lossy(&units[..to])
}

impl SeqText {
    /// A message, without the `wrap:` or `nowrap:` that may open it.
    pub fn parse(raw: &str) -> Self {
        let text = js::trim(raw);
        let wrap = if RE_WRAP.test(text) {
            Some(true)
        } else if RE_NOWRAP.test(text) {
            Some(false)
        } else {
            None
        };
        let text = match wrap {
            None => text.to_string(),
            Some(_) => RE_WRAP_PREFIX.replace(text, |_| String::new()),
        };
        Self {
            text: js::trim(&text).to_string(),
            wrap,
        }
    }
}

impl SeqBoxData {
    /// A box line: a leading color, then the title.
    pub fn parse(text: &str) -> Self {
        let found = split_box(text);
        let mut color = match found {
            Some((color, _)) if !color.is_empty() => js::trim(color).to_string(),
            _ => "transparent".to_string(),
        };
        let mut title = match found {
            Some((_, rest)) if !rest.is_empty() => Some(js::trim(rest).to_string()),
            _ => None,
        };
        if !is_css_color(&color) {
            color = "transparent".into();
            title = Some(js::trim(text).to_string());
        }
        let Some(title) = title else {
            return Self {
                text: None,
                color,
                wrap: None,
            };
        };
        let SeqText { text, wrap } = SeqText::parse(&title);
        Self {
            text: (!text.is_empty()).then_some(text),
            color,
            wrap,
        }
    }
}

/// Pele's `SeqDb`, which is its model.
#[derive(Clone, Debug, PartialEq)]
pub struct SequenceModel {
    pub actors: IndexMap<String, SeqActor>,
    pub created_actors: IndexMap<String, usize>,
    pub destroyed_actors: IndexMap<String, usize>,
    pub boxes: Vec<SeqBox>,
    pub messages: Vec<SeqMessage>,
    pub notes: Vec<SeqNote>,
    pub titles: Titles,
    prev_actor: Option<String>,
    current_box: Option<usize>,
    last_created: Option<String>,
    last_destroyed: Option<String>,
    /// The top-level `wrap` setting, which the `%%{wrap}%%` directive also
    /// sets.
    wrap_enabled: Option<bool>,
    /// `sequence.wrap`.
    sequence_wrap: bool,
    active: IndexMap<String, i64>,
}

impl SequenceModel {
    pub fn new(wrap: Option<bool>, sequence_wrap: bool) -> Self {
        Self {
            actors: IndexMap::new(),
            created_actors: IndexMap::new(),
            destroyed_actors: IndexMap::new(),
            boxes: Vec::new(),
            messages: Vec::new(),
            notes: Vec::new(),
            titles: Titles::default(),
            prev_actor: None,
            current_box: None,
            last_created: None,
            last_destroyed: None,
            wrap_enabled: wrap,
            sequence_wrap,
            active: IndexMap::new(),
        }
    }

    pub fn auto_wrap(&self) -> bool {
        self.wrap_enabled.unwrap_or(self.sequence_wrap)
    }

    pub fn add_box(&mut self, data: SeqBoxData) {
        let wrap = data.wrap.unwrap_or_else(|| self.auto_wrap());
        self.boxes.push(SeqBox {
            name: data.text,
            wrap,
            fill: data.color,
            actor_keys: Vec::new(),
        });
        self.current_box = Some(self.boxes.len() - 1);
    }

    pub fn add_actor(
        &mut self,
        id: &str,
        name: &str,
        mut description: Option<SeqText>,
        mut kind: Option<String>,
        metadata: Option<&str>,
    ) -> Result<(), Error> {
        let mut assigned_box = self.current_box;
        let mut doc = None;
        if let Some(metadata) = metadata {
            let text = if metadata.contains('\n') {
                format!("{metadata}\n")
            } else {
                format!("{{\n{metadata}\n}}")
            };
            let parsed = parse_yaml(&text).map_err(|error| {
                Error::syntax(format!(
                    "Config of participant \"{id}\" is not valid YAML. {}",
                    error.0
                ))
                .of(Kind::Sequence)
            })?;
            if let Value::Object(map) = parsed {
                doc = Some(map);
            }
        }
        if let Some(doc_kind) = doc
            .as_ref()
            .and_then(|doc| doc.get("type"))
            .filter(|value| !matches!(value, Value::Null))
        {
            kind = Some(doc_kind.to_js_string());
        }
        if let Some(alias) = doc
            .as_ref()
            .and_then(|doc| doc.get("alias"))
            .filter(|alias| alias.truthy())
            && description.as_ref().is_none_or(|d| d.text == name)
        {
            description = Some(SeqText {
                text: alias.to_js_string(),
                wrap: description.and_then(|d| d.wrap),
            });
        }

        if let Some(old) = self.actors.get_mut(id) {
            if let (Some(current), Some(old_box)) = (self.current_box, old.box_)
                && current != old_box
            {
                let named = |index: usize| {
                    self.boxes[index]
                        .name
                        .clone()
                        .unwrap_or_else(|| "undefined".into())
                };
                return Err(fail(format!(
                    "A same participant should only be defined in one Box: {} can't be in '{}' and in '{}' at the same time.",
                    old.name,
                    named(old_box),
                    named(current)
                )));
            }
            assigned_box = old.box_.or(self.current_box);
            old.box_ = assigned_box;
            if name == old.name && description.is_none() {
                return Ok(());
            }
        }
        // A participant that is only mentioned in a message has no kind and
        // takes its name as its label.
        let description = match description {
            Some(description) if kind.is_some() => description,
            _ => SeqText {
                text: name.to_string(),
                wrap: None,
            },
        };

        let wrap = description.wrap.unwrap_or_else(|| self.auto_wrap());
        self.actors.insert(
            id.to_string(),
            SeqActor {
                box_: assigned_box,
                name: name.to_string(),
                description: description.text,
                wrap,
                prev_actor: self.prev_actor.clone(),
                next_actor: None,
                links: IndexMap::new(),
                properties: IndexMap::new(),
                details: None,
                kind: kind.unwrap_or_else(|| "participant".into()),
            },
        );
        if let Some(prev) = self.prev_actor.as_ref().filter(|prev| !prev.is_empty())
            && let Some(prev) = self.actors.get_mut(prev.as_str())
        {
            prev.next_actor = Some(id.to_string());
        }
        if let Some(current) = self.current_box {
            self.boxes[current].actor_keys.push(id.to_string());
        }
        self.prev_actor = Some(id.to_string());
        Ok(())
    }

    pub fn add_signal(
        &mut self,
        from: Option<&str>,
        to: Option<&str>,
        message: Option<&SeqText>,
        kind: LineType,
        activate: bool,
        central_connection: CentralConnection,
    ) -> Result<(), Error> {
        // Mermaid recounts every earlier message here. A running count per
        // participant says the same.
        if kind == LineType::ActiveEnd {
            let from = from.filter(|from| !from.is_empty());
            let count = from.map_or(0, |from| self.active.get(from).copied().unwrap_or(0));
            if count < 1 {
                return Err(fail(format!(
                    "Trying to inactivate an inactive participant ({})",
                    from.unwrap_or_default()
                )));
            }
            if let Some(from) = from {
                self.active.insert(from.to_string(), count - 1);
            }
        } else if kind == LineType::ActiveStart
            && let Some(from) = from.filter(|from| !from.is_empty())
        {
            let count = self.active.get(from).copied().unwrap_or(0);
            self.active.insert(from.to_string(), count + 1);
        }
        self.messages.push(SeqMessage {
            id: self.messages.len().to_string(),
            from: from.map(str::to_string),
            to: to.map(str::to_string),
            message: SeqMessageText::Text(message.map_or(String::new(), |m| m.text.clone())),
            wrap: message
                .and_then(|m| m.wrap)
                .unwrap_or_else(|| self.auto_wrap()),
            kind,
            activate: Some(activate),
            placement: None,
            central_connection: Some(central_connection),
        });
        Ok(())
    }

    pub fn add_note(&mut self, actor: NoteActor, placement: Placement, message: &SeqText) {
        let wrap = message.wrap.unwrap_or_else(|| self.auto_wrap());
        let (from, to) = match &actor {
            NoteActor::One(actor) => (actor.clone(), actor.clone()),
            NoteActor::Two([a, b]) => (a.clone(), b.clone()),
        };
        self.notes.push(SeqNote {
            actor,
            placement,
            message: message.text.clone(),
            wrap,
        });
        self.messages.push(SeqMessage {
            id: self.messages.len().to_string(),
            from: Some(from),
            to: Some(to),
            message: SeqMessageText::Text(message.text.clone()),
            wrap,
            kind: LineType::Note,
            activate: None,
            placement: Some(placement),
            central_connection: None,
        });
    }

    /// Mermaid logs and ignores menu text that is not valid JSON. It also
    /// undoes the `&amp;` and `&equals;` its sanitizer writes; Pele has no
    /// sanitizer, and a `;` cannot occur in the text.
    pub fn add_links(&mut self, actor_id: &str, text: &SeqText) {
        let Some(actor) = self.actors.get_mut(actor_id) else {
            return;
        };
        if let Some(parsed) = parse_json(&text.text) {
            for (key, value) in entries(parsed) {
                actor.links.insert(key, value);
            }
        }
    }

    pub fn add_a_link(&mut self, actor_id: &str, text: &SeqText) {
        let Some(actor) = self.actors.get_mut(actor_id) else {
            return;
        };
        let text = text.text.as_str();
        // Mermaid's `text.slice(0, at - 1)` and `text.slice(at + 1)`, with
        // `at` -1 when there is no `@`.
        let (key, value) = match text.find('@') {
            Some(at) => (
                slice_units(text, js::len(&text[..at]) as i64 - 1),
                &text[at + 1..],
            ),
            None => (slice_units(text, -2), text),
        };
        actor.links.insert(
            js::trim(&key).to_string(),
            Value::String(js::trim(value).to_string()),
        );
    }

    pub fn add_properties(&mut self, actor_id: &str, text: &SeqText) {
        let Some(actor) = self.actors.get_mut(actor_id) else {
            return;
        };
        if let Some(parsed) = parse_json(&text.text) {
            for (key, value) in entries(parsed) {
                actor.properties.insert(key, value);
            }
        }
    }

    /// Mermaid reads the properties and links from a page element with this
    /// id. Pele keeps the id.
    pub fn add_details(&mut self, actor_id: &str, text: &SeqText) {
        if let Some(actor) = self.actors.get_mut(actor_id) {
            actor.details = Some(text.text.clone());
        }
    }

    pub fn apply(&mut self, statements: impl Iterator<Item = SeqStatement>) -> Result<(), Error> {
        for statement in statements {
            self.apply_one(statement)?;
        }
        Ok(())
    }

    fn add_participant(&mut self, node: Participant) -> Result<(), Error> {
        self.add_actor(
            &node.actor,
            &node.actor,
            node.description,
            node.draw.map(|kind| kind.name().to_string()),
            node.config.as_deref(),
        )
    }

    fn apply_one(&mut self, statement: SeqStatement) -> Result<(), Error> {
        match statement {
            SeqStatement::SequenceIndex {
                index,
                step,
                visible,
            } => self.messages.push(SeqMessage {
                id: self.messages.len().to_string(),
                from: None,
                to: None,
                message: SeqMessageText::Numbering(SeqNumbering {
                    start: index,
                    step,
                    visible,
                }),
                wrap: false,
                kind: LineType::Autonumber,
                activate: None,
                placement: None,
                central_connection: None,
            }),
            SeqStatement::AddParticipant(node) => self.add_participant(node)?,
            SeqStatement::CreateParticipant(node) => {
                if self.actors.contains_key(&node.actor) {
                    return Err(fail(
                        "It is not possible to have actors with the same id, even if one is destroyed before the next is created. Use 'AS' aliases to simulate the behavior",
                    ));
                }
                let actor = node.actor.clone();
                self.last_created = Some(actor.clone());
                self.add_participant(node)?;
                self.created_actors.insert(actor, self.messages.len());
            }
            SeqStatement::DestroyParticipant(actor) => {
                self.last_destroyed = Some(actor.clone());
                self.destroyed_actors.insert(actor, self.messages.len());
            }
            SeqStatement::Mark { actor, signal_type } => self.add_signal(
                Some(&actor),
                None,
                None,
                signal_type,
                false,
                CentralConnection::Neither,
            )?,
            SeqStatement::AddNote {
                actor,
                placement,
                text,
            } => self.add_note(actor, placement, &text),
            SeqStatement::AddLinks(actor, text) => self.add_links(&actor, &text),
            SeqStatement::AddALink(actor, text) => self.add_a_link(&actor, &text),
            SeqStatement::AddProperties(actor, text) => self.add_properties(&actor, &text),
            SeqStatement::AddDetails(actor, text) => self.add_details(&actor, &text),
            SeqStatement::AddMessage {
                from,
                to,
                signal_type,
                msg,
                activate,
                central_connection,
            } => {
                if let Some(created) = self.last_created.take() {
                    if to != created {
                        return Err(fail(format!(
                            "The created participant {created} does not have an associated creating message after its declaration. Please check the sequence diagram."
                        )));
                    }
                } else if let Some(destroyed) = self.last_destroyed.take()
                    && to != destroyed
                    && from != destroyed
                {
                    return Err(fail(format!(
                        "The destroyed participant {destroyed} does not have an associated destroying message after its declaration. Please check the sequence diagram."
                    )));
                }
                self.add_signal(
                    Some(&from),
                    Some(&to),
                    Some(&msg),
                    signal_type,
                    activate.unwrap_or(false),
                    central_connection.unwrap_or_default(),
                )?;
            }
            SeqStatement::BoxStart(data) => self.add_box(data),
            SeqStatement::BoxEnd => self.current_box = None,
            SeqStatement::Section { text, signal_type } => self.add_signal(
                None,
                None,
                Some(&text),
                signal_type,
                false,
                CentralConnection::Neither,
            )?,
            SeqStatement::End { signal_type } => self.add_signal(
                None,
                None,
                None,
                signal_type,
                false,
                CentralConnection::Neither,
            )?,
        }
        Ok(())
    }
}

/// An actor of a model, with the boxes its box is looked up in.
struct InBoxes<'m>(&'m SeqActor, &'m [SeqBox]);

impl Serialize for InBoxes<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_in(self.1, s)
    }
}

impl Serialize for SequenceModel {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("type", "sequence")?;
        let actors: IndexMap<&str, InBoxes> = self
            .actors
            .iter()
            .map(|(id, actor)| (id.as_str(), InBoxes(actor, &self.boxes)))
            .collect();
        map.serialize_entry("actors", &actors)?;
        map.serialize_entry("createdActors", &self.created_actors)?;
        map.serialize_entry("destroyedActors", &self.destroyed_actors)?;
        map.serialize_entry("boxes", &self.boxes)?;
        map.serialize_entry("messages", &self.messages)?;
        map.serialize_entry("notes", &self.notes)?;
        if let Some(title) = &self.titles.title {
            map.serialize_entry("title", title)?;
        }
        if let Some(text) = &self.titles.acc_title {
            map.serialize_entry("accTitle", text)?;
        }
        if let Some(text) = &self.titles.acc_descr {
            map.serialize_entry("accDescr", text)?;
        }
        if let Some(prev) = &self.prev_actor {
            map.serialize_entry("prevActor", prev)?;
        }
        if let Some(current) = self.current_box {
            map.serialize_entry("currentBox", &self.boxes[current])?;
        }
        if let Some(created) = &self.last_created {
            map.serialize_entry("lastCreated", created)?;
        }
        if let Some(destroyed) = &self.last_destroyed {
            map.serialize_entry("lastDestroyed", destroyed)?;
        }
        if let Some(wrap) = self.wrap_enabled {
            map.serialize_entry("wrapEnabled", &wrap)?;
        }
        map.serialize_entry("sequenceWrap", &self.sequence_wrap)?;
        map.serialize_entry("active", &self.active)?;
        map.end()
    }
}
