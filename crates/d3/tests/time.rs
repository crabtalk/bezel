use d3::time::{
    Interval, Unit,
    date::{Date, LocalZone},
};

fn utc(year: f64, month: f64, day: f64) -> f64 {
    Date::utc(year, month, day, 0.0, 0.0, 0.0, 0.0)
}

#[test]
fn an_every_interval_offsets_back_and_by_whole_steps() {
    let zone = LocalZone::utc();
    let quarter = Interval::every(Unit::Month, 3.0, 0.0, &zone);
    let offset = |at: f64, n: f64| {
        let mut d = zone.date(at);
        quarter.offset(&mut d, n);
        d.time()
    };
    assert_eq!(offset(utc(2020.0, 3.0, 15.0), -1.0), utc(2020.0, 0.0, 15.0));
    assert_eq!(offset(utc(2020.0, 1.0, 15.0), 1.0), utc(2020.0, 3.0, 15.0));
    assert_eq!(offset(utc(2020.0, 1.0, 15.0), 1.5), utc(2020.0, 3.0, 15.0));
    assert_eq!(offset(utc(2020.0, 1.0, 15.0), 2.0), utc(2020.0, 6.0, 15.0));
    assert_eq!(offset(utc(2020.0, 1.0, 15.0), 0.0), utc(2020.0, 1.0, 15.0));
}

#[test]
fn range_is_d3s_from_the_first_boundary_to_before_stop() {
    let zone = LocalZone::utc();
    let month = Interval::new(Unit::Month, 0.0);
    let start = utc(2020.0, 0.0, 15.0);
    let stop = utc(2020.0, 5.0, 1.0);
    assert_eq!(
        month.range(start, stop, 1.0, &zone),
        [1.0, 2.0, 3.0, 4.0].map(|m| utc(2020.0, m, 1.0))
    );
    assert_eq!(
        month.range(start, stop, 2.9, &zone),
        [1.0, 3.0].map(|m| utc(2020.0, m, 1.0))
    );
    assert!(month.range(start, stop, 0.5, &zone).is_empty());
    assert!(month.range(stop, start, 1.0, &zone).is_empty());
    assert_eq!(
        month.ticks(utc(2020.0, 1.0, 1.0), stop, 10.0, &zone),
        Some(
            [1.0, 2.0, 3.0, 4.0, 5.0]
                .map(|m| utc(2020.0, m, 1.0))
                .to_vec()
        )
    );
    assert_eq!(month.ticks(start, stop, 2.0, &zone), None);
}

#[test]
fn count_is_boundaries_between_floored_dates() {
    let zone = LocalZone::utc();
    let year = zone.date(utc(2020.0, 0.0, 1.0));
    let march = zone.date(utc(2020.0, 2.0, 1.0) + 5.0);
    assert_eq!(Interval::new(Unit::Day, 0.0).count(year, march), 60.0);
    assert_eq!(Interval::new(Unit::Month, 0.0).count(year, march), 2.0);
    assert_eq!(Interval::new(Unit::Year, 0.0).count(march, year), 0.0);
    assert_eq!(Interval::new(Unit::Week, 0.0).count(year, march), 9.0);
}
