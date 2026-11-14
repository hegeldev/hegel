mod common;

use common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::Generator;
use hegel::generators as gs;
use hegel::generators::temporal::{
    Ambiguous, Date, DateTime, Granularity, Instant, MAX_INSTANT, MAX_OFFSET, MAX_YEAR,
    MIN_INSTANT, Month, Precision, SignedDuration, Span, SpanSign, SpanUnit, Tag, Time, TimeZone,
    TransitionKind, UNKNOWN_ZONE, UtcOffset, Weekday, ZonedDateTime, info, offset_at, resolve,
    tzdb_version,
};
use jiff::tz::AmbiguousOffset;

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::new(year, month, day)
}

fn time(hour: u8, minute: u8, second: u8, nanosecond: u32) -> Time {
    Time::new(hour, minute, second, nanosecond)
}

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> DateTime {
    DateTime::new(date(year, month, day), time(hour, minute, second, 0))
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

fn jiff_timestamp(t: Instant) -> jiff::Timestamp {
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

fn is_valid_datetime(dt: DateTime) -> bool {
    resolve("UTC", dt).is_ok()
}

fn is_valid_date(d: Date) -> bool {
    is_valid_datetime(DateTime::new(d, time(0, 0, 0, 0)))
}

fn is_valid_time(t: Time) -> bool {
    is_valid_datetime(DateTime::new(date(2000, 1, 1), t))
}

fn is_valid_instant(t: Instant) -> bool {
    offset_at("UTC", t).is_ok()
}

fn is_valid_duration(d: SignedDuration) -> bool {
    d.nanoseconds.unsigned_abs() < 1_000_000_000
        && (d.seconds == 0 || d.nanoseconds == 0 || (d.seconds > 0) == (d.nanoseconds > 0))
}

#[test]
fn test_value_constructors_build_plain_data() {
    const DATE: Date = Date::new(2023, 2, 29);
    const TIME: Time = Time::new(24, 0, 60, 1_000_000_000);
    const DATETIME: DateTime = DateTime::new(DATE, TIME);
    const INSTANT: Instant = Instant::new(MAX_INSTANT + 1, 1_000_000_000);
    const DURATION: SignedDuration = SignedDuration::new(1, -500_000_000);
    assert_eq!(
        DATE,
        Date {
            year: 2023,
            month: 2,
            day: 29
        }
    );
    assert_eq!(
        TIME,
        Time {
            hour: 24,
            minute: 0,
            second: 60,
            nanosecond: 1_000_000_000
        }
    );
    assert_eq!(
        DATETIME,
        DateTime {
            date: DATE,
            time: TIME
        }
    );
    assert_eq!(
        INSTANT,
        Instant {
            second: MAX_INSTANT + 1,
            nanosecond: 1_000_000_000
        }
    );
    assert_eq!(
        DURATION,
        SignedDuration {
            seconds: 1,
            nanoseconds: -500_000_000
        }
    );
    assert!(!is_valid_date(DATE));
    assert!(!is_valid_time(TIME));
    assert!(!is_valid_instant(INSTANT));
    assert!(!is_valid_duration(DURATION));
}

#[test]
fn test_values_that_are_not_real_print_their_fields() {
    assert_eq!(date(2023, 2, 30).to_string(), "2023-02-30");
    assert_eq!(date(2024, 13, 0).to_string(), "2024-13-00");
    assert_eq!(time(24, 60, 61, 0).to_string(), "24:60:61");
    assert_eq!(
        DateTime::new(date(2023, 2, 30), time(25, 0, 0, 0)).to_string(),
        "2023-02-30T25:00:00"
    );
    assert_eq!(UtcOffset::Known(MAX_OFFSET + 1).to_string(), "+26:00");
}

#[test]
fn test_time_zone_new_resolves_links() {
    let zone = TimeZone::new("Asia/Calcutta").unwrap();
    assert_eq!(zone.name, "Asia/Calcutta");
    assert_eq!(zone.primary_name, "Asia/Kolkata");
    assert_eq!(zone.to_string(), "Asia/Calcutta");
    let err = TimeZone::new("Mars/Olympus").unwrap_err();
    assert!(err.to_string().contains("Mars/Olympus"));
    assert!(std::error::Error::source(&err).is_none());
    assert!(TimeZone::new("asia/kolkata").is_err());
    assert_eq!(
        gs::temporal::default_zone_source()
            .zone("Asia/Calcutta")
            .unwrap(),
        zone
    );
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
fn test_display_instants() {
    assert_eq!(
        Instant::new(946_684_800, 0).to_string(),
        "2000-01-01T00:00:00Z"
    );
    assert_eq!(Instant::new(0, 0).to_string(), "1970-01-01T00:00:00Z");
    assert_eq!(Instant::new(-1, 0).to_string(), "1969-12-31T23:59:59Z");
    assert_eq!(
        Instant::new(-2, 500_000_000).to_string(),
        "1969-12-31T23:59:58.5Z"
    );
    assert_eq!(
        Instant::new(MIN_INSTANT, 0).to_string(),
        "-999999-01-01T00:00:00Z"
    );
    assert_eq!(
        Instant::new(MAX_INSTANT, 0).to_string(),
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
fn test_display_durations() {
    assert_eq!(SignedDuration::new(0, 0).to_string(), "PT0S");
    assert_eq!(
        SignedDuration::new(5405, 500_000_000).to_string(),
        "PT1H30M5.5S"
    );
    assert_eq!(SignedDuration::new(3600, 0).to_string(), "PT1H");
    assert_eq!(SignedDuration::new(-1, -500_000_000).to_string(), "-PT1.5S");
    assert_eq!(SignedDuration::new(0, -1).to_string(), "-PT0.000000001S");
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
    assert_eq!(mixed.to_string(), "+1y -2mo");
    for (index, unit) in SpanUnit::ALL.iter().enumerate() {
        assert_eq!(every.get(*unit), index as i64 + 1);
    }
}

#[test]
fn test_display_zoned_datetimes_matches_the_engine_form() {
    let gap = resolve("America/New_York", datetime(2024, 3, 10, 2, 30, 0)).unwrap();
    assert_eq!(gap.kind, TransitionKind::Gap);
    assert_eq!(gap.offset_before, UtcOffset::Known(-18_000));
    assert_eq!(gap.offset_after, UtcOffset::Known(-14_400));
    assert_eq!(gap.offset, gap.offset_after);
    assert_eq!(
        gap.to_string(),
        "2024-03-10T02:30:00-04:00[America/New_York][gap]"
    );
    assert_eq!(gap.instant.to_string(), "2024-03-10T07:30:00Z");

    let fold = resolve("America/New_York", datetime(2024, 11, 3, 1, 30, 0)).unwrap();
    assert_eq!(fold.kind, TransitionKind::Fold);
    assert!(!fold.fold);
    assert_eq!(
        fold.to_string(),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
    );
    let later = ZonedDateTime {
        fold: true,
        offset: fold.offset_after,
        ..fold.clone()
    };
    assert_eq!(
        later.to_string(),
        "2024-11-03T01:30:00-05:00[America/New_York][fold=1]"
    );

    let unique = resolve("America/New_York", datetime(2024, 6, 1, 12, 0, 0)).unwrap();
    assert_eq!(unique.kind, TransitionKind::Unique);
    assert_eq!(unique.offset, UtcOffset::Known(-14_400));
    assert_eq!(
        unique.to_string(),
        "2024-06-01T12:00:00-04:00[America/New_York]"
    );

    let unknown = resolve(UNKNOWN_ZONE, datetime(2024, 6, 1, 12, 0, 0)).unwrap();
    assert_eq!(unknown.offset, UtcOffset::Unknown);
    assert_eq!(unknown.to_string(), "2024-06-01T12:00:00Z[Etc/Unknown]");
    assert_eq!(unknown.instant.to_string(), "2024-06-01T12:00:00Z");

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
        resolve("UTC", last).unwrap().instant,
        Instant::new(MAX_INSTANT, 0)
    );
}

#[test]
fn test_offset_at_reports_the_offset_in_force() {
    let summer = offset_at("Europe/London", Instant::new(1_717_243_200, 0)).unwrap();
    assert_eq!(summer.offset, UtcOffset::Known(3600));
    assert!(summer.is_dst);
    assert_eq!(summer.abbreviation, "BST");
    let winter = offset_at("Europe/London", Instant::new(1_704_067_200, 0)).unwrap();
    assert_eq!(winter.offset, UtcOffset::Known(0));
    assert!(!winter.is_dst);
    assert_eq!(winter.abbreviation, "GMT");
    let unknown = offset_at(UNKNOWN_ZONE, Instant::new(0, 0)).unwrap();
    assert_eq!(unknown.offset, UtcOffset::Unknown);
    assert_eq!(unknown.abbreviation, "-00");
    assert!(offset_at("Mars/Olympus", Instant::new(0, 0)).is_err());
    let out_of_range = Instant {
        second: 0,
        nanosecond: 1_000_000_000,
    };
    assert!(offset_at("UTC", out_of_range).is_err());
}

#[test]
fn test_info_describes_names() {
    let utc = info("UTC").unwrap().unwrap();
    assert_eq!(utc.primary_name, "UTC");
    assert!(!utc.is_link);
    assert_eq!(utc.rank, 0);
    assert!(utc.tags.contains(&Tag::Fixed));
    let calcutta = info("Asia/Calcutta").unwrap().unwrap();
    assert_eq!(calcutta.primary_name, "Asia/Kolkata");
    assert!(calcutta.is_link);
    assert!(calcutta.tags.contains(&Tag::Link));
    assert!(calcutta.tags.contains(&Tag::Fractional));
    let dublin = info("Europe/Dublin").unwrap().unwrap();
    assert!(dublin.tags.contains(&Tag::Dst));
    let unknown = info(UNKNOWN_ZONE).unwrap().unwrap();
    assert_eq!(unknown.primary_name, UNKNOWN_ZONE);
    assert_eq!(unknown.tags, vec![Tag::Undefined]);
    assert_eq!(unknown.rank, u16::MAX);
    assert_eq!(info("Mars/Olympus").unwrap(), None);
    assert_eq!(info("UTC\0").unwrap(), None);
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
    assert_all_examples(gs::temporal::dates(), |d| is_valid_date(*d));
}

#[test]
fn test_dates_min_value() {
    let min = date(2024, 1, 1);
    assert_all_examples(gs::temporal::dates().min_value(min), move |d| *d >= min);
}

#[test]
fn test_dates_max_value() {
    let max = date(1999, 12, 31);
    assert_all_examples(gs::temporal::dates().max_value(max), move |d| *d <= max);
}

#[test]
fn test_dates_weekdays() {
    assert_all_examples(
        gs::temporal::dates()
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
        gs::temporal::dates().months([Month::February, Month::December]),
        |d| d.month == 2 || d.month == 12,
    );
}

#[test]
fn test_dates_in_vec() {
    let max = date(1999, 12, 31);
    assert_all_examples(
        gs::vecs(gs::temporal::dates().max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d <= max),
    );
}

#[test]
fn test_dates_shrink_toward_the_millennium() {
    assert_eq!(minimal(gs::temporal::dates(), |_| true), date(2000, 1, 1));
}

#[test]
fn test_dates_default_generator() {
    check_can_generate_examples(gs::default::<Date>());
}

#[hegel::test]
fn test_dates_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::temporal::dates());
    let hi = tc.draw(gs::temporal::dates().min_value(lo));
    let d = tc.draw(gs::temporal::dates().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
}

#[test]
fn test_times_default() {
    assert_all_examples(gs::temporal::times(), |t| is_valid_time(*t));
}

#[test]
fn test_times_min_value() {
    let min = time(12, 0, 0, 0);
    assert_all_examples(gs::temporal::times().min_value(min), move |t| *t >= min);
}

#[test]
fn test_times_max_value() {
    let max = time(12, 0, 0, 0);
    assert_all_examples(gs::temporal::times().max_value(max), move |t| *t <= max);
}

#[test]
fn test_times_precision() {
    assert_all_examples(gs::temporal::times().precision(Precision::Minutes), |t| {
        t.second == 0 && t.nanosecond == 0
    });
    assert_all_examples(
        gs::temporal::times().precision(Precision::Microseconds),
        |t| t.nanosecond % 1_000 == 0,
    );
    assert_all_examples(
        gs::temporal::times().precision(Precision::Milliseconds),
        |t| t.nanosecond % 1_000_000 == 0,
    );
}

#[test]
fn test_times_in_vec() {
    let max = time(12, 0, 0, 0);
    assert_all_examples(
        gs::vecs(gs::temporal::times().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_times_shrink_toward_midnight() {
    assert_eq!(minimal(gs::temporal::times(), |_| true), time(0, 0, 0, 0));
    assert_eq!(
        minimal(gs::temporal::times().min_value(time(6, 30, 0, 0)), |_| true),
        time(6, 30, 0, 0)
    );
}

#[test]
fn test_times_default_generator() {
    check_can_generate_examples(gs::default::<Time>());
}

#[hegel::test]
fn test_times_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::temporal::times());
    let hi = tc.draw(gs::temporal::times().min_value(lo));
    let t = tc.draw(gs::temporal::times().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[test]
fn test_datetimes_default() {
    assert_all_examples(gs::temporal::datetimes(), |dt| is_valid_datetime(*dt));
}

#[test]
fn test_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 12, 0, 0);
    assert_all_examples(gs::temporal::datetimes().min_value(min), move |dt| {
        *dt >= min
    });
}

#[test]
fn test_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(gs::temporal::datetimes().max_value(max), move |dt| {
        *dt <= max
    });
}

#[test]
fn test_datetimes_precision() {
    assert_all_examples(
        gs::temporal::datetimes().precision(Precision::Hours),
        |dt| dt.time.minute == 0 && dt.time.second == 0 && dt.time.nanosecond == 0,
    );
}

#[test]
fn test_datetimes_in_vec() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(
        gs::vecs(gs::temporal::datetimes().max_value(max)).max_size(5),
        move |v| v.iter().all(|dt| *dt <= max),
    );
}

#[test]
fn test_datetimes_shrink_toward_the_millennium() {
    assert_eq!(
        minimal(gs::temporal::datetimes(), |_| true),
        datetime(2000, 1, 1, 0, 0, 0)
    );
}

#[test]
fn test_datetimes_default_generator() {
    check_can_generate_examples(gs::default::<DateTime>());
}

#[hegel::test]
fn test_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::temporal::datetimes());
    let hi = tc.draw(gs::temporal::datetimes().min_value(lo));
    let dt = tc.draw(gs::temporal::datetimes().min_value(lo).max_value(hi));
    assert!(dt >= lo && dt <= hi);
}

#[test]
fn test_instants_default() {
    assert_all_examples(gs::temporal::instants(), |t| is_valid_instant(*t));
}

#[test]
fn test_instants_min_value() {
    let min = Instant::new(0, 0);
    assert_all_examples(gs::temporal::instants().min_value(min), move |t| *t >= min);
}

#[test]
fn test_instants_max_value() {
    let max = Instant::new(0, 0);
    assert_all_examples(gs::temporal::instants().max_value(max), move |t| *t <= max);
}

#[test]
fn test_instants_precision() {
    assert_all_examples(
        gs::temporal::instants().precision(Precision::Seconds),
        |t| t.nanosecond == 0,
    );
}

#[test]
fn test_instants_in_vec() {
    let max = Instant::new(0, 0);
    assert_all_examples(
        gs::vecs(gs::temporal::instants().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_instants_shrink_toward_the_millennium() {
    assert_eq!(
        minimal(gs::temporal::instants(), |_| true),
        Instant::new(946_684_800, 0)
    );
}

#[test]
fn test_instants_default_generator() {
    check_can_generate_examples(gs::default::<Instant>());
}

#[hegel::test]
fn test_instants_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::temporal::instants());
    let hi = tc.draw(gs::temporal::instants().min_value(lo));
    let t = tc.draw(gs::temporal::instants().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[hegel::test]
fn test_instants_and_dates_print_as_jiff_does(tc: hegel::TestCase) {
    let min = Instant::new(-377_700_000_000, 0);
    let max = Instant::new(253_402_207_200, 0);
    let t = tc.draw(gs::temporal::instants().min_value(min).max_value(max));
    assert_eq!(t.to_string(), jiff_timestamp(t).to_string());
    let d = tc.draw(
        gs::temporal::dates()
            .min_value(JIFF_MIN.date)
            .max_value(JIFF_MAX.date),
    );
    assert_eq!(d.to_string(), jiff_date(d).to_string());
    let dt = tc.draw(
        gs::temporal::datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
    );
    assert_eq!(dt.to_string(), jiff_datetime(dt).to_string());
}

#[test]
fn test_offsets_default() {
    assert_all_examples(
        gs::temporal::offsets(),
        |o| matches!(o, UtcOffset::Known(s) if s.unsigned_abs() <= MAX_OFFSET.unsigned_abs()),
    );
}

#[test]
fn test_offsets_min_value() {
    assert_all_examples(
        gs::temporal::offsets().min_value(UtcOffset::Known(0)),
        |o| matches!(o, UtcOffset::Known(s) if *s >= 0),
    );
}

#[test]
fn test_offsets_max_value() {
    assert_all_examples(
        gs::temporal::offsets().max_value(UtcOffset::Known(0)),
        |o| matches!(o, UtcOffset::Known(s) if *s <= 0),
    );
}

#[test]
fn test_offsets_granularity() {
    assert_all_examples(
        gs::temporal::offsets().granularity(Granularity::Hours),
        |o| matches!(o, UtcOffset::Known(s) if s % 3600 == 0),
    );
    assert_all_examples(
        gs::temporal::offsets().granularity(Granularity::QuarterHours),
        |o| matches!(o, UtcOffset::Known(s) if s % 900 == 0),
    );
    assert_all_examples(
        gs::temporal::offsets().granularity(Granularity::Minutes),
        |o| matches!(o, UtcOffset::Known(s) if s % 60 == 0),
    );
}

#[test]
fn test_offsets_unknown() {
    find_any(gs::temporal::offsets().allow_unknown(true), |o| {
        *o == UtcOffset::Unknown
    });
}

#[test]
fn test_offsets_unknown_shrinks_to_a_known_offset() {
    assert_eq!(
        minimal(gs::temporal::offsets().allow_unknown(true), |_| true),
        UtcOffset::Known(0)
    );
    assert_eq!(
        minimal(gs::temporal::offsets().allow_unknown(true), |o| {
            *o == UtcOffset::Unknown || matches!(o, UtcOffset::Known(s) if s.unsigned_abs() >= 3600)
        }),
        UtcOffset::Known(3600)
    );
    assert_eq!(
        minimal(gs::temporal::offsets().allow_unknown(true), |o| *o
            == UtcOffset::Unknown),
        UtcOffset::Unknown
    );
}

#[test]
fn test_offsets_in_vec() {
    assert_all_examples(
        gs::vecs(gs::temporal::offsets().max_value(UtcOffset::Known(0))).max_size(5),
        |v| {
            v.iter()
                .all(|o| matches!(o, UtcOffset::Known(s) if *s <= 0))
        },
    );
}

#[test]
fn test_offsets_default_generator() {
    check_can_generate_examples(gs::default::<UtcOffset>());
}

#[hegel::test]
fn test_offsets_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i32>()
            .min_value(-MAX_OFFSET)
            .max_value(MAX_OFFSET),
    );
    let hi = tc.draw(gs::integers::<i32>().min_value(lo).max_value(MAX_OFFSET));
    let (granularity, unit) = tc.draw(gs::sampled_from(vec![
        (Granularity::Hours, 3600),
        (Granularity::QuarterHours, 900),
        (Granularity::Minutes, 60),
        (Granularity::Seconds, 1),
    ]));
    tc.assume(-(-lo).div_euclid(unit) <= hi.div_euclid(unit));
    let o = tc.draw(
        gs::temporal::offsets()
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
fn test_zones_default() {
    assert_all_examples(gs::temporal::zones(), |zone| {
        let info = info(&zone.name).unwrap().unwrap();
        info.primary_name == zone.primary_name
            && !info.tags.contains(&Tag::Legacy)
            && zone.name != UNKNOWN_ZONE
            && jiff::tz::TimeZone::get(&zone.name).is_ok()
    });
}

#[test]
fn test_zones_names() {
    let names = ["Europe/London", "Asia/Calcutta"];
    assert_all_examples(gs::temporal::zones().names(names), move |zone| {
        names.contains(&zone.name.as_str())
    });
    find_any(gs::temporal::zones().names(names), |zone| {
        zone.primary_name == "Asia/Kolkata"
    });
}

#[test]
fn test_zones_links() {
    assert_all_examples(gs::temporal::zones().allow_links(false), |zone| {
        !info(&zone.name).unwrap().unwrap().is_link
    });
    find_any(gs::temporal::zones(), |zone| {
        info(&zone.name).unwrap().unwrap().is_link
    });
}

#[test]
fn test_zones_legacy() {
    find_any(gs::temporal::zones().allow_legacy(true), |zone| {
        info(&zone.name)
            .unwrap()
            .unwrap()
            .tags
            .contains(&Tag::Legacy)
    });
}

#[test]
fn test_zones_unknown() {
    find_any(gs::temporal::zones().allow_unknown(true), |zone| {
        zone.name == UNKNOWN_ZONE
    });
}

#[test]
fn test_zones_opaque() {
    assert_all_examples(
        gs::temporal::zones()
            .names(["Mars/Olympus"])
            .allow_opaque(true),
        |zone| zone.name == "Mars/Olympus" && zone.primary_name == "Mars/Olympus",
    );
}

#[test]
fn test_zones_shrink_to_utc() {
    assert_eq!(minimal(gs::temporal::zones(), |_| true).name, "UTC");
}

#[test]
fn test_zones_in_vec() {
    assert_all_examples(gs::vecs(gs::temporal::zones()).max_size(5), |v| {
        v.iter().all(|zone| info(&zone.name).unwrap().is_some())
    });
}

#[test]
fn test_zones_default_generator() {
    check_can_generate_examples(gs::default::<TimeZone>());
}

fn zoned_agrees_with_jiff(z: &ZonedDateTime) -> bool {
    let tz = jiff::tz::TimeZone::get(&z.zone.name).unwrap();
    let ambiguous = tz.to_ambiguous_timestamp(jiff_datetime(z.civil));
    let (kind, before, after) = match ambiguous.offset() {
        AmbiguousOffset::Unambiguous { offset } => (TransitionKind::Unique, offset, offset),
        AmbiguousOffset::Gap { before, after } => (TransitionKind::Gap, before, after),
        AmbiguousOffset::Fold { before, after } => (TransitionKind::Fold, before, after),
    };
    let expected = match (kind, z.fold) {
        (TransitionKind::Fold, true) => ambiguous.later().unwrap(),
        _ => ambiguous.compatible().unwrap(),
    };
    let resolved = match kind {
        TransitionKind::Gap => after,
        TransitionKind::Fold if z.fold => after,
        _ => before,
    };
    kind == z.kind
        && UtcOffset::Known(before.seconds()) == z.offset_before
        && UtcOffset::Known(after.seconds()) == z.offset_after
        && z.offset == UtcOffset::Known(resolved.seconds())
        && expected == jiff_timestamp(z.instant)
        && (z.kind == TransitionKind::Fold || !z.fold)
}

#[test]
fn test_zoned_datetimes_default() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        zoned_agrees_with_jiff,
    );
}

#[test]
fn test_zoned_datetimes_reach_gaps_and_both_folds() {
    let bounds =
        |g: gs::temporal::ZonedDateTimeGenerator| g.min_value(JIFF_MIN).max_value(JIFF_MAX);
    find_any(bounds(gs::temporal::zoned_datetimes()), |z| {
        z.kind == TransitionKind::Gap
    });
    find_any(bounds(gs::temporal::zoned_datetimes()), |z| {
        z.kind == TransitionKind::Fold && !z.fold
    });
    find_any(bounds(gs::temporal::zoned_datetimes()), |z| {
        z.kind == TransitionKind::Fold && z.fold
    });
}

#[test]
fn test_zoned_datetimes_time_zones() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().names(["Europe/London"]))
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        |z| z.zone.name == "Europe/London" && zoned_agrees_with_jiff(z),
    );
    assert_all_examples(
        gs::temporal::zoned_datetimes().zones(gs::temporal::zones().names([UNKNOWN_ZONE])),
        |z| {
            z.zone.name == UNKNOWN_ZONE
                && z.offset == UtcOffset::Unknown
                && z.kind == TransitionKind::Unique
                && resolve(UNKNOWN_ZONE, z.civil).unwrap().instant == z.instant
        },
    );
}

#[test]
fn test_zoned_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 0, 0, 0);
    assert_all_examples(gs::temporal::zoned_datetimes().min_value(min), move |z| {
        z.civil >= min
    });
}

#[test]
fn test_zoned_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 0, 0, 0);
    assert_all_examples(gs::temporal::zoned_datetimes().max_value(max), move |z| {
        z.civil <= max
    });
}

#[test]
fn test_zoned_datetimes_min_instant() {
    let min = Instant::new(0, 0);
    assert_all_examples(gs::temporal::zoned_datetimes().min_instant(min), move |z| {
        z.instant >= min
    });
}

#[test]
fn test_zoned_datetimes_max_instant() {
    let max = Instant::new(0, 0);
    assert_all_examples(gs::temporal::zoned_datetimes().max_instant(max), move |z| {
        z.instant <= max
    });
}

#[test]
fn test_zoned_datetimes_precision() {
    assert_all_examples(
        gs::temporal::zoned_datetimes().precision(Precision::Minutes),
        |z| z.civil.time.second == 0 && z.civil.time.nanosecond == 0,
    );
}

#[test]
fn test_zoned_datetimes_imaginary() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX)
            .allow_imaginary(false),
        |z| z.kind != TransitionKind::Gap && zoned_agrees_with_jiff(z),
    );
}

#[test]
fn test_zoned_datetimes_ambiguous() {
    let earlier = gs::temporal::zoned_datetimes()
        .min_value(JIFF_MIN)
        .max_value(JIFF_MAX)
        .ambiguous(Ambiguous::Earlier);
    assert_all_examples(earlier, |z| !z.fold && zoned_agrees_with_jiff(z));
    let later = gs::temporal::zoned_datetimes()
        .min_value(JIFF_MIN)
        .max_value(JIFF_MAX)
        .ambiguous(Ambiguous::Later);
    assert_all_examples(later, |z| {
        z.fold == (z.kind == TransitionKind::Fold) && zoned_agrees_with_jiff(z)
    });
}

#[test]
fn test_zoned_datetimes_in_vec() {
    assert_all_examples(gs::vecs(gs::temporal::zoned_datetimes()).max_size(3), |v| {
        v.iter()
            .all(|z| resolve(&z.zone.name, z.civil).unwrap().kind == z.kind)
    });
}

#[test]
fn test_zoned_datetimes_shrink_to_utc_at_the_millennium() {
    let z = minimal(gs::temporal::zoned_datetimes(), |_| true);
    assert_eq!(z.to_string(), "2000-01-01T00:00:00+00:00[UTC]");
}

#[test]
fn test_zoned_datetimes_shrink_past_a_fold_at_the_lower_bound() {
    let new_york = || gs::temporal::zones().names(["America/New_York"]);
    let in_fold = datetime(2024, 11, 3, 1, 30, 0);
    let both = minimal(
        gs::temporal::zoned_datetimes()
            .zones(new_york())
            .min_value(in_fold),
        |_| true,
    );
    assert_eq!(
        both.to_string(),
        "2024-11-03T02:00:00-05:00[America/New_York]"
    );
    let repeated = minimal(
        gs::temporal::zoned_datetimes()
            .zones(new_york())
            .min_value(in_fold),
        |z| z.kind == TransitionKind::Fold,
    );
    assert_eq!(
        repeated.to_string(),
        "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
    );
    let earlier = minimal(
        gs::temporal::zoned_datetimes()
            .zones(new_york())
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
    let lo = tc.draw(
        gs::temporal::datetimes()
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
    );
    let hi = tc.draw(gs::temporal::datetimes().min_value(lo).max_value(JIFF_MAX));
    let z = tc.draw(gs::temporal::zoned_datetimes().min_value(lo).max_value(hi));
    assert!(z.civil >= lo && z.civil <= hi);
    assert!(zoned_agrees_with_jiff(&z));
}

#[hegel::test]
fn test_offset_at_agrees_with_jiff(tc: hegel::TestCase) {
    let zone = tc.draw(gs::temporal::zones());
    let min = Instant::new(-377_700_000_000, 0);
    let max = Instant::new(253_402_207_200, 0);
    let t = tc.draw(gs::temporal::instants().min_value(min).max_value(max));
    let expected = jiff::tz::TimeZone::get(&zone.name)
        .unwrap()
        .to_offset(jiff_timestamp(t));
    assert_eq!(
        offset_at(&zone.name, t).unwrap().offset,
        UtcOffset::Known(expected.seconds())
    );
}

#[test]
fn test_durations_default() {
    assert_all_examples(gs::temporal::durations(), |d| is_valid_duration(*d));
}

#[test]
fn test_durations_min_value() {
    let min = SignedDuration::new(0, 0);
    assert_all_examples(gs::temporal::durations().min_value(min), move |d| *d >= min);
}

#[test]
fn test_durations_max_value() {
    let max = SignedDuration::new(0, 0);
    assert_all_examples(gs::temporal::durations().max_value(max), move |d| *d <= max);
}

#[test]
fn test_durations_precision() {
    assert_all_examples(
        gs::temporal::durations().precision(Precision::Seconds),
        |d| d.nanoseconds == 0,
    );
}

#[test]
fn test_durations_in_vec() {
    let max = SignedDuration::new(0, 0);
    assert_all_examples(
        gs::vecs(gs::temporal::durations().max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d <= max),
    );
}

#[test]
fn test_durations_shrink_to_zero() {
    assert_eq!(
        minimal(gs::temporal::durations(), |_| true),
        SignedDuration::new(0, 0)
    );
}

#[test]
fn test_durations_default_generator() {
    check_can_generate_examples(gs::default::<SignedDuration>());
}

#[hegel::test]
fn test_durations_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::temporal::durations());
    let hi = tc.draw(gs::temporal::durations().min_value(lo));
    let d = tc.draw(gs::temporal::durations().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
    assert_eq!(
        d.to_string(),
        jiff::SignedDuration::new(d.seconds, d.nanoseconds).to_string()
    );
}

fn uniform_sign(s: &Span) -> bool {
    let values = SpanUnit::ALL.iter().map(|unit| s.get(*unit));
    values.clone().all(|v| v >= 0) || values.clone().all(|v| v <= 0)
}

#[test]
fn test_spans_default() {
    assert_all_examples(gs::temporal::spans(), uniform_sign);
    find_any(gs::temporal::spans(), |s| s.years < 0);
    find_any(gs::temporal::spans(), |s| s.nanoseconds > 0);
}

#[test]
fn test_spans_unit_bounds() {
    assert_all_examples(
        gs::temporal::spans()
            .unit_bounds(SpanUnit::Days, 0, 30)
            .unit_bounds(SpanUnit::Hours, -23, 23),
        |s| {
            (0..=30).contains(&s.days)
                && (-23..=23).contains(&s.hours)
                && SpanUnit::ALL
                    .iter()
                    .filter(|u| **u != SpanUnit::Days && **u != SpanUnit::Hours)
                    .all(|u| s.get(*u) == 0)
        },
    );
}

#[test]
fn test_spans_sign() {
    assert_all_examples(gs::temporal::spans().sign(SpanSign::NonNegative), |s| {
        SpanUnit::ALL.iter().all(|u| s.get(*u) >= 0)
    });
    find_any(gs::temporal::spans().sign(SpanSign::Mixed), |s| {
        !uniform_sign(s)
    });
    assert_all_examples(
        gs::temporal::spans()
            .unit_bounds(SpanUnit::Days, -30, 30)
            .sign(SpanSign::Uniform),
        uniform_sign,
    );
}

#[test]
fn test_spans_in_vec() {
    assert_all_examples(
        gs::vecs(gs::temporal::spans().unit_bounds(SpanUnit::Days, 0, 30)).max_size(5),
        |v| v.iter().all(|s| (0..=30).contains(&s.days)),
    );
}

#[test]
fn test_spans_shrink_to_zero() {
    assert_eq!(minimal(gs::temporal::spans(), |_| true), Span::default());
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
    let s = tc.draw(gs::temporal::spans().unit_bounds(unit, lo, hi).sign(sign));
    assert!((lo..=hi).contains(&s.get(unit)));
}

fn zone(name: &str) -> TimeZone {
    TimeZone::new(name).unwrap()
}

fn in_new_york() -> gs::temporal::ZonedDateTimeGenerator {
    gs::temporal::zoned_datetimes()
        .zones(gs::just(zone("America/New_York")))
        .min_value(JIFF_MIN)
        .max_value(JIFF_MAX)
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator() {
    let names = ["America/New_York", "Europe/London", "Asia/Calcutta"];
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::sampled_from(names.map(zone).to_vec()))
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        move |z| names.contains(&z.zone.name.as_str()) && zoned_agrees_with_jiff(z),
    );
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_reach_every_zone() {
    let names = ["America/New_York", "Europe/London", "Asia/Calcutta"];
    for name in names {
        find_any(
            gs::temporal::zoned_datetimes().zones(gs::sampled_from(names.map(zone).to_vec())),
            move |z| z.zone.name == name,
        );
    }
}

#[test]
fn test_zoned_datetimes_from_a_boxed_zone_generator() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::temporal::zones().names(["Asia/Calcutta"]).boxed()),
        |z| z.zone.name == "Asia/Calcutta",
    );
    assert_all_examples(
        gs::temporal::zoned_datetimes().zones(gs::just(zone("Europe/London")).boxed()),
        |z| z.zone.name == "Europe/London",
    );
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_keep_the_drawn_zone() {
    assert_all_examples(
        gs::temporal::zoned_datetimes().zones(gs::just(zone("Asia/Calcutta"))),
        |z| z.zone.name == "Asia/Calcutta" && z.zone.primary_name == "Asia/Kolkata",
    );
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_reach_gaps_and_both_folds() {
    find_any(in_new_york(), |z| z.kind == TransitionKind::Gap);
    find_any(in_new_york(), |z| z.kind == TransitionKind::Fold && !z.fold);
    find_any(in_new_york(), |z| z.kind == TransitionKind::Fold && z.fold);
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_apply_the_settings() {
    let min = datetime(2024, 1, 1, 0, 0, 0);
    let max = datetime(2024, 12, 31, 0, 0, 0);
    assert_all_examples(
        in_new_york()
            .min_value(min)
            .max_value(max)
            .precision(Precision::Minutes),
        move |z| {
            z.civil >= min
                && z.civil <= max
                && z.civil.time.second == 0
                && z.civil.time.nanosecond == 0
                && zoned_agrees_with_jiff(z)
        },
    );
    let lo = Instant::new(0, 0);
    let hi = Instant::new(86_400, 0);
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(gs::just(zone("America/New_York")))
            .min_instant(lo)
            .max_instant(hi),
        move |z| z.instant >= lo && z.instant <= hi && zoned_agrees_with_jiff(z),
    );
    assert_all_examples(in_new_york().allow_imaginary(false), |z| {
        z.kind != TransitionKind::Gap && zoned_agrees_with_jiff(z)
    });
    assert_all_examples(in_new_york().ambiguous(Ambiguous::Earlier), |z| {
        !z.fold && zoned_agrees_with_jiff(z)
    });
    assert_all_examples(in_new_york().ambiguous(Ambiguous::Later), |z| {
        z.fold == (z.kind == TransitionKind::Fold) && zoned_agrees_with_jiff(z)
    });
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_shrink_in_the_zone() {
    let z = minimal(
        gs::temporal::zoned_datetimes().zones(gs::just(zone("America/New_York"))),
        |_| true,
    );
    assert_eq!(z.to_string(), "2000-01-01T00:00:00-05:00[America/New_York]");
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_with_the_unknown_zone() {
    assert_all_examples(
        gs::temporal::zoned_datetimes().zones(gs::just(zone(UNKNOWN_ZONE))),
        |z| z.zone.name == UNKNOWN_ZONE && z.offset == UtcOffset::Unknown,
    );
}

#[test]
fn test_zoned_datetimes_from_a_mapped_zones_generator() {
    assert_all_examples(
        gs::temporal::zoned_datetimes()
            .zones(
                gs::temporal::zones()
                    .names(["Europe/London"])
                    .map(|zone| zone),
            )
            .min_value(JIFF_MIN)
            .max_value(JIFF_MAX),
        |z| z.zone.name == "Europe/London" && zoned_agrees_with_jiff(z),
    );
}

#[test]
fn test_zoned_datetimes_from_any_zone_generator_in_vec() {
    let zones = gs::sampled_from(vec![zone("UTC"), zone("Australia/Lord_Howe")]);
    assert_all_examples(
        gs::vecs(gs::temporal::zoned_datetimes().zones(zones)).max_size(5),
        |v| {
            v.iter()
                .all(|z| resolve(&z.zone.name, z.civil).unwrap().kind == z.kind)
        },
    );
}
