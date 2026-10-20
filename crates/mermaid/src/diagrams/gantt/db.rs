//! Pele's `diagrams/gantt/db.ts`.

use crate::diagrams::common::db::Titles;
use indexmap::{IndexMap, IndexSet};
use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::{
    dates::{Unit, WEEKDAYS, Weekday, add, add_days, format, iso_date, parse_strict},
    types::{GanttTask, RawTask},
};
use crate::{
    Error,
    detect::Kind,
    js::{
        self, Lazy,
        date::{Date, LocalZone},
    },
    util::url::safe_url,
};

/// Scheduling walks tasks day by day and parses dates with a format the
/// diagram supplies. This bounds the total, in rough character operations,
/// so a hostile diagram fails instead of hanging.
const WORK_LIMIT: f64 = 4e8;
const DAY_STEP: f64 = 400.0;
const DATED_STEP: f64 = 200.0;
const FORMAT_STEP: f64 = 400.0;

static RE_NUMERIC_FORMAT: Lazy = Lazy::new(r"^[YMDHhmsSZXx\-/.:, T]*$", "");
static RE_TAG: Lazy = Lazy::new(r"^\s*(active|done|crit|milestone|vert)\s*$", "");
static RE_AFTER: Lazy = Lazy::new(r"^after\s+([\d\w- ]+)", "");
static RE_UNTIL: Lazy = Lazy::new(r"^until\s+([\d\w- ]+)", "");
static RE_DURATION: Lazy = Lazy::new(r"^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$", "");
static RE_TOKEN_SPLIT: Lazy = Lazy::new(r"[\s,]+", "");
static RE_DIGITS: Lazy = Lazy::new(r"^\d+$", "");

/// The one place the current time is read.
pub fn clock(now: Option<f64>) -> f64 {
    match now {
        Some(given) if given.is_finite() => given,
        _ => chrono::Utc::now().timestamp_millis() as f64,
    }
}

pub fn parse_duration(text: &str) -> (f64, Unit) {
    match RE_DURATION.exec(js::trim(text)) {
        Some(m) => (js::parse_float(m.str(1)), Unit::parse(m.str(2))),
        None => (f64::NAN, Unit::Ms),
    }
}

fn merge_tokens(existing: &[String], txt: &str) -> Vec<String> {
    let mut merged: IndexSet<String> = existing.iter().cloned().collect();
    for token in RE_TOKEN_SPLIT.split_str(&txt.to_lowercase()) {
        if !token.is_empty() {
            merged.insert(token.to_string());
        }
    }
    merged.into_iter().collect()
}

fn limit() -> Error {
    Error::limit("The gantt chart takes too much work to schedule.").of(Kind::Gantt)
}

fn invalid_date(text: &str) -> Error {
    Error::semantic(format!("Invalid date:{text}")).of(Kind::Gantt)
}

/// A time value, `NaN` for a date that is unset.
pub(super) fn time(date: Option<f64>) -> f64 {
    date.unwrap_or(f64::NAN)
}

/// The day a two-day weekend starts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekend {
    Friday,
    Saturday,
}

impl Weekend {
    /// The ISO weekday number.
    fn iso_day(self) -> f64 {
        match self {
            Weekend::Friday => 5.0,
            Weekend::Saturday => 6.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GanttDb {
    pub titles: Titles,
    pub date_format: String,
    pub axis_format: String,
    pub tick_interval: Option<String>,
    pub today_marker: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
    pub inclusive_end_dates: bool,
    pub top_axis: bool,
    pub display_mode: String,
    /// The day week-based ticks start on, when the chart names one.
    /// Mermaid's default is Sunday.
    pub weekday: Option<Weekday>,
    pub weekend: Weekend,
    pub sections: Vec<String>,
    pub links: IndexMap<String, String>,
    pub tasks: Vec<GanttTask>,

    /// Each id, and the task in `tasks` it names.
    by_id: IndexMap<String, usize>,
    current_section: String,
    task_count: usize,
    last_order: usize,
    last_task_id: Option<String>,
    now: f64,
    work: f64,
    include_set: IndexSet<String>,
    exclude_set: IndexSet<String>,
    excluded_days: Vec<bool>,
    dated_tokens: bool,
}

/// The tasks `by_id` names, as Pele's map of them serializes.
struct ById<'a>(&'a GanttDb);

impl Serialize for ById<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(
            self.0
                .by_id
                .iter()
                .map(|(id, &index)| (id, &self.0.tasks[index])),
        )
    }
}

/// The empty options object Pele's database is made with.
struct NoOptions;

impl Serialize for NoOptions {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(std::iter::empty::<(&str, &str)>())
    }
}

impl Serialize for GanttDb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = s.serialize_struct("GanttDb", 30)?;
        out.serialize_field("type", "gantt")?;
        if let Some(title) = &self.titles.title {
            out.serialize_field("title", title)?;
        }
        if let Some(acc_title) = &self.titles.acc_title {
            out.serialize_field("accTitle", acc_title)?;
        }
        if let Some(acc_descr) = &self.titles.acc_descr {
            out.serialize_field("accDescr", acc_descr)?;
        }
        out.serialize_field("dateFormat", &self.date_format)?;
        out.serialize_field("axisFormat", &self.axis_format)?;
        if let Some(tick_interval) = &self.tick_interval {
            out.serialize_field("tickInterval", tick_interval)?;
        }
        out.serialize_field("todayMarker", &self.today_marker)?;
        out.serialize_field("includes", &self.includes)?;
        out.serialize_field("excludes", &self.excludes)?;
        out.serialize_field("inclusiveEndDates", &self.inclusive_end_dates)?;
        out.serialize_field("topAxis", &self.top_axis)?;
        out.serialize_field("displayMode", &self.display_mode)?;
        if let Some(weekday) = &self.weekday {
            out.serialize_field("weekday", weekday)?;
        }
        out.serialize_field("weekend", &self.weekend)?;
        out.serialize_field("sections", &self.sections)?;
        out.serialize_field("links", &self.links)?;
        out.serialize_field("tasks", &self.tasks)?;
        out.serialize_field("byId", &ById(self))?;
        out.serialize_field("currentSection", &self.current_section)?;
        out.serialize_field("taskCount", &self.task_count)?;
        out.serialize_field("lastOrder", &self.last_order)?;
        if let Some(last_task_id) = &self.last_task_id {
            out.serialize_field("lastTaskId", last_task_id)?;
        }
        out.serialize_field("options", &NoOptions)?;
        out.serialize_field("now", &js::JsonNumber(self.now))?;
        out.serialize_field("work", &js::JsonNumber(self.work))?;
        out.serialize_field("includeSet", &self.include_set)?;
        out.serialize_field("excludeSet", &self.exclude_set)?;
        out.serialize_field("excludedDays", &self.excluded_days)?;
        out.serialize_field("datedTokens", &self.dated_tokens)?;
        out.end()
    }
}

impl Default for GanttDb {
    fn default() -> Self {
        Self::new()
    }
}

impl GanttDb {
    pub fn new() -> Self {
        Self {
            titles: Titles::default(),
            date_format: String::new(),
            axis_format: String::new(),
            tick_interval: None,
            today_marker: String::new(),
            includes: Vec::new(),
            excludes: Vec::new(),
            inclusive_end_dates: false,
            top_axis: false,
            display_mode: String::new(),
            weekday: None,
            weekend: Weekend::Saturday,
            sections: Vec::new(),
            links: IndexMap::new(),
            tasks: Vec::new(),
            by_id: IndexMap::new(),
            current_section: String::new(),
            task_count: 0,
            last_order: 0,
            last_task_id: None,
            now: 0.0,
            work: 0.0,
            include_set: IndexSet::new(),
            exclude_set: IndexSet::new(),
            excluded_days: Vec::new(),
            dated_tokens: false,
        }
    }

    pub fn set_date_format(&mut self, txt: &str) {
        self.date_format = txt.to_string();
    }

    pub fn enable_inclusive_end_dates(&mut self) {
        self.inclusive_end_dates = true;
    }

    /// Mermaid's grammar calls a method its database does not define, so
    /// `topAxis` crashes there.
    pub fn top_axis(&mut self) {
        self.top_axis = true;
    }

    pub fn set_axis_format(&mut self, txt: &str) {
        self.axis_format = txt.to_string();
    }

    pub fn set_tick_interval(&mut self, txt: &str) {
        self.tick_interval = Some(txt.to_string());
    }

    pub fn set_today_marker(&mut self, txt: &str) {
        self.today_marker = txt.to_string();
    }

    pub fn set_includes(&mut self, txt: &str) {
        self.includes = merge_tokens(&self.includes, txt);
    }

    pub fn set_excludes(&mut self, txt: &str) {
        self.excludes = merge_tokens(&self.excludes, txt);
    }

    pub fn set_weekday(&mut self, day: Weekday) {
        self.weekday = Some(day);
    }

    pub fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.sections.push(txt.to_string());
    }

    pub fn add_task(&mut self, descr: &str, data_str: &str) -> Result<(), Error> {
        let mut data: Vec<String> = data_str
            .strip_prefix(':')
            .unwrap_or(data_str)
            .split(',')
            .map(str::to_string)
            .collect();
        let mut task = GanttTask {
            id: String::new(),
            task: descr.to_string(),
            section: self.current_section.clone(),
            start_time: None,
            end_time: None,
            render_end_time: None,
            manual_end_time: false,
            processed: false,
            active: false,
            done: false,
            crit: false,
            milestone: false,
            vert: false,
            order: None,
            classes: Vec::new(),
            prev_task_id: self.last_task_id.clone(),
            raw: RawTask {
                data: data_str.to_string(),
                start: None,
                end: String::new(),
            },
        };

        while let Some(tag) = data.first().and_then(|first| RE_TAG.exec(first)) {
            match tag.str(1) {
                "active" => task.active = true,
                "done" => task.done = true,
                "crit" => task.crit = true,
                "milestone" => task.milestone = true,
                _ => task.vert = true,
            }
            data.remove(0);
        }
        // Mermaid fails with a TypeError on both of these.
        if data.is_empty() {
            return Err(Error::semantic(format!(
                "Task \"{}\" needs a duration or an end date.",
                js::trim(descr)
            ))
            .of(Kind::Gantt));
        }
        if data.len() > 3 {
            return Err(Error::semantic(format!(
                "Task \"{}\" has too many fields. A task takes an id, a start, and an end or duration.",
                js::trim(descr)
            ))
            .of(Kind::Gantt));
        }
        for item in &mut data {
            *item = js::trim(item).to_string();
        }

        if data.len() == 3 {
            task.id = data[0].clone();
        } else {
            self.task_count += 1;
            task.id = format!("task{}", self.task_count);
        }
        if data.len() > 1 {
            task.raw.start = Some(data[data.len() - 2].clone());
        }
        task.raw.end = data[data.len() - 1].clone();

        if !task.vert {
            task.order = Some(self.last_order);
            self.last_order += 1;
        }
        let id = task.id.clone();
        self.tasks.push(task);
        self.last_task_id = Some(id.clone());
        self.by_id.insert(id, self.tasks.len() - 1);
        Ok(())
    }

    pub fn set_link(&mut self, ids: &str, link: &str) {
        let href = safe_url(link);
        for id in ids.split(',') {
            if self.by_id.contains_key(id) {
                self.links.insert(id.to_string(), href.clone());
            }
        }
        self.set_class(ids, "clickable");
    }

    /// Callbacks are recognized and never run; the task is only marked
    /// clickable, as in Mermaid.
    pub fn set_click_event(&mut self, ids: &str) {
        self.set_class(ids, "clickable");
    }

    pub fn set_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            if let Some(&index) = self.by_id.get(id) {
                self.tasks[index].classes.push(class_name.to_string());
            }
        }
    }

    /// Resolves every task's start and end, from scratch on each call. A task
    /// may refer to tasks defined after it, so, like Mermaid, this makes up
    /// to eleven passes, each of which schedules every task again.
    pub(crate) fn get_tasks(
        &mut self,
        zone: &LocalZone,
        now: Option<f64>,
    ) -> Result<&[GanttTask], Error> {
        self.now = clock(now);
        self.work = 0.0;
        self.include_set = self.includes.iter().cloned().collect();
        self.exclude_set = self.excludes.iter().cloned().collect();
        let weekend_start = self.weekend.iso_day();
        let weekends = self.exclude_set.contains("weekends");
        self.excluded_days = WEEKDAYS
            .iter()
            .enumerate()
            .map(|(day, name)| {
                let iso = if day == 0 { 7.0 } else { day as f64 };
                (weekends && (iso == weekend_start || iso == weekend_start + 1.0))
                    || self.exclude_set.contains(&name.to_lowercase())
            })
            .collect();
        // A date written with only these tokens and separators can never read
        // as a weekday name, so when nothing else is excluded or included
        // there is no need to format each day.
        self.dated_tokens =
            !self.includes.is_empty() || !RE_NUMERIC_FORMAT.test(js::trim(&self.date_format));
        for token in &self.excludes {
            if token != "weekends" && !WEEKDAYS.iter().any(|day| day.to_lowercase() == *token) {
                self.dated_tokens = true;
            }
        }

        for task in &mut self.tasks {
            task.start_time = None;
            task.end_time = None;
            task.render_end_time = None;
            task.processed = false;
            task.manual_end_time = false;
        }

        let mut all_processed = self.compile_tasks(zone)?;
        let mut i = 0;
        while !all_processed && i < 10 {
            all_processed = self.compile_tasks(zone)?;
            i += 1;
        }
        Ok(&self.tasks)
    }

    /// Whether a day is excluded. A day matches a token by its weekday name,
    /// by its date as YYYY-MM-DD, or by its date written in the chart's own
    /// format.
    pub(crate) fn is_invalid_date(&mut self, zone: &LocalZone, date: f64) -> Result<bool, Error> {
        let date = zone.date(date);
        let day = date.day();
        let weekday = !day.is_nan() && self.excluded_days.get(day as usize) == Some(&true);
        if !self.dated_tokens {
            return Ok(weekday);
        }
        let template = js::trim(&self.date_format).to_string();
        let date_only = iso_date(date);
        let mut formatted = Some(date_only.clone());
        self.spend(DATED_STEP)?;
        if template != "YYYY-MM-DD" {
            self.spend(FORMAT_STEP + js::len(&template) as f64 * 8.0)?;
            formatted = format(date, &template);
        }
        let listed = |set: &IndexSet<String>| {
            set.contains(&date_only) || formatted.as_ref().is_some_and(|f| set.contains(f))
        };
        if listed(&self.include_set) {
            return Ok(false);
        }
        Ok(weekday || listed(&self.exclude_set))
    }

    fn spend(&mut self, units: f64) -> Result<(), Error> {
        self.work += units;
        if self.work > WORK_LIMIT {
            return Err(limit());
        }
        Ok(())
    }

    fn parse<'z>(
        &mut self,
        zone: &'z LocalZone,
        text: &str,
        template: &str,
    ) -> Result<Option<Date<'z>>, Error> {
        self.spend((js::len(template) as f64 + 1.0) * (js::len(text) as f64 + 1.0))?;
        Ok(parse_strict(text, template, zone.date(self.now)))
    }

    fn today(&self, zone: &LocalZone) -> f64 {
        let mut today = zone.date(self.now);
        today.set_hours(0.0, 0.0, 0.0, 0.0);
        today.time()
    }

    fn compile_tasks(&mut self, zone: &LocalZone) -> Result<bool, Error> {
        let mut all_processed = true;
        for index in 0..self.tasks.len() {
            self.compile_task(zone, index)?;
            all_processed = all_processed && self.tasks[index].processed;
        }
        Ok(all_processed)
    }

    fn compile_task(&mut self, zone: &LocalZone, index: usize) -> Result<(), Error> {
        let start = self.tasks[index].raw.start.clone();
        match start {
            None => {
                // Mermaid fails with a TypeError when the first task has no
                // start. It starts today here.
                let previous = self.tasks[index]
                    .prev_task_id
                    .as_ref()
                    .and_then(|id| self.by_id.get(id))
                    .copied();
                self.tasks[index].start_time = match previous {
                    Some(previous) => self.tasks[previous].end_time,
                    None => Some(self.today(zone)),
                };
            }
            Some(start) => {
                if let Some(start_time) = self.get_start_date(zone, &start)? {
                    self.tasks[index].start_time = Some(start_time);
                }
            }
        }
        let Some(start_time) = self.tasks[index].start_time else {
            return Ok(());
        };
        let end = self.tasks[index].raw.end.clone();
        self.tasks[index].end_time = self.get_end_date(zone, start_time, &end)?;
        if self.tasks[index].end_time.is_none() {
            return Ok(());
        }
        self.tasks[index].processed = true;
        self.tasks[index].manual_end_time = self.parse(zone, &end, "YYYY-MM-DD")?.is_some();
        self.check_task_dates(zone, index)
    }

    fn references(&self, list: &str) -> Vec<usize> {
        list.split(' ')
            .filter(|id| !id.is_empty())
            .filter_map(|id| self.by_id.get(id).copied())
            .collect()
    }

    /// The start a task's `raw.start` gives, `None` where it is a task that
    /// has no end yet.
    fn get_start_date(&mut self, zone: &LocalZone, raw: &str) -> Result<Option<f64>, Error> {
        let text = js::trim(raw);
        let template = js::trim(&self.date_format).to_string();

        if (template == "x" || template == "X") && RE_DIGITS.test(text) {
            let date = zone.date(js::number(text));
            if !date.is_valid() {
                return Err(invalid_date(text));
            }
            return Ok(Some(date.time()));
        }

        if let Some(after) = RE_AFTER.exec(text) {
            let mut latest: Option<usize> = None;
            for task in self.references(after.str(1)) {
                if latest.is_none_or(|latest| {
                    time(self.tasks[task].end_time) > time(self.tasks[latest].end_time)
                }) {
                    latest = Some(task);
                }
            }
            return Ok(match latest {
                Some(latest) => self.tasks[latest].end_time,
                None => Some(self.today(zone)),
            });
        }

        if let Some(parsed) = self.parse(zone, text, &template)? {
            return Ok(Some(parsed.time()));
        }

        // What does not fit the format gets a second chance with the
        // JavaScript date parser, as in Mermaid.
        let date = zone.parse(text);
        if !date.is_valid() || date.full_year() < -10000.0 || date.full_year() > 10000.0 {
            return Err(invalid_date(text));
        }
        Ok(Some(date.time()))
    }

    fn get_end_date(
        &mut self,
        zone: &LocalZone,
        start_time: f64,
        raw: &str,
    ) -> Result<Option<f64>, Error> {
        let text = js::trim(raw);

        if let Some(until) = RE_UNTIL.exec(text) {
            let mut earliest: Option<usize> = None;
            for task in self.references(until.str(1)) {
                if earliest.is_none_or(|earliest| {
                    time(self.tasks[task].start_time) < time(self.tasks[earliest].start_time)
                }) {
                    earliest = Some(task);
                }
            }
            return Ok(match earliest {
                Some(earliest) => self.tasks[earliest].start_time,
                None => Some(self.today(zone)),
            });
        }

        let template = js::trim(&self.date_format).to_string();
        if let Some(parsed) = self.parse(zone, text, &template)? {
            return Ok(Some(if self.inclusive_end_dates {
                add_days(parsed, 1.0).time()
            } else {
                parsed.time()
            }));
        }

        let (value, unit) = parse_duration(text);
        if value.is_nan() {
            return Ok(Some(zone.date(start_time).time()));
        }
        let end = add(zone.date(start_time), value, unit);
        Ok(Some(if end.is_valid() {
            end.time()
        } else {
            zone.date(start_time).time()
        }))
    }

    /// Pushes the end of a task out by one day for every excluded day it
    /// covers.
    fn check_task_dates(&mut self, zone: &LocalZone, index: usize) -> Result<(), Error> {
        let task = &self.tasks[index];
        if self.excludes.is_empty() || task.manual_end_time {
            return Ok(());
        }
        let mut cursor = add_days(zone.date(time(task.start_time)), 1.0);
        let mut end_time = zone.date(time(task.end_time));
        let max_end_time = add_days(end_time, 10000.0).time();
        let mut invalid = false;
        let mut render_end_time: Option<f64> = None;
        while cursor.time() <= end_time.time() {
            self.spend(DAY_STEP)?;
            if !invalid {
                render_end_time = Some(end_time.time());
            }
            invalid = self.is_invalid_date(zone, cursor.time())?;
            if invalid {
                end_time.set_date(end_time.date() + 1.0);
                if end_time.time() > max_end_time {
                    return Err(Error::semantic(
                        "Failed to find a valid date that was not excluded by `excludes` after 10,000 iterations.",
                    )
                    .of(Kind::Gantt));
                }
            }
            cursor.set_date(cursor.date() + 1.0);
        }
        self.tasks[index].end_time = Some(end_time.time());
        self.tasks[index].render_end_time = render_end_time;
        Ok(())
    }
}
