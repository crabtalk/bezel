use bezel_ui::date::*;

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::new(year, month, day).expect("a real date")
}

#[test]
fn only_real_days_exist() {
    assert!(Date::new(2026, 2, 29).is_none());
    assert!(Date::new(2024, 2, 29).is_some());
    assert!(Date::new(2026, 0, 1).is_none());
    assert!(Date::new(2026, 13, 1).is_none());
    assert!(Date::new(2026, 4, 31).is_none());
    assert!(Date::new(2026, 1, 0).is_none());
}

#[test]
fn the_leap_rule_is_the_whole_rule() {
    // The century cases are the ones a wrong implementation gets wrong.
    assert!(!is_leap(1900));
    assert!(is_leap(2000));
    assert!(is_leap(2024));
    assert!(!is_leap(2026));
    assert_eq!(days_in_month(1900, 2), 28);
    assert_eq!(days_in_month(2000, 2), 29);
}

#[test]
fn days_and_dates_round_trip() {
    assert_eq!(date(1970, 1, 1).to_days(), 0);
    assert_eq!(Date::from_days(0), date(1970, 1, 1));
    // Before the epoch, where the sign handling in both directions bites.
    assert_eq!(date(1969, 12, 31).to_days(), -1);
    assert_eq!(Date::from_days(-1), date(1969, 12, 31));
    for date in [
        date(1, 1, 1),
        date(1900, 3, 1),
        date(1969, 7, 20),
        date(2000, 2, 29),
        date(2026, 8, 17),
        date(9999, 12, 31),
    ] {
        assert_eq!(Date::from_days(date.to_days()), date, "{date}");
    }
}

#[test]
fn weekdays_are_known_ones() {
    assert_eq!(date(1970, 1, 1).weekday(), Weekday::Thursday);
    assert_eq!(date(2000, 1, 1).weekday(), Weekday::Saturday);
    assert_eq!(date(2026, 8, 17).weekday(), Weekday::Monday);
    // Across the epoch, where a plain `%` would answer negative.
    assert_eq!(date(1969, 12, 31).weekday(), Weekday::Wednesday);
}

#[test]
fn adding_days_crosses_every_boundary() {
    assert_eq!(date(2026, 1, 31).add_days(1), date(2026, 2, 1));
    assert_eq!(date(2026, 12, 31).add_days(1), date(2027, 1, 1));
    assert_eq!(date(2027, 1, 1).add_days(-1), date(2026, 12, 31));
    assert_eq!(date(2024, 2, 28).add_days(1), date(2024, 2, 29));
    assert_eq!(date(2026, 2, 28).add_days(1), date(2026, 3, 1));
}

#[test]
fn adding_months_clamps_the_day() {
    // The case that makes month arithmetic its own operation.
    assert_eq!(date(2026, 1, 31).add_months(1), date(2026, 2, 28));
    assert_eq!(date(2024, 1, 31).add_months(1), date(2024, 2, 29));
    assert_eq!(date(2026, 3, 31).add_months(-1), date(2026, 2, 28));
    // Ordinary months keep the day, and the year moves with the count.
    assert_eq!(date(2026, 8, 17).add_months(1), date(2026, 9, 17));
    assert_eq!(date(2026, 12, 17).add_months(1), date(2027, 1, 17));
    assert_eq!(date(2026, 1, 17).add_months(-1), date(2025, 12, 17));
    assert_eq!(date(2026, 8, 17).add_months(-12), date(2025, 8, 17));
}

#[test]
fn a_week_starts_where_it_is_told() {
    assert_eq!(
        weekday_labels(Weekday::Monday),
        ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
    );
    assert_eq!(
        weekday_labels(Weekday::Sunday),
        ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
    );
    assert_eq!(Weekday::Monday.offset_from(Weekday::Monday), 0);
    assert_eq!(Weekday::Sunday.offset_from(Weekday::Monday), 6);
    assert_eq!(Weekday::Monday.offset_from(Weekday::Sunday), 1);
}

#[test]
fn the_grid_opens_on_the_weeks_first_day() {
    // August 2026 starts on a Saturday: a Monday-start grid opens on
    // 27 July, a Sunday-start one on 26 July.
    let grid = month_grid(date(2026, 8, 17), Weekday::Monday);
    assert_eq!(grid[0], date(2026, 7, 27));
    assert_eq!(grid[0].weekday(), Weekday::Monday);
    let grid = month_grid(date(2026, 8, 17), Weekday::Sunday);
    assert_eq!(grid[0], date(2026, 7, 26));
    assert_eq!(grid[0].weekday(), Weekday::Sunday);
}

#[test]
fn the_grid_holds_every_day_of_its_month() {
    // Including the two extremes: a 28-day February starting on the week's
    // first day, and a 31-day month starting on its last.
    for month in [date(2026, 2, 1), date(2026, 8, 1), date(2021, 5, 1)] {
        for start in [Weekday::Monday, Weekday::Sunday] {
            let grid = month_grid(month, start);
            assert_eq!(grid.len(), GRID_CELLS);
            let held = grid
                .iter()
                .filter(|day| day.month() == month.month() && day.year() == month.year())
                .count();
            assert_eq!(
                held,
                days_in_month(month.year(), month.month()) as usize,
                "{month} missing days from its own grid"
            );
            // Consecutive, with no repeats or gaps at the month seams.
            for pair in grid.windows(2) {
                assert_eq!(pair[1], pair[0].add_days(1));
            }
        }
    }
}

#[test]
fn the_grid_never_changes_height() {
    // Six rows even for the shortest possible month, so the card cannot
    // resize under the pointer as you page.
    let february = month_grid(date(2021, 2, 1), Weekday::Monday);
    assert_eq!(february.len() / 7, GRID_ROWS);
    assert_eq!(february[0], date(2021, 2, 1));
}

#[test]
fn dates_are_written_unambiguously() {
    assert_eq!(date(2026, 8, 17).to_string(), "2026-08-17");
    assert_eq!(date(1, 1, 1).to_string(), "0001-01-01");
}
