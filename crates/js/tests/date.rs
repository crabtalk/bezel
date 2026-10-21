use js::date::{self, Date, LocalZone};

#[test]
fn the_utc_zone_reads_local_fields_as_utc() {
    let zone = LocalZone::utc();
    let at = Date::utc(2020.0, 0.0, 31.0, 13.0, 45.0, 30.0, 250.0);
    let date = zone.date(at);
    assert_eq!(date.full_year(), 2020.0);
    assert_eq!(date.month(), 0.0);
    assert_eq!(date.date(), 31.0);
    assert_eq!(date.day(), 5.0);
    assert_eq!(date.hours(), 13.0);
    assert_eq!(date.minutes(), 45.0);
    assert_eq!(date.seconds(), 30.0);
    assert_eq!(date.milliseconds(), 250.0);
    assert_eq!(date.timezone_offset(), 0.0);
    assert_eq!(
        zone.local(2020.0, 0.0, 31.0, 13.0, 45.0, 30.0, 250.0)
            .time(),
        at
    );
}

#[test]
fn the_utc_zone_parses_strings_without_an_offset_as_utc() {
    let zone = LocalZone::utc();
    let midnight = Date::utc(2024.0, 2.0, 5.0, 0.0, 0.0, 0.0, 0.0);
    assert_eq!(zone.parse("2024-03-05").time(), midnight);
    assert_eq!(zone.parse("2024/03/05").time(), midnight);
    assert_eq!(zone.parse("Mar 5 2024").time(), midnight);
    assert_eq!(
        zone.parse("2024-03-05T14:30").time(),
        midnight + 14.5 * 3_600_000.0
    );
    assert_eq!(
        zone.parse("2024-03-05T14:30+02:00").time(),
        midnight + 12.5 * 3_600_000.0
    );
    assert!(zone.parse("nope").time().is_nan());
}

#[test]
fn day_numbers_round_trip_the_calendar() {
    for days in [-800_000, -1, 0, 59, 10_957, 19_782, 2_000_000] {
        let (y, m, d) = date::civil_from_days(days);
        assert_eq!(date::days_from_civil(y, m, d), days);
    }
    assert_eq!(date::civil_from_days(0), (1970, 1, 1));
    assert_eq!(date::days_from_civil(2000, 3, 1), 11_017);
}

#[test]
fn zones_compare_by_kind() {
    assert_eq!(LocalZone::utc(), LocalZone::utc().clone());
    assert_ne!(LocalZone::utc(), LocalZone::default());
}

#[test]
fn now_is_the_clock() {
    let system = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert!((Date::now() - system).abs() < 60_000.0);
}
