//! Pele's `diagrams/gantt/tokens.ts`.

use super::dates::Weekday;
use crate::token::grammar;

grammar! {
    /// The gantt lexer's token kinds.
    pub enum T {
        End = "$end",
        /// After `%%{`, which enters a lexer state the grammar never defines.
        Error = "INVALID",
        OpenDirective = "open_directive",
        AccTitle = "acc_title",
        AccTitleValue = "acc_title_value",
        AccDescr = "acc_descr",
        AccDescrValue = "acc_descr_value",
        AccDescrMultilineValue = "acc_descr_multiline_value",
        Nl = "NL",
        Href = "href",
        Callbackname = "callbackname",
        Callbackargs = "callbackargs",
        Click = "click",
        Gantt = "gantt",
        DateFormat = "dateFormat",
        InclusiveEndDates = "inclusiveEndDates",
        TopAxis = "topAxis",
        AxisFormat = "axisFormat",
        TickInterval = "tickInterval",
        Includes = "includes",
        Excludes = "excludes",
        TodayMarker = "todayMarker",
        WeekdayMonday = "weekday_monday",
        WeekdayTuesday = "weekday_tuesday",
        WeekdayWednesday = "weekday_wednesday",
        WeekdayThursday = "weekday_thursday",
        WeekdayFriday = "weekday_friday",
        WeekdaySaturday = "weekday_saturday",
        WeekdaySunday = "weekday_sunday",
        WeekendFriday = "weekend_friday",
        WeekendSaturday = "weekend_saturday",
        Date = "date",
        Title = "title",
        AccDescription = "accDescription",
        Section = "section",
        TaskTxt = "taskTxt",
        TaskData = "taskData",
        Colon = ":",
        Eof = "EOF",
    }

    fn got(self, _text, _previous) {
        match self {
            Self::End => "1",
            _ => self.name(),
        }
    }
}

impl T {
    /// The `weekday` statements and the day each names, from Monday.
    pub const WEEKDAYS: [(T, Weekday); 7] = [
        (T::WeekdayMonday, Weekday::Monday),
        (T::WeekdayTuesday, Weekday::Tuesday),
        (T::WeekdayWednesday, Weekday::Wednesday),
        (T::WeekdayThursday, Weekday::Thursday),
        (T::WeekdayFriday, Weekday::Friday),
        (T::WeekdaySaturday, Weekday::Saturday),
        (T::WeekdaySunday, Weekday::Sunday),
    ];

    /// The day a `weekday` statement names.
    pub fn weekday(self) -> Option<Weekday> {
        Self::WEEKDAYS
            .iter()
            .find(|(kind, _)| *kind == self)
            .map(|&(_, day)| day)
    }

    /// Whether a line can start with this kind, which is also whether it can
    /// follow a complete statement.
    pub fn starts_line(self) -> bool {
        self.weekday().is_some()
            || matches!(
                self,
                T::WeekendFriday
                    | T::WeekendSaturday
                    | T::Eof
                    | T::Nl
                    | T::DateFormat
                    | T::InclusiveEndDates
                    | T::TopAxis
                    | T::AxisFormat
                    | T::TickInterval
                    | T::Excludes
                    | T::Includes
                    | T::TodayMarker
                    | T::Title
                    | T::AccTitle
                    | T::AccDescr
                    | T::AccDescrMultilineValue
                    | T::Section
                    | T::TaskTxt
                    | T::Click
            )
    }
}
