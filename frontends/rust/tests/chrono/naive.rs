use crate::common::utils::{assert_all_examples, check_can_generate_examples};
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday};
use hegel::extras::chrono as chrono_gs;
use hegel::generators as gs;

#[test]
fn test_naive_dates_default() {
    check_can_generate_examples(chrono_gs::naive_dates());
}

#[test]
fn test_naive_dates_min_value() {
    let min = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    assert_all_examples(chrono_gs::naive_dates().min_value(min), move |d| *d >= min);
}

#[test]
fn test_naive_dates_max_value() {
    let max = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    assert_all_examples(chrono_gs::naive_dates().max_value(max), move |d| *d <= max);
}

#[test]
fn test_naive_dates_in_vec() {
    let min = NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
    let max = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    assert_all_examples(
        gs::vecs(chrono_gs::naive_dates().min_value(min).max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d >= min && *d <= max),
    );
}

#[test]
fn test_naive_date_default_generator() {
    check_can_generate_examples(gs::default::<NaiveDate>());
}

#[hegel::test]
fn test_naive_dates_property(tc: hegel::TestCase) {
    let lo_days = tc.draw(gs::integers::<i32>().min_value(-100_000).max_value(100_000));
    let hi_days = tc.draw(gs::integers::<i32>().min_value(lo_days).max_value(200_000));
    let min = NaiveDate::from_num_days_from_ce_opt(lo_days).unwrap();
    let max = NaiveDate::from_num_days_from_ce_opt(hi_days).unwrap();
    let v = tc.draw(chrono_gs::naive_dates().min_value(min).max_value(max));
    assert!(v >= min && v <= max);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_naive_dates_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        chrono_gs::naive_dates()
            .min_value(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap())
            .max_value(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
    );
}

#[test]
fn test_naive_times_default() {
    assert_all_examples(chrono_gs::naive_times(), |t| t.nanosecond() < 1_000_000_000);
}

#[test]
fn test_naive_times_min_value() {
    let min = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    assert_all_examples(chrono_gs::naive_times().min_value(min), move |t| *t >= min);
}

#[test]
fn test_naive_times_max_value() {
    let max = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    assert_all_examples(chrono_gs::naive_times().max_value(max), move |t| *t <= max);
}

#[test]
fn test_naive_times_in_vec() {
    let max = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
    assert_all_examples(
        gs::vecs(chrono_gs::naive_times().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_naive_time_default_generator() {
    check_can_generate_examples(gs::default::<NaiveTime>());
}

#[hegel::test]
fn test_naive_times_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::integers::<u32>().min_value(0).max_value(86_399));
    let hi = tc.draw(gs::integers::<u32>().min_value(lo).max_value(86_399));
    let min = NaiveTime::from_num_seconds_from_midnight_opt(lo, 0).unwrap();
    let max = NaiveTime::from_num_seconds_from_midnight_opt(hi, 999_999_999).unwrap();
    let v = tc.draw(chrono_gs::naive_times().min_value(min).max_value(max));
    assert!(v >= min && v <= max);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_naive_times_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        chrono_gs::naive_times()
            .min_value(NaiveTime::from_hms_opt(20, 0, 0).unwrap())
            .max_value(NaiveTime::from_hms_opt(8, 0, 0).unwrap()),
    );
}

#[test]
fn test_naive_times_end_of_day_leap_bound_is_a_usage_error() {
    use crate::common::utils::expect_panic;
    let leap = NaiveTime::from_hms_nano_opt(23, 59, 59, 1_500_000_000).unwrap();
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(chrono_gs::naive_times().min_value(leap));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "leap second",
    );
}

#[test]
fn test_naive_dates_weekdays() {
    assert_all_examples(
        chrono_gs::naive_dates().weekdays([Weekday::Sat, Weekday::Sun]),
        |d| matches!(d.weekday(), Weekday::Sat | Weekday::Sun),
    );
}

#[test]
fn test_naive_dates_months() {
    use chrono::Month;
    assert_all_examples(
        chrono_gs::naive_dates().months([Month::February, Month::December]),
        |d| d.month() == 2 || d.month() == 12,
    );
}

#[hegel::test]
fn test_naive_dates_masks_property(tc: hegel::TestCase) {
    use chrono::Month;
    let weekdays = tc.draw(
        gs::vecs(gs::sampled_from(vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]))
        .min_size(1),
    );
    let months = tc.draw(
        gs::vecs(gs::sampled_from(vec![
            Month::January,
            Month::February,
            Month::March,
            Month::April,
            Month::May,
            Month::June,
            Month::July,
            Month::August,
            Month::September,
            Month::October,
            Month::November,
            Month::December,
        ]))
        .min_size(1),
    );
    let d = tc.draw(
        chrono_gs::naive_dates()
            .min_value(NaiveDate::from_ymd_opt(1900, 1, 1).unwrap())
            .max_value(NaiveDate::from_ymd_opt(2100, 12, 31).unwrap())
            .weekdays(weekdays.iter().copied())
            .months(months.iter().copied()),
    );
    assert!(weekdays.contains(&d.weekday()));
    assert!(months.iter().any(|m| m.number_from_month() == d.month()));
}

#[test]
fn test_naive_times_precision() {
    use hegel::datetime::Precision;
    assert_all_examples(
        chrono_gs::naive_times().precision(Precision::Minutes),
        |t| t.second() == 0 && t.nanosecond() == 0,
    );
}

#[test]
fn test_naive_datetimes_precision() {
    use hegel::datetime::Precision;
    assert_all_examples(
        chrono_gs::naive_datetimes().precision(Precision::Milliseconds),
        |dt| dt.nanosecond() % 1_000_000 == 0,
    );
}

#[hegel::test]
fn test_naive_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(
        gs::integers::<i64>()
            .min_value(-4_000_000_000)
            .max_value(4_000_000_000),
    );
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(4_000_000_000));
    let min = chrono::DateTime::<chrono::Utc>::from_timestamp(lo, 0)
        .unwrap()
        .naive_utc();
    let max = chrono::DateTime::<chrono::Utc>::from_timestamp(hi, 999_999_999)
        .unwrap()
        .naive_utc();
    let v = tc.draw(chrono_gs::naive_datetimes().min_value(min).max_value(max));
    assert!(v >= min && v <= max);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_naive_datetimes_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        chrono_gs::naive_datetimes()
            .min_value(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap().into())
            .max_value(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap().into()),
    );
}

#[test]
fn test_naive_datetimes_default() {
    check_can_generate_examples(chrono_gs::naive_datetimes());
}

#[test]
fn test_naive_datetimes_min_value() {
    let min = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );
    assert_all_examples(chrono_gs::naive_datetimes().min_value(min), move |dt| {
        *dt >= min
    });
}

#[test]
fn test_naive_datetimes_max_value() {
    let max = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap(),
    );
    assert_all_examples(chrono_gs::naive_datetimes().max_value(max), move |dt| {
        *dt <= max
    });
}

#[test]
fn test_naive_datetimes_in_vec() {
    let min = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
    );
    let max = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 12, 31).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap(),
    );
    assert_all_examples(
        gs::vecs(chrono_gs::naive_datetimes().min_value(min).max_value(max)).max_size(5),
        move |v| v.iter().all(|dt| *dt >= min && *dt <= max),
    );
}

#[test]
fn test_naive_datetime_default_generator() {
    check_can_generate_examples(gs::default::<NaiveDateTime>());
}

#[test]
fn test_iso_weeks_default() {
    assert_all_examples(gs::default::<chrono::IsoWeek>(), |w| {
        (1..=53).contains(&w.week())
    });
}

#[test]
fn test_iso_weeks_in_vec() {
    assert_all_examples(
        gs::vecs(gs::default::<chrono::IsoWeek>()).max_size(5),
        |v| v.iter().all(|w| (1..=53).contains(&w.week())),
    );
}

#[test]
fn test_iso_week_default_generator() {
    check_can_generate_examples(gs::default::<chrono::IsoWeek>());
}

#[test]
fn test_naive_weeks_default() {
    check_can_generate_examples(chrono_gs::naive_weeks());
}

#[test]
fn test_naive_weeks_default_accessors_do_not_panic() {
    assert_all_examples(chrono_gs::naive_weeks(), |w| {
        let first = w.first_day();
        let last = w.last_day();
        let days = w.days();
        first <= last && days.contains(&first)
    });
}

#[test]
fn test_naive_times_mid_day_leap_bound_is_a_usage_error() {
    use crate::common::utils::expect_panic;
    let leap = NaiveTime::from_hms_nano_opt(12, 0, 59, 1_500_000_000).unwrap();
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(chrono_gs::naive_times().max_value(leap));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "leap second",
    );
}

#[test]
fn test_naive_datetimes_leap_bound_is_a_usage_error() {
    use crate::common::utils::expect_panic;
    let leap = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(2024, 6, 30).unwrap(),
        NaiveTime::from_hms_nano_opt(23, 59, 59, 1_500_000_000).unwrap(),
    );
    expect_panic(
        || {
            hegel::Hegel::new(move |tc| {
                let _ = tc.draw(chrono_gs::naive_datetimes().max_value(leap));
            })
            .settings(hegel::Settings::new().database(None))
            .run();
        },
        "leap second",
    );
}

#[test]
fn test_naive_weeks_start() {
    assert_all_examples(
        chrono_gs::naive_weeks().weekday_starts(gs::just(Weekday::Sun)),
        |w| match w.checked_first_day() {
            Some(d) => d.weekday() == Weekday::Sun,
            None => true,
        },
    );
}

#[test]
fn test_naive_weeks_min_date() {
    let min = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
    assert_all_examples(chrono_gs::naive_weeks().min_date(min), move |w| {
        match w.checked_last_day() {
            Some(d) => d >= min,
            None => true,
        }
    });
}

#[test]
fn test_naive_weeks_max_date() {
    let max = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
    assert_all_examples(chrono_gs::naive_weeks().max_date(max), move |w| {
        match w.checked_first_day() {
            Some(d) => d <= max,
            None => true,
        }
    });
}

#[test]
fn test_naive_weeks_in_vec() {
    assert_all_examples(
        gs::vecs(chrono_gs::naive_weeks().weekday_starts(gs::just(Weekday::Mon))).max_size(5),
        |v| {
            v.iter().all(|w| match w.checked_first_day() {
                Some(d) => d.weekday() == Weekday::Mon,
                None => true,
            })
        },
    );
}

#[test]
fn test_naive_week_default_generator() {
    check_can_generate_examples(gs::default::<chrono::NaiveWeek>());
}

#[test]
fn test_naive_dates_shrink_toward_2000_01_01() {
    use crate::common::utils::minimal;
    let d: NaiveDate = minimal(chrono_gs::naive_dates(), |_| true);
    assert_eq!(d, NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());

    let min = NaiveDate::from_ymd_opt(2005, 3, 2).unwrap();
    let d: NaiveDate = minimal(chrono_gs::naive_dates().min_value(min), |_| true);
    assert_eq!(d, min);

    let max = NaiveDate::from_ymd_opt(1990, 6, 6).unwrap();
    let d: NaiveDate = minimal(chrono_gs::naive_dates().max_value(max), |_| true);
    assert_eq!(d, max);
}
