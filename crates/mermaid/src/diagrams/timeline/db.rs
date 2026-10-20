//! Pele's `diagrams/timeline/db.ts`.

use crate::diagrams::common::db::Titles;
use serde::{Serialize, Serializer};

use crate::{Error, detect::Kind};

fn index_or_minus_one<S: Serializer>(index: &Option<usize>, s: S) -> Result<S::Ok, S::Error> {
    match index {
        Some(index) => s.serialize_u64(*index as u64),
        None => s.serialize_i64(-1),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePeriod {
    pub text: String,
    pub events: Vec<String>,
    /// Name of the section in force when the period was declared, `""`
    /// before the first one.
    pub section: String,
    /// Index into `sections` of that section, `None` before the first one,
    /// which the JSON writes as -1.
    #[serde(serialize_with = "index_or_minus_one")]
    pub section_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    /// `LR` or `TD`.
    pub direction: &'static str,
    pub sections: Vec<String>,
    pub periods: Vec<TimelinePeriod>,
    current_section: String,
}

impl Default for TimelineModel {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineModel {
    pub fn new() -> Self {
        Self {
            kind: "timeline",
            titles: Titles::default(),
            direction: "LR",
            sections: Vec::new(),
            periods: Vec::new(),
            current_section: String::new(),
        }
    }

    pub fn add_section(&mut self, text: &str) {
        self.current_section = text.to_string();
        self.sections.push(text.to_string());
    }

    pub fn add_period(&mut self, period: &str) {
        self.periods.push(TimelinePeriod {
            text: period.to_string(),
            events: Vec::new(),
            section: self.current_section.clone(),
            section_index: self.sections.len().checked_sub(1),
        });
    }

    pub fn add_event(&mut self, event: &str) -> Result<(), Error> {
        // Mermaid fails here too, with a TypeError.
        let Some(period) = self.periods.last_mut() else {
            return Err(
                Error::semantic("An event needs a time period before it.").of(Kind::Timeline)
            );
        };
        period.events.push(event.to_string());
        Ok(())
    }
}
