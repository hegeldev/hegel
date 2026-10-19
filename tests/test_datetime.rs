mod common;

use common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::datetime::{
    Ambiguous, Date, DateTime, Granularity, Imaginary, Kind, MAX_INSTANT, MAX_OFFSET, MAX_YEAR,
    MIN_INSTANT, MIN_YEAR, Month, Precision, SignedDuration, Span, SpanSign, SpanUnit, Tag, Time,
    TimeZone, Timestamp, UNKNOWN_ZONE, UtcOffset, Weekday, ZonedDateTime, info, offset_at, resolve,
    tzdb_version,
};
use hegel::generators as gs;
use jiff::tz::AmbiguousOffset;

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::new(year, month, day).unwrap()
}

fn time(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Time {
    Time::new(hour, minute, second, nanosecond).unwrap()
}

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> DateTime {
    DateTime::new(date(year, month, day), time(hour, minute, second, 0)).unwrap()
}

fn jiff_date(d: Date) -> jiff::civil::Date {
    jiff::civil::Date::new(d.year as i16, d.month as i8, d.day as i8).unwrap()
}

fn jiff_datetime(dt: DateTime) -> jiff::civil::DateTime {
    jiff_date(dt.date).at(
        dt.time.hour as i8,
        dt.time.minute as i8,
        dt.time.second as i8,
        dt.time.nanosecond as i32,
    )
}

fn jiff_timestamp(t: Timestamp) -> jiff::Timestamp {
    jiff::Timestamp::new(t.second, t.nanosecond as i32).unwrap()
}

const JIFF_MIN: DateTime = DateTime {
    date: Date {
        year: -9990,
        month: 1,
        day: 1,
    },
    time: Time {
        hour: 0,
        minute: 0,
        second: 0,
        nanosecond: 0,
    },
};

const JIFF_MAX: DateTime = DateTime {
    date: Date {
        year: 9990,
        month: 12,
        day: 31,
    },
    time: Time {
        hour: 23,
        minute: 59,
        second: 59,
        nanosecond: 999_999_999,
    },
};

#[test]
fn test_date_new_checks_the_range() {
    assert!(Date::new(2024, 2, 29).is_ok());
    assert!(Date::new(2023, 2, 29).is_err());
    assert!(Date::new(1900, 2, 29).is_err());
    assert!(Date::new(2000, 2, 29).is_ok());
    assert!(Date::new(2024, 4, 31).is_err());
    assert!(Date::new(2024, 0, 1).is_err());
    assert!(Date::new(2024, 13, 1).is_err());
    assert!(Date::new(2024, 1, 0).is_err());
    assert!(Date::new(MIN_YEAR - 1, 1, 1).is_err());
    assert!(Date::new(MAX_YEAR + 1, 1, 1).is_err());
    assert!(Date::new(MIN_YEAR, 1, 1).is_ok());
    let err = Date::new(2023, 2, 29).unwrap_err();
    assert_eq!(err.to_string(), "2023-02-29 is not a valid date");
    assert!(std::error::Error::source(&err).is_none());
}

#[test]
fn test_time_new_checks_the_range() {
    assert!(Time::new(23, 59, 59, 999_999_999).is_ok());
    assert!(Time::new(24, 0, 0, 0).is_err());
    assert!(Time::new(0, 60, 0, 0).is_err());
    assert!(Time::new(0, 0, 60, 0).is_err());
    assert!(Time::new(0, 0, 0, 1_000_000_000).is_err());
}

#[test]
fn test_datetime_new_checks_both_parts() {
    let bad_date = Date {
        year: 2024,
        month: 2,
        day: 30,
    };
    let bad_time = Time {
        hour: 25,
        minute: 0,
        second: 0,
        nanosecond: 0,
    };
    assert!(DateTime::new(bad_date, time(0, 0, 0, 0)).is_err());
    assert!(DateTime::new(date(2024, 1, 1), bad_time).is_err());
    assert!(DateTime::new(date(2024, 1, 1), time(0, 0, 0, 0)).is_ok());
}

#[test]
fn test_timestamp_new_checks_the_range() {
    assert!(Timestamp::new(MIN_INSTANT, 0).is_ok());
    assert!(Timestamp::new(MAX_INSTANT, 999_999_999).is_ok());
    assert!(Timestamp::new(MIN_INSTANT - 1, 0).is_err());
    assert!(Timestamp::new(MAX_INSTANT + 1, 0).is_err());
    assert!(Timestamp::new(0, 1_000_000_000).is_err());
}

#[test]
fn test_utc_offset_new_checks_the_range() {
    assert_eq!(
        UtcOffset::new(MAX_OFFSET).unwrap(),
        UtcOffset::Known(MAX_OFFSET)
    );
    assert_eq!(
        UtcOffset::new(-MAX_OFFSET).unwrap(),
        UtcOffset::Known(-MAX_OFFSET)
    );
    assert!(UtcOffset::new(MAX_OFFSET + 1).is_err());
    assert!(UtcOffset::new(-MAX_OFFSET - 1).is_err());
}

#[test]
fn test_signed_duration_new_checks_the_range() {
    assert!(SignedDuration::new(-1, -500_000_000).is_ok());
    assert!(SignedDuration::new(0, -500_000_000).is_ok());
    assert!(SignedDuration::new(-1, 0).is_ok());
    assert!(SignedDuration::new(-1, 500_000_000).is_err());
    assert!(SignedDuration::new(1, -500_000_000).is_err());
    assert!(SignedDuration::new(1, 1_000_000_000).is_err());
}

#[test]
fn test_time_zone_new_resolves_links() {
    let zone = TimeZone::new("Asia/Calcutta").unwrap();
    assert_eq!(zone.name, "Asia/Calcutta");
    assert_eq!(zone.primary_name, "Asia/Kolkata");
    assert_eq!(zone.to_string(), "Asia/Calcutta");
    let err = TimeZone::new("Mars/Olympus").unwrap_err();
    assert!(err.to_string().contains("Mars/Olympus"));
    assert!(TimeZone::new("asia/kolkata").is_err());
}

#[test]
fn test_display_dates_and_times() {
    assert_eq!(date(2024, 3, 10).to_string(), "2024-03-10");
    assert_eq!(date(1, 1, 1).to_string(), "0001-01-01");
    assert_eq!(date(10000, 1, 1).to_string(), "+010000-01-01");
    assert_eq!(date(-1, 12, 31).to_string(), "-000001-12-31");
    assert_eq!(time(2, 30, 0, 0).to_string(), "02:30:00");
    assert_eq!(time(2, 30, 0, 500_000_000).to_string(), "02:30:00.5");
    assert_eq!(time(2, 30, 0, 1).to_string(), "02:30:00.000000001");
    assert_eq!(
        datetime(2024, 3, 10, 2, 30, 0).to_string(),
        "2024-03-10T02:30:00"
    );
}

#[test]
fn test_display_timestamps() {
    assert_eq!(
        Timestamp::new(946_684_800, 0).unwrap().to_string(),
        "2000-01-01T00:00:00Z"
    );
    assert_eq!(
        Timestamp::new(0, 0).unwrap().to_string(),
        "1970-01-01T00:00:00Z"
    );
    assert_eq!(
        Timestamp::new(-1, 0).unwrap().to_string(),
        "1969-12-31T23:59:59Z"
    );
    assert_eq!(
        Timestamp::new(-2, 500_000_000).unwrap().to_string(),
        "1969-12-31T23:59:58.5Z"
    );
    assert_eq!(
        Timestamp::new(MIN_INSTANT, 0).unwrap().to_string(),
        "-999999-01-01T00:00:00Z"
    );
    assert_eq!(
        Timestamp::new(MAX_INSTANT, 0).unwrap().to_string(),
        "+999999-12-31T23:59:59Z"
    );
}

#[test]
fn test_display_offsets() {
    assert_eq!(UtcOffset::Known(0).to_string(), "+00:00");
    assert_eq!(UtcOffset::Known(19_800).to_string(), "+05:30");
    assert_eq!(UtcOffset::Known(-18_000).to_string(), "-05:00");
    assert_eq!(UtcOffset::Known(-2_670).to_string(), "-00:44:30");
    assert_eq!(UtcOffset::Unknown.to_string(), "-00:00");
}

#[test]
fn test_display_signed_durations() {
    assert_eq!(SignedDuration::new(0, 0).unwrap().to_string(), "PT0S");
    assert_eq!(
        SignedDuration::new(5405, 500_000_000).unwrap().to_string(),
        "PT1H30M5.5S"
    );
    assert_eq!(SignedDuration::new(3600, 0).unwrap().to_string(), "PT1H");
    assert_eq!(
        SignedDuration::new(-1, -500_000_000).unwrap().to_string(),
        "-PT1.5S"
    );
    assert_eq!(
        SignedDuration::new(0, -1).unwrap().to_string(),
        "-PT0.000000001S"
    );
}

#[test]
fn test_display_spans() {
    assert_eq!(Span::default().to_string(), "0s");
    let every = Span {
        years: 1,
        months: 2,
        weeks: 3,
        days: 4,
        hours: 5,
        minutes: 6,
        seconds: 7,
        milliseconds: 8,
        microseconds: 9,
        nanoseconds: 10,
    };
    assert_eq!(every.to_string(), "1y 2mo 3w 4d 5h 6m 7s 8ms 9µs 10ns");
    let negative = Span {
        years: -1,
        months: -2,
        ..Span::default()
    };
    assert_eq!(negative.to_string(), "-1y 2mo");
    let mixed = Span {
        years: 1,
        months: -2,
        ..Span::default()
    };
    assert_eq!(mixed.to_string(), "1y -2mo");
    for unit in SpanUnit::ALL {
        assert_eq!(every.get(unit), unit as i64 + 1);
    }
}

#[test]
fn test_display_zoned_datetimes_matches_the_engine_form() {
    let gap = resolve("America/New_York", datetime(2024, 3, 10, 2, 30, 0)).unwrap();
    assert_eq!(gap.kind, Kind::Gap);
    assert_eq!(gap.offset_before, -18_000);
    assert_eq!(gap.offset_after, -14_400);
    assert_eq!(gap.offset, UtcOffset::Known(gap.offset_after));
    assert_eq!(
        gap.to_string(),
        "2024-03-10T02:30:00-04:00[America/New_York][gap]"
    );
    assert_eq!(gap.timestamp.to_string(), "2024-03-10T07:30:00Z");

    let fold = resolve("America/New_York", datetime(2024, 11, 3, 1, 30, 0)).unwrap();
    assert_eq!(fold.kind, Kind::Fold);
    assert!(!fold.fold);
    assert_eq!(
        fold.to_string(),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
    );
    let later = ZonedDateTime {
        fold: true,
        offset: UtcOffset::Known(fold.offset_after),
        ..fold.clone()
    };
    assert_eq!(
        later.to_string(),
        "2024-11-03T01:30:00-05:00[America/New_York][fold=1]"
    );

    let unique = resolve("America/New_York", datetime(2024, 6, 1, 12, 0, 0)).unwrap();
    assert_eq!(unique.kind, Kind::Unique);
    assert_eq!(unique.offset, UtcOffset::Known(-14_400));
    assert_eq!(
        unique.to_string(),
        "2024-06-01T12:00:00-04:00[America/New_York]"
    );

    let unknown = resolve(UNKNOWN_ZONE, datetime(2024, 6, 1, 12, 0, 0)).unwrap();
    assert_eq!(unknown.offset, UtcOffset::Unknown);
    assert_eq!(unknown.to_string(), "2024-06-01T12:00:00Z[Etc/Unknown]");
    assert_eq!(unknown.timestamp.to_string(), "2024-06-01T12:00:00Z");

    let opaque = ZonedDateTime {
        zone: TimeZone {
            name: "Mars/Olympus".to_string(),
            primary_name: "Mars/Olympus".to_string(),
        },
        offset: UtcOffset::Unknown,
        ..unknown
    };
    assert_eq!(
        opaque.to_string(),
        "2024-06-01T12:00:00-00:00[Mars/Olympus]"
    );

    let calcutta = resolve("Asia/Calcutta", datetime(2024, 6, 1, 12, 0, 0)).unwrap();
    assert_eq!(calcutta.zone.primary_name, "Asia/Kolkata");
    assert_eq!(
        calcutta.to_string(),
        "2024-06-01T12:00:00+05:30[Asia/Calcutta]"
    );
}

#[test]
fn test_resolve_rejects_bad_input() {
    assert!(resolve("Mars/Olympus", datetime(2024, 6, 1, 12, 0, 0)).is_err());
    let bad = DateTime {
        date: Date {
            year: 2024,
            month: 2,
            day: 30,
        },
        time: time(0, 0, 0, 0),
    };
    assert!(resolve("UTC", bad).is_err());
    let last = datetime(MAX_YEAR, 12, 31, 23, 59, 59);
    assert!(resolve("America/New_York", last).is_err());
    assert_eq!(
        resolve("UTC", last).unwrap().timestamp,
        Timestamp::new(MAX_INSTANT, 0).unwrap()
    );
}

#[test]
fn test_offset_at_reports_the_offset_in_force() {
    let summer = offset_at("Europe/London", Timestamp::new(1_717_243_200, 0).unwrap()).unwrap();
    assert_eq!(summer.offset, UtcOffset::Known(3600));
    assert!(summer.is_dst);
    assert_eq!(summer.abbreviation, "BST");
    let winter = offset_at("Europe/London", Timestamp::new(1_704_067_200, 0).unwrap()).unwrap();
    assert_eq!(winter.offset, UtcOffset::Known(0));
    assert!(!winter.is_dst);
    assert_eq!(winter.abbreviation, "GMT");
    let unknown = offset_at(UNKNOWN_ZONE, Timestamp::new(0, 0).unwrap()).unwrap();
    assert_eq!(unknown.offset, UtcOffset::Unknown);
    assert_eq!(unknown.abbreviation, "-00");
    assert!(offset_at("Mars/Olympus", Timestamp::new(0, 0).unwrap()).is_err());
    let out_of_range = Timestamp {
        second: 0,
        nanosecond: 1_000_000_000,
    };
    assert!(offset_at("UTC", out_of_range).is_err());
}

#[test]
fn test_info_describes_names() {
    let utc = info("UTC").unwrap();
    assert_eq!(utc.primary_name, "UTC");
    assert!(!utc.is_link);
    assert_eq!(utc.rank, 0);
    assert!(utc.tags.contains(&Tag::Fixed));
    let calcutta = info("Asia/Calcutta").unwrap();
    assert_eq!(calcutta.primary_name, "Asia/Kolkata");
    assert!(calcutta.is_link);
    assert!(calcutta.tags.contains(&Tag::Link));
    assert!(calcutta.tags.contains(&Tag::Fractional));
    let dublin = info("Europe/Dublin").unwrap();
    assert!(dublin.tags.contains(&Tag::Dst));
    let unknown = info(UNKNOWN_ZONE).unwrap();
    assert_eq!(unknown.primary_name, UNKNOWN_ZONE);
    assert_eq!(unknown.tags, vec![Tag::Undefined]);
    assert_eq!(unknown.rank, u16::MAX);
    assert!(info("Mars/Olympus").is_none());
    assert!(info("UTC\0").is_none());
}

#[test]
fn test_tzdb_version_is_an_iana_release() {
    let version = tzdb_version();
    assert_eq!(version.len(), 5, "{version}");
    assert!(
        version[..4].chars().all(|c| c.is_ascii_digit()),
        "{version}"
    );
    assert!(
        version.ends_with(|c: char| c.is_ascii_lowercase()),
        "{version}"
    );
}

#[test]
fn test_dates_default() {
    assert_all_examples(gs::dates(), |d| Date::new(d.year, d.month, d.day).is_ok());
}

#[test]
fn test_dates_min_value() {
    let min = date(2024, 1, 1);
    assert_all_examples(gs::dates().min_value(min), move |d| *d >= min);
}

#[test]
fn test_dates_max_value() {
    let max = date(1999, 12, 31);
    assert_all_examples(gs::dates().max_value(max), move |d| *d <= max);
}

#[test]
fn test_dates_weekdays() {
    assert_all_examples(
        gs::dates()
            .min_value(date(-9999, 1, 1))
            .max_value(date(9999, 12, 31))
            .weekdays([Weekday::Saturday, Weekday::Sunday]),
        |d| {
            let weekday = jiff_date(*d).weekday();
            weekday == jiff::civil::Weekday::Saturday || weekday == jiff::civil::Weekday::Sunday
        },
    );
}

#[test]
fn test_dates_months() {
    assert_all_examples(
        gs::dates().months([Month::February, Month::December]),
        |d| d.month == 2 || d.month == 12,
    );
}

#[test]
fn test_dates_in_vec() {
    let max = date(1999, 12, 31);
    assert_all_examples(gs::vecs(gs::dates().max_value(max)).max_size(5), move |v| {
        v.iter().all(|d| *d <= max)
    });
}

#[test]
fn test_dates_shrink_toward_the_millennium() {
    assert_eq!(minimal(gs::dates(), |_| true), date(2000, 1, 1));
}

#[test]
fn test_dates_default_generator() {
    check_can_generate_examples(gs::default::<Date>());
}

#[hegel::test]
fn test_dates_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::dates());
    let hi = tc.draw(gs::dates().min_value(lo));
    let d = tc.draw(gs::dates().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
}

#[test]
fn test_times_default() {
    assert_all_examples(gs::times(), |t| {
        Time::new(t.hour, t.minute, t.second, t.nanosecond).is_ok()
    });
}

#[test]
fn test_times_min_value() {
    let min = time(12, 0, 0, 0);
    assert_all_examples(gs::times().min_value(min), move |t| *t >= min);
}

#[test]
fn test_times_max_value() {
    let max = time(12, 0, 0, 0);
    assert_all_examples(gs::times().max_value(max), move |t| *t <= max);
}

#[test]
fn test_times_precision() {
    assert_all_examples(gs::times().precision(Precision::Minutes), |t| {
        t.second == 0 && t.nanosecond == 0
    });
    assert_all_examples(gs::times().precision(Precision::Microseconds), |t| {
        t.nanosecond % 1_000 == 0
    });
    assert_all_examples(gs::times().precision(Precision::Milliseconds), |t| {
        t.nanosecond % 1_000_000 == 0
    });
}

#[test]
fn test_times_in_vec() {
    let max = time(12, 0, 0, 0);
    assert_all_examples(gs::vecs(gs::times().max_value(max)).max_size(5), move |v| {
        v.iter().all(|t| *t <= max)
    });
}

#[test]
fn test_times_shrink_toward_midnight() {
    assert_eq!(minimal(gs::times(), |_| true), time(0, 0, 0, 0));
    assert_eq!(
        minimal(gs::times().min_value(time(6, 30, 0, 0)), |_| true),
        time(6, 30, 0, 0)
    );
}

#[test]
fn test_times_default_generator() {
    check_can_generate_examples(gs::default::<Time>());
}

#[hegel::test]
fn test_times_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::times());
    let hi = tc.draw(gs::times().min_value(lo));
    let t = tc.draw(gs::times().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[test]
fn test_datetimes_default() {
    assert_all_examples(gs::datetimes(), |dt| {
        DateTime::new(dt.date, dt.time).is_ok()
    });
}

#[test]
fn test_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 12, 0, 0);
    assert_all_examples(gs::datetimes().min_value(min), move |dt| *dt >= min);
}

#[test]
fn test_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(gs::datetimes().max_value(max), move |dt| *dt <= max);
}

#[test]
fn test_datetimes_precision() {
    assert_all_examples(gs::datetimes().precision(Precision::Hours), |dt| {
        dt.time.minute == 0 && dt.time.second == 0 && dt.time.nanosecond == 0
    });
}

#[test]
fn test_datetimes_in_vec() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(
        gs::vecs(gs::datetimes().max_value(max)).max_size(5),
        move |v| v.iter().all(|dt| *dt <= max),
    );
}

#[test]
fn test_datetimes_shrink_toward_the_millennium() {
    assert_eq!(
        minimal(gs::datetimes(), |_| true),
        datetime(2000, 1, 1, 0, 0, 0)
    );
}

#[test]
fn test_datetimes_default_generator() {
    check_can_generate_examples(gs::default::<DateTime>());
}

#[hegel::test]
fn test_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::datetimes());
    let hi = tc.draw(gs::datetimes().min_value(lo));
    let dt = tc.draw(gs::datetimes().min_value(lo).max_value(hi));
    assert!(dt >= lo && dt <= hi);
}

#[test]
fn test_timestamps_default() {
    assert_all_examples(gs::timestamps(), |t| {
        Timestamp::new(t.second, t.nanosecond).is_ok()
    });
}

#[test]
fn test_timestamps_min_value() {
    let min = Timestamp::new(0, 0).unwrap();
    assert_all_examples(gs::timestamps().min_value(min), move |t| *t >= min);
}

#[test]
fn test_timestamps_max_value() {
    let max = Timestamp::new(0, 0).unwrap();
    assert_all_examples(gs::timestamps().max_value(max), move |t| *t <= max);
}

#[test]
fn test_timestamps_precision() {
    assert_all_examples(gs::timestamps().precision(Precision::Seconds), |t| {
        t.nanosecond == 0
    });
}

#[test]
fn test_timestamps_in_vec() {
    let max = Timestamp::new(0, 0).unwrap();
    assert_all_examples(
        gs::vecs(gs::timestamps().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_timestamps_shrink_toward_the_millennium() {
    assert_eq!(
        minimal(gs::timestamps(), |_| true),
        Timestamp::new(946_684_800, 0).unwrap()
    );
}

#[test]
fn test_timestamps_default_generator() {
    check_can_generate_examples(gs::default::<Timestamp>());
}

#[hegel::test]
fn test_timestamps_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::timestamps());
    let hi = tc.draw(gs::timestamps().min_value(lo));
    let t = tc.draw(gs::timestamps().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[hegel::test]
fn test_timestamps_and_dates_print_as_jiff_does(tc: hegel::TestCase) {
    let min = Timestamp::new(-377_700_000_000, 0).unwrap();
    let max = Timestamp::new(253_402_207_200, 0).unwrap();
    let t = tc.draw(gs::timestamps().min_value(min).max_value(max));
    assert_eq!(t.to_string(), jiff_timestamp(t).to_string());
    let d = tc.draw(
        gs::dates()
            .min_value(JIFF_MIN.date)
            .max_value(JIFF_MAX.date),
    );
    assert_eq!(d.to_string(), jiff_date(d).to_string());
    let dt = tc.draw(gs::datetimes().min_value(JIFF_MIN).max_value(JIFF_MAX));
    assert_eq!(dt.to_string(), jiff_datetime(dt).to_string());
}

#[test]
fn test_utc_offsets_default() {
    assert_all_examples(
        gs::utc_offsets(),
        |o| matches!(o, UtcOffset::Known(s) if s.unsigned_abs() <= MAX_OFFSET.unsigned_abs()),
    );
}

#[test]
fn test_utc_offsets_min_value() {
    assert_all_examples(
        gs::utc_offsets().min_value(UtcOffset::Known(0)),
        |o| matches!(o, UtcOffset::Known(s) if *s >= 0),
    );
}

#[test]
fn test_utc_offsets_max_value() {
    assert_all_examples(
        gs::utc_offsets().max_value(UtcOffset::Known(0)),
        |o| matches!(o, UtcOffset::Known(s) if *s <= 0),
    );
}

#[test]
fn test_utc_offsets_granularity() {
    assert_all_examples(
        gs::utc_offsets().granularity(Granularity::Hours),
        |o| matches!(o, UtcOffset::Known(s) if s % 3600 == 0),
    );
    assert_all_examples(
        gs::utc_offsets().granularity(Granularity::QuarterHours),
        |o| matches!(o, UtcOffset::Known(s) if s % 900 == 0),
    );
    assert_all_examples(
        gs::utc_offsets().granularity(Granularity::Minutes),
        |o| matches!(o, UtcOffset::Known(s) if s % 60 == 0),
    );
}

#[test]
fn test_utc_offsets_unknown() {
    find_any(gs::utc_offsets().unknown(true), |o| {
        *o == UtcOffset::Unknown
    });
}

#[test]
fn test_utc_offsets_unknown_shrinks_to_a_known_offset() {
    assert_eq!(
        minimal(gs::utc_offsets().unknown(true), |_| true),
        UtcOffset::Known(0)
    );
    assert_eq!(
        minimal(gs::utc_offsets().unknown(true), |o| {
            *o == UtcOffset::Unknown || matches!(o, UtcOffset::Known(s) if s.unsigned_abs() >= 3600)
        }),
        UtcOffset::Known(3600)
    );
    assert_eq!(
        minimal(gs::utc_offsets().unknown(true), |o| *o
            == UtcOffset::Unknown),
        UtcOffset::Unknown
    );
}

#[test]
fn test_utc_offsets_in_vec() {
    assert_all_examples(
        gs::vecs(gs::utc_offsets().max_value(UtcOffset::Known(0))).max_size(5),
        |v| {
            v.iter()
                .all(|o| matches!(o, UtcOffset::Known(s) if *s <= 0))
        },
    );
}

#[test]
fn test_utc_offsets_default_generator() {
    check_can_generate_examples(gs::default::<UtcOffset>());
}

#[hegel::test]
fn test_utc_offsets_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i32>()
            .min_value(-MAX_OFFSET)
            .max_value(MAX_OFFSET),
    );
    let hi = tc.draw(gs::integers::<i32>().min_value(lo).max_value(MAX_OFFSET));
    let granularity = tc.draw(gs::sampled_from(vec![
        Granularity::Hours,
        Granularity::QuarterHours,
        Granularity::Minutes,
        Granularity::Seconds,
    ]));
    let unit = match granularity {
        Granularity::Hours => 3600,
        Granularity::QuarterHours => 900,
        Granularity::Minutes => 60,
        Granularity::Seconds => 1,
    };
    tc.assume(-(-lo).div_euclid(unit) <= hi.div_euclid(unit));
    let o = tc.draw(
        gs::utc_offsets()
            .min_value(UtcOffset::Known(lo))
            .max_value(UtcOffset::Known(hi))
            .granularity(granularity),
    );
    let UtcOffset::Known(s) = o else {
        panic!("unknown offset drawn without unknown(true)")
    };
    assert!(s >= lo && s <= hi && s % unit == 0);
}

#[test]
fn test_time_zones_default() {
    assert_all_examples(gs::time_zones(), |zone| {
        let info = info(&zone.name).unwrap();
        info.primary_name == zone.primary_name
            && !info.tags.contains(&Tag::Legacy)
            && zone.name != UNKNOWN_ZONE
            && jiff::tz::TimeZone::get(&zone.name).is_ok()
    });
}

#[test]
fn test_time_zones_names() {
    let names = ["Europe/London", "Asia/Calcutta"];
    assert_all_examples(gs::time_zones().names(names), move |zone| {
        names.contains(&zone.name.as_str())
    });
    find_any(gs::time_zones().names(names), |zone| {
        zone.primary_name == "Asia/Kolkata"
    });
}

#[test]
fn test_time_zones_links() {
    assert_all_examples(gs::time_zones().links(false), |zone| {
        !info(&zone.name).unwrap().is_link
    });
    find_any(gs::time_zones(), |zone| info(&zone.name).unwrap().is_link);
}

#[test]
fn test_time_zones_legacy() {
    find_any(gs::time_zones().legacy(true), |zone| {
        info(&zone.name).unwrap().tags.contains(&Tag::Legacy)
    });
}

#[test]
fn test_time_zones_unknown() {
    find_any(gs::time_zones().unknown(true), |zone| {
        zone.name == UNKNOWN_ZONE
    });
}

#[test]
fn test_time_zones_opaque() {
    assert_all_examples(
        gs::time_zones().names(["Mars/Olympus"]).opaque(true),
        |zone| zone.name == "Mars/Olympus" && zone.primary_name == "Mars/Olympus",
    );
}

#[test]
fn test_time_zones_shrink_to_utc() {
    assert_eq!(minimal(gs::time_zones(), |_| true).name, "UTC");
}

#[test]
fn test_time_zones_in_vec() {
    assert_all_examples(gs::vecs(gs::time_zones()).max_size(5), |v| {
        v.iter().all(|zone| info(&zone.name).is_some())
    });
}

#[test]
fn test_time_zones_default_generator() {
    check_can_generate_examples(gs::default::<TimeZone>());
}

fn zoned_agrees_with_jiff(z: &ZonedDateTime) -> bool {
    let tz = jiff::tz::TimeZone::get(&z.zone.name).unwrap();
    let ambiguous = tz.to_ambiguous_timestamp(jiff_datetime(z.civil));
    let (kind, before, after) = match ambiguous.offset() {
        AmbiguousOffset::Unambiguous { offset } => (Kind::Unique, offset, offset),
        AmbiguousOffset::Gap { before, after } => (Kind::Gap, before, after),
        AmbiguousOffset::Fold { before, after } => (Kind::Fold, before, after),
    };
    let expected = match (kind, z.fold) {
        (Kind::Fold, true) => ambiguous.later().unwrap(),
        _ => ambiguous.compatible().unwrap(),
    };
    let resolved = match kind {
        Kind::Gap => after,
        Kind::Fold if z.fold => after,
        _ => before,
    };
    kind == z.kind
        && before.seconds() == z.offset_before
        && after.seconds() == z.offset_after
        && z.offset == UtcOffset::Known(resolved.seconds())
        && expected == jiff_timestamp(z.timestamp)
        && (z.kind == Kind::Fold || !z.fold)
}

#[test]
fn test_zoned_datetimes_default() {
    assert_all_examples(
        gs::zoned_datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        zoned_agrees_with_jiff,
    );
}

#[test]
fn test_zoned_datetimes_reach_gaps_and_both_folds() {
    let bounds = |g: gs::ZonedDateTimeGenerator| g.min_value(JIFF_MIN).max_value(JIFF_MAX);
    find_any(bounds(gs::zoned_datetimes()), |z| z.kind == Kind::Gap);
    find_any(bounds(gs::zoned_datetimes()), |z| {
        z.kind == Kind::Fold && !z.fold
    });
    find_any(bounds(gs::zoned_datetimes()), |z| {
        z.kind == Kind::Fold && z.fold
    });
}

#[test]
fn test_zoned_datetimes_time_zones() {
    assert_all_examples(
        gs::zoned_datetimes()
            .time_zones(gs::time_zones().names(["Europe/London"]))
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        |z| z.zone.name == "Europe/London" && zoned_agrees_with_jiff(z),
    );
    assert_all_examples(
        gs::zoned_datetimes().time_zones(gs::time_zones().names([UNKNOWN_ZONE])),
        |z| {
            z.zone.name == UNKNOWN_ZONE
                && z.offset == UtcOffset::Unknown
                && z.kind == Kind::Unique
                && resolve(UNKNOWN_ZONE, z.civil).unwrap().timestamp == z.timestamp
        },
    );
}

#[test]
fn test_zoned_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 0, 0, 0);
    assert_all_examples(gs::zoned_datetimes().min_value(min), move |z| {
        z.civil >= min
    });
}

#[test]
fn test_zoned_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples(gs::zoned_datetimes().max_value(max), move |z| {
        z.civil <= max
    });
}

#[test]
fn test_zoned_datetimes_min_instant() {
    let min = Timestamp::new(0, 0).unwrap();
    assert_all_examples(gs::zoned_datetimes().min_instant(min), move |z| {
        z.timestamp >= min
    });
}

#[test]
fn test_zoned_datetimes_max_instant() {
    let max = Timestamp::new(0, 0).unwrap();
    assert_all_examples(gs::zoned_datetimes().max_instant(max), move |z| {
        z.timestamp <= max
    });
}

#[test]
fn test_zoned_datetimes_instant_bounds_replace_civil_bounds_and_back() {
    let min = Timestamp::new(0, 0).unwrap();
    let max = Timestamp::new(86_400, 0).unwrap();
    assert_all_examples(
        gs::zoned_datetimes()
            .min_value(datetime(1999, 1, 1, 0, 0, 0))
            .max_instant(max)
            .min_instant(min)
            .min_value(datetime(2000, 1, 1, 0, 0, 0))
            .max_value(datetime(2000, 12, 31, 0, 0, 0)),
        |z| z.civil.date.year == 2000,
    );
    assert_all_examples(
        gs::zoned_datetimes()
            .min_instant(min)
            .max_value(datetime(1999, 12, 31, 0, 0, 0)),
        |z| z.civil.date.year <= 1999,
    );
}

#[test]
fn test_zoned_datetimes_precision() {
    assert_all_examples(gs::zoned_datetimes().precision(Precision::Minutes), |z| {
        z.civil.time.second == 0 && z.civil.time.nanosecond == 0
    });
}

#[test]
fn test_zoned_datetimes_imaginary() {
    assert_all_examples(
        gs::zoned_datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX)
            .imaginary(Imaginary::Avoid),
        |z| z.kind != Kind::Gap && zoned_agrees_with_jiff(z),
    );
}

#[test]
fn test_zoned_datetimes_ambiguous() {
    let earlier = gs::zoned_datetimes()
        .min_value(JIFF_MIN)
        .max_value(JIFF_MAX)
        .ambiguous(Ambiguous::Earlier);
    assert_all_examples(earlier, |z| !z.fold && zoned_agrees_with_jiff(z));
    let later = gs::zoned_datetimes()
        .min_value(JIFF_MIN)
        .max_value(JIFF_MAX)
        .ambiguous(Ambiguous::Later);
    assert_all_examples(later, |z| {
        z.fold == (z.kind == Kind::Fold) && zoned_agrees_with_jiff(z)
    });
}

#[test]
fn test_zoned_datetimes_in_vec() {
    assert_all_examples(gs::vecs(gs::zoned_datetimes()).max_size(3), |v| {
        v.iter()
            .all(|z| resolve(&z.zone.name, z.civil).unwrap().kind == z.kind)
    });
}

#[test]
fn test_zoned_datetimes_shrink_to_utc_at_the_millennium() {
    let z = minimal(gs::zoned_datetimes(), |_| true);
    assert_eq!(z.to_string(), "2000-01-01T00:00:00+00:00[UTC]");
}

#[test]
fn test_zoned_datetimes_shrink_past_a_fold_at_the_lower_bound() {
    let new_york = || gs::time_zones().names(["America/New_York"]);
    let in_fold = datetime(2024, 11, 3, 1, 30, 0);
    let both = minimal(
        gs::zoned_datetimes()
            .time_zones(new_york())
            .min_value(in_fold),
        |_| true,
    );
    assert_eq!(
        both.to_string(),
        "2024-11-03T02:00:00-05:00[America/New_York]"
    );
    let repeated = minimal(
        gs::zoned_datetimes()
            .time_zones(new_york())
            .min_value(in_fold),
        |z| z.kind == Kind::Fold,
    );
    assert_eq!(
        repeated.to_string(),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
    );
    let earlier = minimal(
        gs::zoned_datetimes()
            .time_zones(new_york())
            .min_value(in_fold)
            .ambiguous(Ambiguous::Earlier),
        |_| true,
    );
    assert_eq!(
        earlier.to_string(),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
    );
}

#[test]
fn test_zoned_datetimes_default_generator() {
    check_can_generate_examples(gs::default::<ZonedDateTime>());
}

#[hegel::test]
fn test_zoned_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::datetimes().min_value(JIFF_MIN).max_value(JIFF_MAX));
    let hi = tc.draw(gs::datetimes().min_value(lo).max_value(JIFF_MAX));
    let z = tc.draw(gs::zoned_datetimes().min_value(lo).max_value(hi));
    assert!(z.civil >= lo && z.civil <= hi);
    assert!(zoned_agrees_with_jiff(&z));
}

#[hegel::test]
fn test_offset_at_agrees_with_jiff(tc: hegel::TestCase) {
    let zone = tc.draw(gs::time_zones());
    let min = Timestamp::new(-377_700_000_000, 0).unwrap();
    let max = Timestamp::new(253_402_207_200, 0).unwrap();
    let t = tc.draw(gs::timestamps().min_value(min).max_value(max));
    let expected = jiff::tz::TimeZone::get(&zone.name)
        .unwrap()
        .to_offset(jiff_timestamp(t));
    assert_eq!(
        offset_at(&zone.name, t).unwrap().offset,
        UtcOffset::Known(expected.seconds())
    );
}

#[test]
fn test_signed_durations_default() {
    assert_all_examples(gs::signed_durations(), |d| {
        SignedDuration::new(d.seconds, d.nanoseconds).is_ok()
    });
}

#[test]
fn test_signed_durations_min_value() {
    let min = SignedDuration::new(0, 0).unwrap();
    assert_all_examples(gs::signed_durations().min_value(min), move |d| *d >= min);
}

#[test]
fn test_signed_durations_max_value() {
    let max = SignedDuration::new(0, 0).unwrap();
    assert_all_examples(gs::signed_durations().max_value(max), move |d| *d <= max);
}

#[test]
fn test_signed_durations_precision() {
    assert_all_examples(gs::signed_durations().precision(Precision::Seconds), |d| {
        d.nanoseconds == 0
    });
}

#[test]
fn test_signed_durations_in_vec() {
    let max = SignedDuration::new(0, 0).unwrap();
    assert_all_examples(
        gs::vecs(gs::signed_durations().max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d <= max),
    );
}

#[test]
fn test_signed_durations_shrink_to_zero() {
    assert_eq!(
        minimal(gs::signed_durations(), |_| true),
        SignedDuration::new(0, 0).unwrap()
    );
}

#[test]
fn test_signed_durations_default_generator() {
    check_can_generate_examples(gs::default::<SignedDuration>());
}

#[hegel::test]
fn test_signed_durations_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::signed_durations());
    let hi = tc.draw(gs::signed_durations().min_value(lo));
    let d = tc.draw(gs::signed_durations().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
    assert_eq!(
        d.to_string(),
        jiff::SignedDuration::new(d.seconds, d.nanoseconds).to_string()
    );
}

fn uniform_sign(s: &Span) -> bool {
    let values = SpanUnit::ALL.map(|unit| s.get(unit));
    values.iter().all(|v| *v >= 0) || values.iter().all(|v| *v <= 0)
}

#[test]
fn test_spans_default() {
    assert_all_examples(gs::spans(), uniform_sign);
    find_any(gs::spans(), |s| s.years < 0);
    find_any(gs::spans(), |s| s.nanoseconds > 0);
}

#[test]
fn test_spans_unit_bounds() {
    assert_all_examples(
        gs::spans()
            .unit_bounds(SpanUnit::Days, 0, 30)
            .unit_bounds(SpanUnit::Hours, -23, 23),
        |s| {
            (0..=30).contains(&s.days)
                && (-23..=23).contains(&s.hours)
                && SpanUnit::ALL
                    .into_iter()
                    .filter(|u| *u != SpanUnit::Days && *u != SpanUnit::Hours)
                    .all(|u| s.get(u) == 0)
        },
    );
}

#[test]
fn test_spans_sign() {
    assert_all_examples(gs::spans().sign(SpanSign::NonNegative), |s| {
        SpanUnit::ALL.iter().all(|u| s.get(*u) >= 0)
    });
    find_any(gs::spans().sign(SpanSign::Mixed), |s| !uniform_sign(s));
    assert_all_examples(
        gs::spans()
            .unit_bounds(SpanUnit::Days, -30, 30)
            .sign(SpanSign::Uniform),
        uniform_sign,
    );
}

#[test]
fn test_spans_in_vec() {
    assert_all_examples(
        gs::vecs(gs::spans().unit_bounds(SpanUnit::Days, 0, 30)).max_size(5),
        |v| v.iter().all(|s| (0..=30).contains(&s.days)),
    );
}

#[test]
fn test_spans_shrink_to_zero() {
    assert_eq!(minimal(gs::spans(), |_| true), Span::default());
}

#[test]
fn test_spans_default_generator() {
    check_can_generate_examples(gs::default::<Span>());
}

#[hegel::test]
fn test_spans_property(tc: hegel::TestCase) {
    let unit = tc.draw(gs::sampled_from(SpanUnit::ALL.to_vec()));
    let lo = tc.draw(gs::integers::<i64>().min_value(-1000).max_value(1000));
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(1000));
    let sign = tc.draw(gs::sampled_from(vec![
        SpanSign::Uniform,
        SpanSign::Mixed,
        SpanSign::NonNegative,
    ]));
    tc.assume(sign != SpanSign::NonNegative || lo >= 0);
    let s = tc.draw(gs::spans().unit_bounds(unit, lo, hi).sign(sign));
    assert!((lo..=hi).contains(&s.get(unit)));
}
