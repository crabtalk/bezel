//! Pele's `diagrams/timeline/db.ts`.

use crate::diagrams::common::db::Titles;
use serde::{Serialize, Serializer};

use crate::{Error, detect::Kind, geom::Direction, js};

/// `TD` running down, `LR` otherwise.
fn direction_name<S: Serializer>(direction: &Direction, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(if direction.is_vertical() { "TD" } else { "LR" })
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
    #[serde(serialize_with = "js::json_index")]
    pub section_index: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(flatten)]
    pub titles: Titles,
    /// Left to right, or top to bottom.
    #[serde(serialize_with = "direction_name")]
    pub direction: Direction,
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
            direction: Direction::LeftToRight,
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
