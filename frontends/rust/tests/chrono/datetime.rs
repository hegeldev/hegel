use crate::common::utils::{assert_all_examples, check_can_generate_examples};
use chrono::{DateTime, FixedOffset, Utc};
use hegel::extras::chrono as chrono_gs;
use hegel::generators as gs;

#[test]
fn test_datetimes_default() {
    check_can_generate_examples(chrono_gs::datetimes());
}

#[test]
fn test_datetimes_in_vec() {
    check_can_generate_examples(gs::vecs(chrono_gs::datetimes()).max_size(5));
}

#[test]
fn test_datetimes_min_value() {
    let min = DateTime::<Utc>::from_timestamp(0, 0).unwrap().naive_utc();
    assert_all_examples(chrono_gs::datetimes().min_value(min), move |dt| {
        dt.naive_local() >= min
    });
}

#[test]
fn test_datetimes_max_value() {
    let max = DateTime::<Utc>::from_timestamp(2_000_000_000, 0)
        .unwrap()
        .naive_utc();
    assert_all_examples(chrono_gs::datetimes().max_value(max), move |dt| {
        dt.naive_local() <= max
    });
}

#[test]
fn test_datetimes_offset_constrained() {
    let min_off = FixedOffset::east_opt(-3600).unwrap();
    let max_off = FixedOffset::east_opt(3600).unwrap();
    assert_all_examples(
        chrono_gs::datetimes().timezones(
            chrono_gs::fixed_offsets()
                .min_value(min_off)
                .max_value(max_off),
        ),
        move |dt| {
            let secs = dt.offset().local_minus_utc();
            (-3600..=3600).contains(&secs)
        },
    );
}

#[test]
fn test_datetime_fixed_offset_default_generator() {
    check_can_generate_examples(gs::default::<DateTime<FixedOffset>>());
}

#[test]
#[should_panic(expected = "max_value < min_value")]
fn test_datetimes_min_greater_than_max() {
    let g = chrono_gs::datetimes()
        .min_value(
            DateTime::<Utc>::from_timestamp(2_000_000_000, 0)
                .unwrap()
                .naive_utc(),
        )
        .max_value(DateTime::<Utc>::from_timestamp(0, 0).unwrap().naive_utc());
    check_can_generate_examples(g);
}

#[test]
fn test_datetimes_utc_default() {
    check_can_generate_examples(chrono_gs::datetimes().timezones(gs::just(Utc)));
}

#[test]
fn test_datetimes_utc_min_value() {
    let min = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .min_value(min.naive_utc()),
        move |dt| *dt >= min,
    );
}

#[test]
fn test_datetimes_utc_max_value() {
    let max = DateTime::<Utc>::from_timestamp(2_000_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .max_value(max.naive_utc()),
        move |dt| *dt <= max,
    );
}

#[test]
fn test_datetimes_utc_in_vec() {
    let max = DateTime::<Utc>::from_timestamp(2_000_000_000, 0).unwrap();
    assert_all_examples(
        gs::vecs(
            chrono_gs::datetimes()
                .timezones(gs::just(Utc))
                .max_value(max.naive_utc()),
        )
        .max_size(5),
        move |v| v.iter().all(|dt| *dt <= max),
    );
}

#[test]
fn test_datetime_utc_default_generator() {
    check_can_generate_examples(gs::default::<DateTime<Utc>>());
}

#[hegel::test]
fn test_datetimes_utc_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::integers::<i64>().min_value(0).max_value(2_000_000_000));
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(4_000_000_000));
    let min = DateTime::<Utc>::from_timestamp(lo, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(hi, 0).unwrap();
    let v = tc.draw(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .min_value(min.naive_utc())
            .max_value(max.naive_utc()),
    );
    assert!(v >= min && v <= max);
}

#[test]
fn test_datetimes_full_range_bounds() {
    check_can_generate_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .min_value(DateTime::<Utc>::MIN_UTC.naive_utc())
            .max_value(DateTime::<Utc>::MAX_UTC.naive_utc()),
    );
}

#[test]
fn test_datetimes_precision() {
    use chrono::Timelike;
    use hegel::datetime::Precision;
    assert_all_examples(chrono_gs::datetimes().precision(Precision::Minutes), |dt| {
        let local = dt.naive_local();
        local.second() == 0 && local.nanosecond() == 0
    });
}

#[test]
fn test_datetimes_utc_precision() {
    use chrono::Timelike;
    use hegel::datetime::Precision;
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .precision(Precision::Hours),
        |dt| dt.minute() == 0 && dt.second() == 0 && dt.nanosecond() == 0,
    );
}

#[test]
fn test_datetimes_utc_instant_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .min_instant(min)
            .max_instant(max),
        move |dt| *dt >= min && *dt <= max,
    );
}

#[test]
fn test_datetimes_fixed_offset_instant_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes().min_instant(min).max_instant(max),
        move |dt| *dt >= min && *dt <= max,
    );
}

#[test]
fn test_datetimes_instant_bounds_replace_wall_clock_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let far = DateTime::<Utc>::from_timestamp(4_000_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .min_value(far.naive_utc())
            .max_instant(max)
            .min_instant(min),
        move |dt| *dt >= min && *dt <= max,
    );
    assert_all_examples(
        chrono_gs::datetimes()
            .max_value(far.naive_utc())
            .min_instant(min)
            .max_instant(max),
        move |dt| *dt >= min && *dt <= max,
    );
}

#[test]
fn test_datetimes_wall_clock_bounds_replace_instant_bounds() {
    let min = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let far = DateTime::<Utc>::from_timestamp(4_000_000_000, 0).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .min_instant(far)
            .max_value(max.naive_utc())
            .min_value(min.naive_utc()),
        move |dt| dt.naive_local() >= min.naive_utc() && dt.naive_local() <= max.naive_utc(),
    );
    assert_all_examples(
        chrono_gs::datetimes()
            .max_instant(far)
            .min_value(min.naive_utc())
            .max_value(max.naive_utc()),
        move |dt| dt.naive_local() >= min.naive_utc() && dt.naive_local() <= max.naive_utc(),
    );
}

#[hegel::test]
fn test_datetimes_fixed_offset_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i64>()
            .min_value(-4_000_000_000)
            .max_value(4_000_000_000),
    );
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(4_000_000_000));
    let instants = tc.draw(gs::booleans());
    let min = DateTime::<Utc>::from_timestamp(lo, 0).unwrap();
    let max = DateTime::<Utc>::from_timestamp(hi, 999_999_999).unwrap();
    if instants {
        let dt = tc.draw(chrono_gs::datetimes().min_instant(min).max_instant(max));
        assert!(dt >= min && dt <= max);
    } else {
        let dt = tc.draw(
            chrono_gs::datetimes()
                .min_value(min.naive_utc())
                .max_value(max.naive_utc()),
        );
        assert!(dt.naive_local() >= min.naive_utc() && dt.naive_local() <= max.naive_utc());
    }
}

#[test]
fn test_datetimes_imaginary_and_ambiguous_accepted_without_transitions() {
    use hegel::datetime::Ambiguous;
    check_can_generate_examples(
        chrono_gs::datetimes()
            .allow_imaginary(true)
            .ambiguous(Ambiguous::Later),
    );
}

#[test]
fn test_datetimes_shrink_toward_2000_at_utc() {
    use crate::common::utils::minimal;
    let dt: DateTime<FixedOffset> = minimal(chrono_gs::datetimes(), |_| true);
    assert_eq!(dt.to_rfc3339(), "2000-01-01T00:00:00+00:00");
}

#[test]
fn test_datetimes_unreachable_wall_clock_window_is_rejected() {
    use crate::common::utils::expect_panic;
    let min = DateTime::<Utc>::MIN_UTC.naive_utc();
    let max = min + chrono::TimeDelta::hours(1);
    let ahead = FixedOffset::east_opt(2 * 3600).unwrap();
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(
                    chrono_gs::datetimes()
                        .min_value(min)
                        .max_value(max)
                        .timezones(gs::just(ahead)),
                );
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "Unsatisfiable|FilterTooMuch",
    );
}

#[test]
fn test_datetimes_wall_clock_window_is_clipped_to_the_offset() {
    let max = DateTime::<Utc>::MAX_UTC.naive_utc();
    let min = max - chrono::TimeDelta::hours(3);
    let behind = FixedOffset::west_opt(2 * 3600).unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .min_value(min)
            .max_value(max)
            .timezones(gs::just(behind)),
        move |dt| dt.naive_local() >= min && dt.naive_local() <= max,
    );
}

#[test]
fn test_datetimes_leap_second_bound_is_a_usage_error() {
    use crate::common::utils::expect_panic;
    let leap = chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
        chrono::NaiveTime::from_hms_nano_opt(23, 59, 59, 1_500_000_000).unwrap(),
    );
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(chrono_gs::datetimes().max_value(leap));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "leap second",
    );
}

#[hegel::test]
#[should_panic(expected = "max_instant < min_instant")]
fn test_datetimes_min_instant_greater_than_max_instant(tc: hegel::TestCase) {
    tc.draw(
        chrono_gs::datetimes()
            .min_instant(DateTime::<Utc>::from_timestamp(2_000_000_000, 0).unwrap())
            .max_instant(DateTime::<Utc>::from_timestamp(0, 0).unwrap()),
    );
}

#[test]
fn test_datetimes_year_3000() {
    let min = "3000-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    let max = "3001-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    assert_all_examples(
        chrono_gs::datetimes()
            .timezones(gs::just(Utc))
            .min_value(min.naive_utc())
            .max_value(max.naive_utc()),
        move |dt| *dt >= min && *dt <= max,
    );
}
