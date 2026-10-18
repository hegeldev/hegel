use crate::common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::datetime::{Ambiguous, Granularity, Precision, UNKNOWN_ZONE};
use hegel::extras::time as time_gs;
use hegel::generators as gs;
use jiff::tz::AmbiguousOffset;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> PrimitiveDateTime {
    PrimitiveDateTime::new(
        Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap(),
        Time::from_hms(hour, minute, second).unwrap(),
    )
}

#[test]
fn test_utc_offsets_default() {
    assert_all_examples(time_gs::utc_offsets(), |o| {
        (-93_599..=93_599).contains(&o.whole_seconds())
    });
    assert_eq!(minimal(time_gs::utc_offsets(), |_| true), UtcOffset::UTC);
}

#[test]
fn test_utc_offsets_min_value() {
    let min = UtcOffset::from_hms(-1, 0, 0).unwrap();
    assert_all_examples(time_gs::utc_offsets().min_value(min), move |o| {
        o.whole_seconds() >= min.whole_seconds()
    });
}

#[test]
fn test_utc_offsets_max_value() {
    let max = UtcOffset::from_hms(1, 0, 0).unwrap();
    assert_all_examples(time_gs::utc_offsets().max_value(max), move |o| {
        o.whole_seconds() <= max.whole_seconds()
    });
}

#[test]
fn test_utc_offsets_granularity() {
    assert_all_examples(
        time_gs::utc_offsets().granularity(Granularity::Hours),
        |o| o.whole_seconds() % 3600 == 0,
    );
    assert_all_examples(
        time_gs::utc_offsets().granularity(Granularity::QuarterHours),
        |o| o.whole_seconds() % 900 == 0,
    );
    assert_all_examples(
        time_gs::utc_offsets().granularity(Granularity::Minutes),
        |o| o.seconds_past_minute() == 0,
    );
}

#[test]
fn test_utc_offsets_in_vec() {
    let max = UtcOffset::from_hms(1, 0, 0).unwrap();
    assert_all_examples(
        gs::vecs(time_gs::utc_offsets().max_value(max)).max_size(5),
        move |v| v.iter().all(|o| o.whole_seconds() <= max.whole_seconds()),
    );
}

#[test]
fn test_utc_offset_default_generator() {
    check_can_generate_examples(gs::default::<UtcOffset>());
}

#[hegel::test]
fn test_utc_offsets_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::utc_offsets());
    let hi = tc.draw(time_gs::utc_offsets().min_value(lo));
    let o = tc.draw(time_gs::utc_offsets().min_value(lo).max_value(hi));
    assert!(o.whole_seconds() >= lo.whole_seconds() && o.whole_seconds() <= hi.whole_seconds());
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_utc_offsets_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::utc_offsets()
            .min_value(UtcOffset::from_hms(1, 0, 0).unwrap())
            .max_value(UtcOffset::UTC),
    );
}

fn london() -> time_gs::OffsetDateTimeGenerator {
    time_gs::offset_datetimes()
        .time_zones(gs::time_zones().names(["Europe/London"]))
        .min_value(datetime(1900, 1, 1, 0, 0, 0))
        .max_value(datetime(2100, 12, 31, 23, 59, 59))
}

fn jiff_civil(dt: &OffsetDateTime) -> jiff::civil::DateTime {
    jiff::civil::DateTime::new(
        dt.year() as i16,
        dt.month() as i8,
        dt.day() as i8,
        dt.hour() as i8,
        dt.minute() as i8,
        dt.second() as i8,
        dt.nanosecond() as i32,
    )
    .unwrap()
}

fn london_offset_agrees_with_jiff(dt: &OffsetDateTime) -> bool {
    let instant = jiff::Timestamp::from_nanosecond(dt.unix_timestamp_nanos()).unwrap();
    let tz = jiff::tz::TimeZone::get("Europe/London").unwrap();
    tz.to_offset(instant).seconds() == dt.offset().whole_seconds()
}

fn london_fold(dt: &OffsetDateTime) -> Option<(i32, i32)> {
    let tz = jiff::tz::TimeZone::get("Europe/London").unwrap();
    match tz.to_ambiguous_timestamp(jiff_civil(dt)).offset() {
        AmbiguousOffset::Fold { before, after } => Some((before.seconds(), after.seconds())),
        _ => None,
    }
}

#[test]
fn test_offset_datetimes_default() {
    check_can_generate_examples(time_gs::offset_datetimes());
    assert_all_examples(london(), london_offset_agrees_with_jiff);
}

#[test]
fn test_offset_datetimes_shrink_to_utc_at_the_millennium() {
    assert_eq!(
        minimal(time_gs::offset_datetimes(), |_| true),
        datetime(2000, 1, 1, 0, 0, 0).assume_utc()
    );
}

#[test]
fn test_offset_datetimes_reach_both_repeats_of_a_fold() {
    find_any(london(), |dt| {
        london_fold(dt).is_some_and(|(before, _)| dt.offset().whole_seconds() == before)
    });
    find_any(london(), |dt| {
        london_fold(dt).is_some_and(|(_, after)| dt.offset().whole_seconds() == after)
    });
}

#[test]
fn test_offset_datetimes_time_zones() {
    assert_all_examples(
        time_gs::offset_datetimes().time_zones(gs::time_zones().names(["Etc/GMT+5"])),
        |dt| dt.offset() == UtcOffset::from_hms(-5, 0, 0).unwrap(),
    );
    assert_all_examples(
        time_gs::offset_datetimes().time_zones(gs::time_zones().names([UNKNOWN_ZONE])),
        |dt| dt.offset() == UtcOffset::UTC,
    );
}

#[test]
fn test_offset_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 0, 0, 0);
    assert_all_examples(time_gs::offset_datetimes().min_value(min), move |dt| {
        PrimitiveDateTime::new(dt.date(), dt.time()) >= min
    });
}

#[test]
fn test_offset_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples(time_gs::offset_datetimes().max_value(max), move |dt| {
        PrimitiveDateTime::new(dt.date(), dt.time()) <= max
    });
}

#[test]
fn test_offset_datetimes_min_instant() {
    let min = OffsetDateTime::UNIX_EPOCH;
    assert_all_examples(time_gs::offset_datetimes().min_instant(min), move |dt| {
        *dt >= min
    });
}

#[test]
fn test_offset_datetimes_max_instant() {
    let max = OffsetDateTime::UNIX_EPOCH;
    assert_all_examples(time_gs::offset_datetimes().max_instant(max), move |dt| {
        *dt <= max
    });
}

#[test]
fn test_offset_datetimes_precision() {
    assert_all_examples(
        time_gs::offset_datetimes().precision(Precision::Minutes),
        |dt| dt.second() == 0 && dt.nanosecond() == 0,
    );
}

#[test]
fn test_offset_datetimes_ambiguous() {
    assert_all_examples(london().ambiguous(Ambiguous::Earlier), |dt| {
        london_offset_agrees_with_jiff(dt)
            && london_fold(dt).is_none_or(|(before, _)| dt.offset().whole_seconds() == before)
    });
    assert_all_examples(london().ambiguous(Ambiguous::Later), |dt| {
        london_offset_agrees_with_jiff(dt)
            && london_fold(dt).is_none_or(|(_, after)| dt.offset().whole_seconds() == after)
    });
}

#[test]
fn test_offset_datetimes_in_vec() {
    let max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples(
        gs::vecs(time_gs::offset_datetimes().max_value(max)).max_size(3),
        move |v| {
            v.iter()
                .all(|dt| PrimitiveDateTime::new(dt.date(), dt.time()) <= max)
        },
    );
}

#[test]
fn test_offset_datetime_default_generator() {
    check_can_generate_examples(gs::default::<OffsetDateTime>());
}

#[hegel::test]
fn test_offset_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::primitive_datetimes());
    let hi = tc.draw(time_gs::primitive_datetimes().min_value(lo));
    let dt = tc.draw(time_gs::offset_datetimes().min_value(lo).max_value(hi));
    let local = PrimitiveDateTime::new(dt.date(), dt.time());
    assert!(local >= lo && local <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_offset_datetimes_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::offset_datetimes()
            .min_value(datetime(2025, 1, 1, 0, 0, 0))
            .max_value(datetime(2024, 1, 1, 0, 0, 0)),
    );
}

#[hegel::test]
#[should_panic(expected = "max_instant < min_instant")]
fn test_offset_datetimes_min_instant_greater_than_max_instant(tc: hegel::TestCase) {
    tc.draw(
        time_gs::offset_datetimes()
            .min_instant(datetime(2025, 1, 1, 0, 0, 0).assume_utc())
            .max_instant(datetime(2024, 1, 1, 0, 0, 0).assume_utc()),
    );
}

#[hegel::test(database = None)]
#[should_panic(expected = "FilterTooMuch")]
fn test_offset_datetimes_reject_a_wall_time_past_the_time_range(tc: hegel::TestCase) {
    let last = PrimitiveDateTime::MAX.assume_utc();
    tc.draw(
        time_gs::offset_datetimes()
            .time_zones(gs::time_zones().names(["Etc/GMT-14"]))
            .min_instant(last)
            .max_instant(last),
    );
}
