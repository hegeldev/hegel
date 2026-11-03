use super::*;

#[test]
#[should_panic(expected = "jiff's time zone database is empty")]
fn default_names_without_a_database_is_a_usage_error() {
    default_names(&[], true, true);
}

#[test]
fn within_instant_bounds_checks_jiff_reading_against_the_bounds() {
    let zoned = Timestamp::from_second(3_600)
        .unwrap()
        .to_zoned(TimeZone::UTC);
    let bounds = |lo: i64, hi: i64| {
        ZonedBounds::Instant(
            Timestamp::from_second(lo).unwrap(),
            Timestamp::from_second(hi).unwrap(),
        )
    };
    assert!(within_instant_bounds(&zoned, &bounds(0, 7_200)));
    assert!(within_instant_bounds(&zoned, &bounds(3_600, 3_600)));
    assert!(!within_instant_bounds(&zoned, &bounds(3_601, 7_200)));
    assert!(!within_instant_bounds(&zoned, &bounds(0, 3_599)));
    assert!(within_instant_bounds(
        &zoned,
        &ZonedBounds::Civil(DateTime::constant(2000, 1, 1, 0, 0, 0, 0), MAX_CIVIL)
    ));
}
