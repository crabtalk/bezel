//! [`Calendar`] — a date picker: the closed face of a select over an anchored
//! month grid.
//!
//! An entity for the same reason [`crate::combobox::Combobox`] is one. It owns
//! navigation state the app has no opinion about — which month is on screen,
//! where the keyboard cursor sits — and reports the one thing the app does care
//! about, through [`CalendarEvent`]. A select needs no such component, because a
//! select has no state but the caller's; a calendar does.
//!
//! [`Date`] is bezel's own, deliberately. chrono is already in the graph under
//! gpui, so taking it would cost nothing to compile — and would make it a
//! *public* dependency, so a consumer declaring its own `chrono` would end up
//! with two incompatible ones. That is the split-graph failure `bezel::gpui`
//! exists to prevent, and it buys nothing here: a picker needs no timezones, no
//! parsing and no formatting. It needs the civil calendar, which is sixty lines
//! and pure — everything above the horizontal rule below is testable without a
//! window, and is tested.
//!
//! ```ignore
//! ui::date::init(cx);   // once, at startup
//! let picker = cx.new(|cx| Calendar::new(today, cx));
//! cx.subscribe(&picker, |_, _, event, _| match event {
//!     CalendarEvent::Selected(date) => { /* the chosen day */ }
//! })
//! .detach();
//! ```

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyBinding, SharedString, Window, actions,
    div, prelude::*, px,
};

use theme::{Theme, ink};

use crate::{icons, popover, widgets, widgets::Controls};

/// A day in the proleptic Gregorian calendar.
///
/// Fields are private and [`Date::new`] is checked, so nothing downstream —
/// including the grid below — ever has to ask whether a date is real. Ordering
/// is chronological, which is what the field order buys.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

impl Date {
    /// `None` unless the day exists: month in `1..=12`, day within that month's
    /// own length, in that year — so 29 February answers differently depending
    /// on the year, which is the whole point of asking.
    pub fn new(year: i32, month: u8, day: u8) -> Option<Self> {
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    /// Days since 1970-01-01, negative before it.
    ///
    /// Howard Hinnant's `days_from_civil`, transcribed rather than re-derived
    /// (<http://howardhinnant.github.io/date_algorithms.html>, public domain).
    /// It is exact across the whole proleptic Gregorian range, and every other
    /// operation here is a round trip through it and its inverse — so there is
    /// one piece of calendar arithmetic in this file, not six.
    pub fn to_days(self) -> i64 {
        let year = self.year as i64 - (self.month <= 2) as i64;
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let year_of_era = year - era * 400;
        let month = self.month as i64;
        let day_of_year =
            (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + self.day as i64 - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146097 + day_of_era - 719468
    }

    /// The inverse of [`Date::to_days`] — Hinnant's `civil_from_days`. Total by
    /// construction: every integer is a day.
    pub fn from_days(days: i64) -> Self {
        let days = days + 719468;
        let era = if days >= 0 { days } else { days - 146096 } / 146097;
        let day_of_era = days - era * 146097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
        let year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_position = (5 * day_of_year + 2) / 153;
        let day = (day_of_year - (153 * month_position + 2) / 5 + 1) as u8;
        let month = (month_position + if month_position < 10 { 3 } else { -9 }) as u8;
        Self {
            year: (year + (month <= 2) as i64) as i32,
            month,
            day,
        }
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_days(self.to_days() + days)
    }

    /// Whole months, keeping the day where the target month has one: 31 January
    /// plus a month is 28 February, not 3 March. Clamping is what a calendar's
    /// month arrows mean — adding thirty days is a different question.
    pub fn add_months(self, months: i32) -> Self {
        // Counted in i64 so a far-fetched year cannot overflow the multiply and
        // panic — this is a library, and no input to it should be able to.
        let total = self.year as i64 * 12 + self.month as i64 - 1 + months as i64;
        let year = total.div_euclid(12) as i32;
        let month = total.rem_euclid(12) as u8 + 1;
        Self {
            year,
            month,
            day: self.day.min(days_in_month(year, month)),
        }
    }

    pub fn weekday(self) -> Weekday {
        // 1970-01-01 was a Thursday, three days into a week starting Monday.
        Weekday::from_index(((self.to_days() + 3).rem_euclid(7)) as u8)
    }
}

/// ISO 8601, because it is the one written form no locale argues with and it
/// sorts. An app wanting "17 Aug" formats it from the accessors — that is its
/// call to make, not a component library's.
impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// Days from Monday. Monday rather than Sunday only because something had
    /// to be zero; [`month_grid`] takes the week's start as an argument.
    pub fn index(self) -> u8 {
        self as u8
    }

    fn from_index(index: u8) -> Self {
        use Weekday::*;
        [
            Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday,
        ][(index % 7) as usize]
    }

    /// How many days into a week starting on `start` this weekday falls.
    pub fn offset_from(self, start: Weekday) -> u8 {
        (7 + self.index() - start.index()) % 7
    }
}

pub fn is_leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

/// Length of a month, or `0` for a month outside `1..=12` — which no [`Date`]
/// can hold, so only an unchecked caller can see it.
pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// English month names. There is no locale system here and there will not be
/// one until something needs it: localizing means shipping ICU, and an app that
/// needs it can draw its own grid from [`month_grid`], which is the reusable
/// half.
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Two-letter weekday headings, in the order a week starting on `start` runs.
pub fn weekday_labels(start: Weekday) -> [&'static str; 7] {
    const FROM_MONDAY: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    std::array::from_fn(|column| FROM_MONDAY[((start.index() as usize) + column) % 7])
}

/// Rows of a month grid — six, always.
pub const GRID_ROWS: usize = 6;
/// Cells in a month grid.
pub const GRID_CELLS: usize = GRID_ROWS * 7;

/// The block a month is drawn in: 42 real dates, beginning on the `start`
/// weekday on or before the 1st.
///
/// Six rows even for a February that fits in four, so the card never changes
/// height as you page months — a popover that resizes under the pointer moves
/// the day you were about to click. Leading and trailing cells are real dates
/// from the neighbouring months rather than blanks, which makes
/// `cell.month() != month.month()` the only test a cell needs and leaves
/// clicking one meaningful.
pub fn month_grid(month: Date, start: Weekday) -> [Date; GRID_CELLS] {
    let first = Date {
        year: month.year,
        month: month.month,
        day: 1,
    };
    let origin = first.add_days(-(first.weekday().offset_from(start) as i64));
    std::array::from_fn(|cell| origin.add_days(cell as i64))
}

// ---------------------------------------------------------------------------
// The picker
// ---------------------------------------------------------------------------

actions!(
    bezel_calendar,
    [
        PrevDay, NextDay, PrevWeek, NextWeek, PrevMonth, NextMonth, Confirm, Dismiss
    ]
);

/// The key context the picker claims, closed as well as open — `enter` on a
/// focused-but-closed picker opens it, the way a focused button presses.
pub const KEY_CONTEXT: &str = "Calendar";

/// Install the picker's bindings. Call once, alongside [`crate::input::init`].
///
/// Optional like every other `init` here: the actions are public, so an app
/// that wants different keys binds those instead. Arrows walk days and weeks
/// because the grid is two-dimensional, and `pageup`/`pagedown` page months —
/// the chords a browser's own date input uses.
pub fn init(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("left", PrevDay, ctx),
        KeyBinding::new("right", NextDay, ctx),
        KeyBinding::new("up", PrevWeek, ctx),
        KeyBinding::new("down", NextWeek, ctx),
        KeyBinding::new("pageup", PrevMonth, ctx),
        KeyBinding::new("pagedown", NextMonth, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("space", Confirm, ctx),
        KeyBinding::new("escape", Dismiss, ctx),
    ]);
}

/// What the picker reports. Emitted on choosing a day, never on merely moving
/// the cursor over one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalendarEvent {
    Selected(Date),
}

pub struct Calendar {
    /// The app's, not a clock's: bezel has no time source, and the only thing
    /// that knows which day it is where you are is the app.
    today: Date,
    selected: Option<Date>,
    /// The keyboard cursor — and, by its own month, the month on screen. One
    /// value rather than two, so walking off the end of a month and paging to
    /// the next are the same operation and cannot disagree about where you are.
    cursor: Date,
    menu: popover::Popup<()>,
    placeholder: SharedString,
    focus_handle: FocusHandle,
    week_start: Weekday,
}

impl EventEmitter<CalendarEvent> for Calendar {}

impl Calendar {
    pub fn new(today: Date, cx: &mut Context<Self>) -> Self {
        Self {
            today,
            selected: None,
            cursor: today,
            menu: popover::Popup::default(),
            placeholder: SharedString::from("Pick a date"),
            // One stop per picker, like the combobox: the grid is keyboard-
            // driven from here, so nothing inside it takes focus of its own.
            focus_handle: cx.focus_handle().tab_stop(true),
            week_start: Weekday::Monday,
        }
    }

    /// The date a form field starts on.
    pub fn with_selection(mut self, date: Date) -> Self {
        self.selected = Some(date);
        self.cursor = date;
        self
    }

    pub fn with_week_start(mut self, start: Weekday) -> Self {
        self.week_start = start;
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn selection(&self) -> Option<Date> {
        self.selected
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The note was taken on mouse-down: a menu mounted then means this
        // click is the dismissal, not a fresh open.
        if self.menu.take_press_was_open() {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Reopening lands where the value is, not where you last wandered.
        self.cursor = self.selected.unwrap_or(self.today);
        self.menu.open(());
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.menu.begin_close() {
            popover::reap_popup(cx, |calendar: &mut Self| &mut calendar.menu);
        }
        cx.notify();
    }

    fn choose(&mut self, date: Date, cx: &mut Context<Self>) {
        self.selected = Some(date);
        self.cursor = date;
        cx.emit(CalendarEvent::Selected(date));
        self.close(cx);
    }

    /// Move the cursor, if there is one to move. Every arrow lands here, and
    /// they differ only in how many days — a week is seven of them, and a month
    /// is the one step that is not a fixed number of days at all.
    fn walk(&mut self, days: i64, cx: &mut Context<Self>) {
        if self.menu.is_open() {
            self.cursor = self.cursor.add_days(days);
            cx.notify();
        }
    }

    fn page(&mut self, months: i32, cx: &mut Context<Self>) {
        if self.menu.is_open() {
            self.cursor = self.cursor.add_months(months);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &Confirm, window: &mut Window, cx: &mut Context<Self>) {
        // Closed, `enter` opens — the same key means "act on this control"
        // either way, which is what makes it reachable by keyboard at all.
        if self.menu.is_open() {
            self.choose(self.cursor, cx);
        } else {
            self.open(window, cx);
        }
    }

    fn dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        self.close(cx);
    }

    fn card(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let month = self.cursor;
        let heading = format!("{} {}", MONTHS[month.month() as usize - 1], month.year());
        let grid = month_grid(month, self.week_start);

        popover::popover_card(theme)
            .p(px(8.0))
            .gap(px(6.0))
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|calendar, _, _, cx| calendar.close(cx)))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        month_step(theme, icons::ALT_ARROW_LEFT)
                            .id("calendar-prev")
                            .on_click(cx.listener(|calendar, _, _, cx| calendar.page(-1, cx))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_align(gpui::TextAlign::Center)
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.text)
                            .child(SharedString::from(heading)),
                    )
                    .child(
                        month_step(theme, icons::ALT_ARROW_RIGHT)
                            .id("calendar-next")
                            .on_click(cx.listener(|calendar, _, _, cx| calendar.page(1, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .children(weekday_labels(self.week_start).map(|label| {
                        div()
                            .w(px(CELL))
                            .text_align(gpui::TextAlign::Center)
                            .text_size(px(10.5))
                            .text_color(theme.text_faint)
                            .child(SharedString::from(label))
                    })),
            )
            .children(grid.chunks(7).enumerate().map(|(row, week)| {
                div()
                    .flex()
                    .flex_row()
                    .children(week.iter().enumerate().map(|(column, &day)| {
                        let cell = row * 7 + column;
                        day_cell(
                            theme,
                            day,
                            day.month() == month.month(),
                            Some(day) == self.selected,
                            day == self.today,
                            day == self.cursor,
                        )
                        .id(SharedString::from(format!("day-{cell}")))
                        .on_click(cx.listener(move |calendar, _, _, cx| calendar.choose(day, cx)))
                    }))
            }))
            .into_any_element()
    }
}

/// Side of a square day cell, and of a weekday heading above it.
const CELL: f32 = 30.0;

/// A month arrow in the card's header.
fn month_step(theme: &Theme, icon: &'static str) -> gpui::Div {
    div()
        .size(px(24.0))
        .rounded(px(Theme::control_radius()))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(|s| s.bg(ink(0.06)))
        .child(
            icons::icon(icon)
                .size(px(14.0))
                .text_color(theme.text_muted),
        )
}

/// One day. The four states are carried by four different properties on
/// purpose, so no two of them can collide: the fill says selected, the text
/// tone says today or another month, the border says where the keyboard is, and
/// the wash says where the pointer is.
fn day_cell(
    theme: &Theme,
    day: Date,
    in_month: bool,
    selected: bool,
    is_today: bool,
    cursor: bool,
) -> gpui::Div {
    let text = match (selected, in_month, is_today) {
        (true, _, _) => theme.on_accent,
        (_, _, true) => theme.accent,
        (_, true, _) => theme.text,
        (_, false, _) => theme.text_faint,
    };
    div()
        .size(px(CELL))
        .rounded(px(7.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        .text_color(text)
        .when(selected, |cell| {
            cell.bg(theme.accent).font_weight(gpui::FontWeight::MEDIUM)
        })
        // The ring slot again: always a border, so moving the cursor across the
        // grid can never nudge a single cell by a pixel.
        .border_1()
        .border_color(if cursor {
            theme.caret
        } else {
            widgets::RING_SLOT
        })
        .cursor_pointer()
        .hover(|s| s.bg(if selected { theme.accent } else { ink(0.06) }))
        .child(SharedString::from(day.day().to_string()))
}

impl Focusable for Calendar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Calendar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let open = self.menu.is_open() || self.menu.is_closing();
        let label = match self.selected {
            Some(date) => SharedString::from(date.to_string()),
            None => self.placeholder.clone(),
        };
        let card = open.then(|| self.card(&theme, cx));

        div()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|calendar, _: &PrevDay, _, cx| calendar.walk(-1, cx)))
            .on_action(cx.listener(|calendar, _: &NextDay, _, cx| calendar.walk(1, cx)))
            .on_action(cx.listener(|calendar, _: &PrevWeek, _, cx| calendar.walk(-7, cx)))
            .on_action(cx.listener(|calendar, _: &NextWeek, _, cx| calendar.walk(7, cx)))
            .on_action(cx.listener(|calendar, _: &PrevMonth, _, cx| calendar.page(-1, cx)))
            .on_action(cx.listener(|calendar, _: &NextMonth, _, cx| calendar.page(1, cx)))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::dismiss))
            .relative()
            .w_full()
            .child(
                div()
                    .id("calendar-trigger")
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|calendar, _, _, _| calendar.menu.note_trigger_press()),
                    )
                    .on_click(cx.listener(|calendar, _, window, cx| calendar.toggle(window, cx)))
                    .child(theme.select_trigger(label, open)),
            )
            .when_some(card, |trigger, card| {
                trigger.child(popover::anchored_menu_below(
                    "calendar-menu",
                    card,
                    self.menu.closing_since(),
                ))
            })
    }
}
