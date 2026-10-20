//! Pele's `diagrams/journey/db.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::IndexSet;
use serde::Serialize;

use crate::js::{self};

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
    #[serde(flatten)]
    pub titles: Titles,
    pub sections: Vec<String>,
    pub tasks: Vec<JourneyTask>,
    pub current_section: String,
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
            titles: Titles::default(),
            sections: Vec::new(),
            tasks: Vec::new(),
            current_section: String::new(),
        }
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

    /// Everyone named on a task, once each, in code unit order.
    pub fn actors(&self) -> Vec<String> {
        let mut names = IndexSet::new();
        for task in &self.tasks {
            for name in &task.people {
                names.insert(name.clone());
            }
        }
        let mut names: Vec<String> = names.into_iter().collect();
        js::sort_strings(&mut names);
        names
    }
}
