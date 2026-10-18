//! Pele's `diagrams/timeline/db.ts`.

use serde::Serialize;

use crate::mermaid::{Error, detect::Kind, js::Lazy};

static RE_LEAD: Lazy = Lazy::new(r"^\s+", "g");
static RE_INDENT: Lazy = Lazy::new(r"\n\s+", "g");

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePeriod {
    pub text: String,
    pub events: Vec<String>,
    /// Name of the section in force when the period was declared, `""`
    /// before the first one.
    pub section: String,
    /// Index into `sections` of that section, -1 before the first one.
    pub section_index: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineModel {
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acc_descr: Option<String>,
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
            title: None,
            acc_title: None,
            acc_descr: None,
            direction: "LR",
            sections: Vec::new(),
            periods: Vec::new(),
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

    pub fn set_direction(&mut self, direction: &'static str) {
        self.direction = direction;
    }

    pub fn add_section(&mut self, text: &str) {
        self.current_section = text.to_string();
        self.sections.push(text.to_string());
    }

    pub fn add_task(&mut self, period: &str, _length: f64, event: Option<&str>) {
        self.periods.push(TimelinePeriod {
            text: period.to_string(),
            events: event
                .filter(|event| !event.is_empty())
                .map(|event| vec![event.to_string()])
                .unwrap_or_default(),
            section: self.current_section.clone(),
            section_index: self.sections.len() as i64 - 1,
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
