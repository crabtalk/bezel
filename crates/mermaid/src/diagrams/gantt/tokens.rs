//! Pele's `diagrams/gantt/tokens.ts`.

pub const END: i32 = 0;
pub const ERROR: i32 = 1;
pub const OPEN_DIRECTIVE: i32 = 2;
pub const ACC_TITLE: i32 = 3;
pub const ACC_TITLE_VALUE: i32 = 4;
pub const ACC_DESCR: i32 = 5;
pub const ACC_DESCR_VALUE: i32 = 6;
pub const ACC_DESCR_MULTILINE_VALUE: i32 = 7;
pub const NL: i32 = 8;
pub const HREF: i32 = 9;
pub const CALLBACKNAME: i32 = 10;
pub const CALLBACKARGS: i32 = 11;
pub const CLICK: i32 = 12;
pub const GANTT: i32 = 13;
pub const DATE_FORMAT: i32 = 14;
pub const INCLUSIVE_END_DATES: i32 = 15;
pub const TOP_AXIS: i32 = 16;
pub const AXIS_FORMAT: i32 = 17;
pub const TICK_INTERVAL: i32 = 18;
pub const INCLUDES: i32 = 19;
pub const EXCLUDES: i32 = 20;
pub const TODAY_MARKER: i32 = 21;
pub const WEEKDAY_MONDAY: i32 = 22;
pub const WEEKDAY_TUESDAY: i32 = 23;
pub const WEEKDAY_WEDNESDAY: i32 = 24;
pub const WEEKDAY_THURSDAY: i32 = 25;
pub const WEEKDAY_FRIDAY: i32 = 26;
pub const WEEKDAY_SATURDAY: i32 = 27;
pub const WEEKDAY_SUNDAY: i32 = 28;
pub const WEEKEND_FRIDAY: i32 = 29;
pub const WEEKEND_SATURDAY: i32 = 30;
pub const DATE: i32 = 31;
pub const TITLE: i32 = 32;
pub const ACC_DESCRIPTION: i32 = 33;
pub const SECTION: i32 = 34;
pub const TASK_TXT: i32 = 35;
pub const TASK_DATA: i32 = 36;
pub const COLON: i32 = 37;
pub const EOF: i32 = 38;

pub const TOKEN_NAMES: [&str; 39] = [
    "$end",
    "INVALID",
    "open_directive",
    "acc_title",
    "acc_title_value",
    "acc_descr",
    "acc_descr_value",
    "acc_descr_multiline_value",
    "NL",
    "href",
    "callbackname",
    "callbackargs",
    "click",
    "gantt",
    "dateFormat",
    "inclusiveEndDates",
    "topAxis",
    "axisFormat",
    "tickInterval",
    "includes",
    "excludes",
    "todayMarker",
    "weekday_monday",
    "weekday_tuesday",
    "weekday_wednesday",
    "weekday_thursday",
    "weekday_friday",
    "weekday_saturday",
    "weekday_sunday",
    "weekend_friday",
    "weekend_saturday",
    "date",
    "title",
    "accDescription",
    "section",
    "taskTxt",
    "taskData",
    ":",
    "EOF",
];
