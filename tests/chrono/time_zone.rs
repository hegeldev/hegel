use crate::common::utils::{
    AssertAllExamples, assert_all_examples, check_can_generate_examples, expect_panic, find_any,
    minimal,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use hegel::HealthCheck;
use hegel::datetime::{Ambiguous, Imaginary, Precision, Tag, info};
use hegel::extras::chrono as chrono_gs;
use hegel::generators as gs;

fn new_york(date: (i32, u32, u32), time: (u32, u32, u32)) -> NaiveDateTime {
    NaiveDateTime::new(
        NaiveDate::from_ymd_opt(date.0, date.1, date.2).unwrap(),
        NaiveTime::from_hms_opt(time.0, time.1, time.2).unwrap(),
    )
}

fn resolves_to_itself(dt: &DateTime<Tz>) -> bool {
    dt.timezone().from_utc_datetime(&dt.naive_utc()) == *dt
}

#[test]
fn test_time_zones_default() {
    assert_all_examples(chrono_gs::time_zones(), |tz| {
        info(tz.name()).is_none_or(|info| !info.tags.contains(&Tag::Legacy))
    });
}

#[test]
fn test_time_zones_names() {
    assert_all_examples(
        chrono_gs::time_zones().names([Tz::Europe__London, Tz::Asia__Tokyo]),
        |tz| *tz == Tz::Europe__London || *tz == Tz::Asia__Tokyo,
    );
}

#[test]
fn test_time_zones_names_reach_every_name() {
    find_any(
        chrono_gs::time_zones().names([Tz::Europe__London, Tz::Asia__Tokyo]),
        |tz| *tz == Tz::Asia__Tokyo,
    );
}

#[test]
fn test_time_zones_links() {
    find_any(chrono_gs::time_zones(), |tz| {
        info(tz.name()).is_some_and(|info| info.is_link)
    });
    assert_all_examples(chrono_gs::time_zones().links(false), |tz| {
        info(tz.name()).is_none_or(|info| !info.is_link)
    });
}

#[test]
fn test_time_zones_legacy() {
    find_any(chrono_gs::time_zones().legacy(true), |tz| {
        info(tz.name()).is_some_and(|info| info.tags.contains(&Tag::Legacy))
    });
}

#[test]
fn test_time_zones_in_vec() {
    assert_all_examples(
        gs::vecs(chrono_gs::time_zones().names([Tz::UTC, Tz::Europe__Paris])).max_size(5),
        |v| {
            v.iter()
                .all(|tz| *tz == Tz::UTC || *tz == Tz::Europe__Paris)
        },
    );
}

#[test]
fn test_time_zone_default_generator() {
    check_can_generate_examples(gs::default::<Tz>());
}

#[test]
fn test_time_zones_shrink_to_utc() {
    let tz: Tz = minimal(chrono_gs::time_zones(), |_| true);
    assert_eq!(tz, Tz::UTC);
}

#[test]
fn test_time_zones_empty_names_is_a_usage_error() {
    expect_panic(
        || {
            hegel::Hegel::new(|tc| {
                let _ = tc.draw(chrono_gs::time_zones().names([]));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "at least one allowed name",
    );
}

#[test]
fn test_datetimes_tz_default() {
    assert_all_examples(
        chrono_gs::datetimes().timezones(chrono_gs::time_zones()),
        resolves_to_itself,
    );
}

#[test]
fn test_datetime_tz_default_generator() {
    assert_all_examples(gs::default::<DateTime<Tz>>(), resolves_to_itself);
}

#[test]
fn test_datetimes_tz_in_vec() {
    assert_all_examples(
        gs::vecs(chrono_gs::datetimes().timezones(chrono_gs::time_zones())).max_size(3),
        |v| v.iter().all(resolves_to_itself),
    );
}

#[test]
fn test_datetimes_tz_wall_clock_bounds() {
    let min = new_york((2020, 1, 1), (0, 0, 0));
    let max = new_york((2024, 12, 31), (23, 59, 59));
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones())
            .min_value(min)
            .max_value(max),
        move |dt| dt.naive_local() >= min && dt.naive_local() <= max,
    );
}

#[test]
fn test_datetimes_tz_instant_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones())
            .min_instant(min)
            .max_instant(max),
        move |dt| *dt >= min && *dt <= max,
    );
}

#[test]
fn test_datetimes_tz_precision() {
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones())
            .precision(Precision::Seconds),
        |dt| dt.nanosecond() == 0,
    );
}

#[test]
fn test_datetimes_tz_avoids_gaps_by_default() {
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 3, 10), (0, 0, 0)))
            .max_value(new_york((2024, 3, 10), (23, 59, 59))),
        |dt| {
            Tz::America__New_York
                .from_local_datetime(&dt.naive_local())
                .single()
                .is_some()
        },
    );
}

#[test]
fn test_datetimes_tz_allows_gaps_as_the_shifted_instant() {
    let max = new_york((2024, 3, 10), (2, 59, 59));
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 3, 10), (2, 0, 0)))
            .max_value(max)
            .imaginary(Imaginary::Allow),
        move |dt| {
            dt.naive_local() > max
                && dt.naive_local() < new_york((2024, 3, 10), (4, 0, 0))
                && dt.offset().fix().local_minus_utc() == -4 * 3600
        },
    );
}

#[test]
fn test_datetimes_tz_draws_both_repeats_of_a_fold() {
    let generator = || {
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
    };
    find_any(generator(), |dt| {
        dt.offset().fix().local_minus_utc() == -4 * 3600
    });
    find_any(generator(), |dt| {
        dt.offset().fix().local_minus_utc() == -5 * 3600
    });
}

#[test]
fn test_datetimes_tz_ambiguous_earlier() {
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
            .ambiguous(Ambiguous::Earlier),
        |dt| dt.offset().fix().local_minus_utc() == -4 * 3600,
    );
}

#[test]
fn test_datetimes_tz_ambiguous_later() {
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
            .min_value(new_york((2024, 11, 3), (1, 0, 0)))
            .max_value(new_york((2024, 11, 3), (1, 59, 59)))
            .ambiguous(Ambiguous::Later),
        |dt| dt.offset().fix().local_minus_utc() == -5 * 3600,
    );
}

#[test]
fn test_datetimes_tz_unreachable_instant_is_rejected() {
    let max = DateTime::<Utc>::MAX_UTC.naive_utc();
    let min = max - chrono::TimeDelta::hours(1);
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(
                    chrono_gs::datetimes()
                        .timezones(chrono_gs::time_zones().names([Tz::America__New_York]))
                        .min_value(min)
                        .max_value(max),
                );
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "Unsatisfiable|FilterTooMuch",
    );
}

#[test]
fn test_datetimes_tz_shrinks_to_2000_at_utc() {
    let dt: DateTime<Tz> = minimal(
        chrono_gs::datetimes().timezones(chrono_gs::time_zones()),
        |_| true,
    );
    assert_eq!(dt.timezone(), Tz::UTC);
    assert_eq!(dt.naive_local(), new_york((2000, 1, 1), (0, 0, 0)));
}

#[hegel::test]
fn test_datetimes_tz_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i64>()
            .min_value(-4_000_000_000)
            .max_value(4_000_000_000),
    );
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(4_000_000_000));
    let min = DateTime::<Utc>::from_timestamp(lo, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(hi, 999_999_999).unwrap();
    let dt = tc.draw(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones())
            .min_instant(min)
            .max_instant(max),
    );
    assert!(dt >= min && dt <= max);
    assert!(resolves_to_itself(&dt));
}

#[hegel::test]
fn test_datetimes_tz_rebuilt_after_a_draw_uses_the_new_bounds(tc: hegel::TestCase) {
    let max = new_york((2000, 1, 1), (0, 0, 0));
    let g = chrono_gs::datetimes().timezones(chrono_gs::time_zones().names([Tz::UTC]));
    let _ = tc.draw(&g);
    let dt = tc.draw(g.max_value(max));
    assert!(dt.naive_local() <= max);
}

#[test]
fn test_datetimes_tz_stay_in_bounds_where_chrono_tz_and_the_engine_disagree_on_a_gap() {
    let min = new_york((2027, 3, 14), (2, 0, 0));
    let max = new_york((2027, 3, 14), (3, 59, 59));
    AssertAllExamples::new(
        chrono_gs::datetimes()
            .timezones(chrono_gs::time_zones().names([Tz::America__Edmonton]))
            .min_value(min)
            .max_value(max),
        move |dt| dt.naive_local() >= min && dt.naive_local() <= max && resolves_to_itself(dt),
    )
    .run_with_health_checks_suppressed(&[HealthCheck::FilterTooMuch]);
}
