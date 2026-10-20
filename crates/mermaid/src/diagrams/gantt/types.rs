//! Pele's `diagrams/gantt/types.ts`. The model itself is
//! [`super::db::GanttDb`].

use serde::Serialize;

use crate::js::{self, date};

/// A task as written.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RawTask {
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    pub end: String,
}

/// Times are milliseconds since the epoch, written as the `Date`s Pele holds.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GanttTask {
    pub id: String,
    /// The task's label, under the name Mermaid gives it.
    pub task: String,
    pub section: String,
    /// Unset until `get_tasks` has scheduled the task, and still unset when
    /// what it refers to never resolves.
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "date::json_opt"
    )]
    pub start_time: Option<f64>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "date::json_opt"
    )]
    pub end_time: Option<f64>,
    /// Where the bar ends when excluded days pushed the task's end out;
    /// `None` when the end was given as a date.
    #[serde(serialize_with = "date::json_opt")]
    pub render_end_time: Option<f64>,
    pub manual_end_time: bool,
    pub processed: bool,
    pub active: bool,
    pub done: bool,
    pub crit: bool,
    pub milestone: bool,
    pub vert: bool,
    /// Row index in source order; `None` for vertical markers, which take
    /// no row.
    #[serde(serialize_with = "js::json_index")]
    pub order: Option<usize>,
    pub classes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_task_id: Option<String>,
    pub raw: RawTask,
}
