//! Pele's `diagrams/journey/db.ts`.

use indexmap::IndexSet;
use serde::Serialize;

use crate::mermaid::js::{self, Lazy};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyTask {
    pub task: String,
    /// As written: any number, or NaN when the score is not one.
    #[serde(serialize_with = "js::json_number")]
    pub score: f64,
    pub people: Vec<String>,
    /// Name of the section in force when the task was declared, `""` before
    /// the first one.
    pub section: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JourneyModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<JourneyTask>,
    pub current_section: String,
}

/// Everyone named on a task, once each, in code unit order.
pub fn journey_actors(tasks: &[JourneyTask]) -> Vec<String> {
    let mut names = IndexSet::new();
    for task in tasks {
        for name in &task.people {
            names.insert(name.clone());
        }
    }
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names
}

impl Default for JourneyModel {
    fn default() -> Self {
        Self::new()
    }
}

impl JourneyModel {
    pub fn new() -> Self {
        Self {
            kind: "journey",
            title: None,
            acc_title: None,
            acc_descr: None,
            sections: Vec::new(),
            tasks: Vec::new(),
            current_section: String::new(),
        }
    }

    pub fn set_diagram_title(&mut self, text: &str) {
        self.title = Some(text.to_string());
    }

    pub fn set_acc_title(&mut self, text: &str) {
        self.acc_title = Some(RE_LEAD.replace(text, |_| String::new()));
    }

    pub fn set_acc_description(&mut self, text: &str) {
        self.acc_descr = Some(RE_INDENT.replace(text, |_| "\n".into()));
    }

    pub fn add_section(&mut self, text: &str) {
        self.current_section = text.to_string();
        self.sections.push(text.to_string());
    }

    /// `data` is the rest of the line from its first colon: `:score` or
    /// `:score:actor, actor`.
    pub fn add_task(&mut self, task: &str, data: &str) {
        let pieces: Vec<&str> = data[1..].split(':').collect();
        let people = if pieces.len() == 1 {
            Vec::new()
        } else {
            pieces[1]
                .split(',')
                .map(|name| js::trim(name).to_string())
                .collect()
        };
        self.tasks.push(JourneyTask {
            task: task.to_string(),
            score: js::number(pieces[0]),
            people,
            section: self.current_section.clone(),
        });
    }

    pub fn actors(&self) -> Vec<String> {
        journey_actors(&self.tasks)
    }
}
