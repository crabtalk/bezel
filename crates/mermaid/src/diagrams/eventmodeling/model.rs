//! Pele's `diagrams/eventmodeling/model.ts`.

use crate::diagrams::common::db::Titles;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use indexmap::IndexMap;
use serde::Serialize;

use super::parser::{EntityType, EventModelAst};
use crate::{
    detect::Kind,
    error::{Error, Place},
    js::{self},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmLane {
    /// Mermaid's ordering key: 0 for UI/Automation, 100 for Command/Read
    /// Model, 200 for Events, and the next free number after those for each
    /// namespace.
    #[serde(serialize_with = "js::json_number")]
    pub index: f64,
    pub label: String,
    pub group: Group,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Which lane this is, the same for every copy of it.
    #[serde(skip)]
    pub key: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmBox {
    /// The frame number as written, such as "01".
    pub id: String,
    pub kind: EntityType,
    pub entity: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub reset: bool,
    pub lane: EmLane,
    /// Example data from the frame or from the data block it refers to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
}

/// Copies of the two boxes; `source_index` and `target_index` are their
/// places in the model's `boxes`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmRelation {
    pub source: EmBox,
    pub target: EmBox,
    #[serde(skip)]
    pub source_index: usize,
    #[serde(skip)]
    pub target_index: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmNote {
    pub frame: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmSpecification {
    pub frame: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventModelingModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    pub boxes: Vec<EmBox>,
    /// Sorted from top to bottom.
    pub lanes: Vec<EmLane>,
    pub relations: Vec<EmRelation>,
    pub notes: Vec<EmNote>,
    pub specifications: Vec<EmSpecification>,
    pub entities: Vec<String>,
}

impl EntityType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ui => "ui",
            Self::Processor => "processor",
            Self::ReadModel => "readmodel",
            Self::Command => "command",
            Self::Event => "event",
        }
    }

    fn group(self) -> Group {
        match self {
            Self::Ui | Self::Processor => Group::Trigger,
            Self::Event => Group::Stream,
            Self::ReadModel | Self::Command => Group::State,
        }
    }
}

/// The lanes a box's kind puts it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Trigger,
    State,
    Stream,
}

impl Group {
    /// The group lane's index, its label, and the label prefix of its
    /// namespace lanes.
    fn info(self) -> (f64, &'static str, &'static str) {
        match self {
            Self::Trigger => (0.0, "UI/Automation", "UI/A: "),
            Self::State => (100.0, "Command/Read Model", "C/RM: "),
            Self::Stream => (200.0, "Events", "Stream: "),
        }
    }
}

/// The text between the outer braces, without the indentation its lines
/// share.
fn block_text(raw: &str) -> String {
    let open = raw.find('{');
    let close = raw.rfind('}');
    let inner = match (open, close) {
        (Some(open), Some(close)) if close > open => &raw[open + 1..close],
        (None, _) if js::len(raw) >= 2 => {
            let first = raw.chars().next().map_or(0, char::len_utf8);
            let last = raw.chars().next_back().map_or(0, char::len_utf8);
            &raw[first..raw.len() - last]
        }
        _ => "",
    };
    let inner = inner.replace('\r', "");
    let mut lines: Vec<&str> = inner.split('\n').collect();
    while !lines.is_empty() && js::trim(lines[0]).is_empty() {
        lines.remove(0);
    }
    while lines.last().is_some_and(|line| js::trim(line).is_empty()) {
        lines.pop();
    }
    let mut indent = usize::MAX;
    for line in &lines {
        if js::trim(line).is_empty() {
            continue;
        }
        indent = indent.min(js::len(line) - js::len(js::trim_start(line)));
    }
    lines
        .iter()
        .map(|line| js::trim_end(&line[js::units_end(line, indent)..]))
        .collect::<Vec<_>>()
        .join("\n")
}

impl EventModelingModel {
    pub fn new(ast: EventModelAst, title: Option<&str>) -> Result<Self, Error> {
        let mut ids: HashSet<&str> = HashSet::new();
        for frame in &ast.frames {
            if !ids.insert(&frame.name) {
                let line = Place::locate(&ast.source, frame.offset).line;
                let mut error = Error::semantic(format!(
                    "Duplicate event modeling frame ID \"{}\" on line {line}",
                    frame.name
                ))
                .of(Kind::Eventmodeling);
                error.line = line;
                return Err(error);
            }
        }

        let mut data: HashMap<&str, &str> = HashMap::new();
        for entity in &ast.data_entities {
            data.entry(&entity.name).or_insert(&entity.data_block_value);
        }

        // One lane per group, and one more per namespace within a group.
        let mut lane_of: IndexMap<(Group, Option<&str>), EmLane> = IndexMap::new();
        let mut next: HashMap<Group, f64> = HashMap::new();
        let mut lane = |group: Group, namespace| -> EmLane {
            let key = (group, namespace);
            if let Some(found) = lane_of.get(&key) {
                return found.clone();
            }
            let (first, label, prefix) = group.info();
            let mut index = first;
            if namespace.is_some() {
                index = next.get(&group).copied().unwrap_or(first + 1.0);
                next.insert(group, index + 1.0);
            }
            let found = EmLane {
                index,
                label: match namespace {
                    None => label.to_string(),
                    Some(namespace) => format!("{prefix}{namespace}"),
                },
                group,
                namespace: namespace.map(str::to_string),
                key: lane_of.len(),
            };
            lane_of.insert(key, found.clone());
            found
        };

        let mut boxes: Vec<EmBox> = Vec::new();
        let mut relations: Vec<EmRelation> = Vec::new();
        for (index, frame) in ast.frames.iter().enumerate() {
            // Only a two-part name has a namespace.
            let parts: Vec<&str> = frame.entity_identifier.split('.').collect();
            let namespace = (parts.len() == 2).then(|| parts[0]);
            let kind = frame.model_entity_type;
            let mut text = frame.data_inline_value.as_deref().map(block_text);
            if let Some(reference) = &frame.data_reference
                && let Some(block) = data.get(reference.as_str())
            {
                text = Some(block_text(block));
            }
            let lane = lane(kind.group(), namespace);
            let current = EmBox {
                id: frame.name.clone(),
                kind,
                entity: frame.entity_identifier.clone(),
                name: if parts.len() == 2 {
                    parts[1].to_string()
                } else {
                    frame.entity_identifier.clone()
                },
                namespace: namespace.map(str::to_string),
                reset: frame.reset,
                lane,
                data: text,
                data_type: frame.data_type.clone(),
            };
            boxes.push(current);
            let box_ = &boxes[index];

            if !frame.source_frames.is_empty() {
                // Sources connect in frame order, and only frames that already
                // have a box.
                let wanted: HashSet<&str> =
                    frame.source_frames.iter().map(String::as_str).collect();
                for (at, source) in boxes.iter().enumerate() {
                    if wanted.contains(source.id.as_str()) {
                        relations.push(EmRelation {
                            source: source.clone(),
                            target: box_.clone(),
                            source_index: at,
                            target_index: index,
                        });
                    }
                }
            } else if index > 0 && !box_.reset {
                // Otherwise the nearest earlier frame in another lane feeds this
                // one.
                for i in (0..index).rev() {
                    if boxes[i].lane.key != box_.lane.key {
                        relations.push(EmRelation {
                            source: boxes[i].clone(),
                            target: box_.clone(),
                            source_index: i,
                            target_index: index,
                        });
                        break;
                    }
                }
            }
        }

        let mut lanes: Vec<EmLane> = lane_of.into_values().collect();
        // An accessibility statement with an empty value is kept.
        let header = ast.header;
        let mut titles = Titles {
            title: header.title.filter(|title| !title.is_empty()),
            ..Titles::default()
        };
        if titles.title.is_none() {
            titles.title = title.map(str::to_string);
        }
        if let Some(text) = header.acc_title {
            titles.set_acc_title(&text);
        }
        if let Some(text) = header.acc_descr {
            titles.set_acc_descr(&text);
        }
        lanes.sort_by(|a, b| a.index.partial_cmp(&b.index).unwrap_or(Ordering::Equal));
        Ok(Self {
            kind: "eventmodeling",
            titles,
            boxes,
            lanes,
            relations,
            notes: ast
                .note_entities
                .iter()
                .map(|note| EmNote {
                    frame: note.source_frame.clone(),
                    text: block_text(&note.data_block_value),
                    data_type: note.data_type.clone(),
                })
                .collect(),
            specifications: ast
                .gwt_entities
                .iter()
                .map(|gwt| EmSpecification {
                    frame: gwt.source_frame.clone(),
                    given: gwt.given_statements.clone(),
                    when: gwt.when_statements.clone(),
                    then: gwt.then_statements.clone(),
                })
                .collect(),
            entities: ast.model_entities,
        })
    }
}
