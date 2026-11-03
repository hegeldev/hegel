use crate::common::utils::{assert_all_examples, check_can_generate_examples, minimal};
use hegel::datetime::Precision;
use hegel::extras::time as time_gs;
use hegel::generators as gs;
use time::{Date, Month, PrimitiveDateTime, Time, UtcDateTime, Weekday};

fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

fn datetime(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> PrimitiveDateTime {
    PrimitiveDateTime::new(
        date(year, month, day),
        Time::from_hms(hour, minute, second).unwrap(),
    )
}

#[test]
fn test_dates_default() {
    check_can_generate_examples(time_gs::dates());
}

#[test]
fn test_dates_shrink_to_the_millennium() {
    assert_eq!(minimal(time_gs::dates(), |_| true), date(2000, 1, 1));
}

#[test]
fn test_dates_min_value() {
    let min = date(2024, 1, 1);
    assert_all_examples(time_gs::dates().min_value(min), move |d| *d >= min);
}

#[test]
fn test_dates_max_value() {
    let max = date(1999, 12, 31);
    assert_all_examples(time_gs::dates().max_value(max), move |d| *d <= max);
}

#[test]
fn test_dates_weekdays() {
    assert_all_examples(
        time_gs::dates().weekdays([Weekday::Saturday, Weekday::Sunday]),
        |d| matches!(d.weekday(), Weekday::Saturday | Weekday::Sunday),
    );
    for n in 0..7 {
        let weekday = Weekday::Monday.nth_next(n);
        assert_all_examples(time_gs::dates().weekdays([weekday]), move |d| {
            d.weekday() == weekday
        });
    }
}

#[test]
fn test_dates_months() {
    assert_all_examples(
        time_gs::dates().months([Month::February, Month::December]),
        |d| matches!(d.month(), Month::February | Month::December),
    );
    for n in 1..=12 {
        let month = Month::try_from(n).unwrap();
        assert_all_examples(time_gs::dates().months([month]), move |d| {
            d.month() == month
        });
    }
}

#[test]
fn test_dates_in_vec() {
    let max = date(1999, 12, 31);
    assert_all_examples(
        gs::vecs(time_gs::dates().max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d <= max),
    );
}

#[test]
fn test_date_default_generator() {
    check_can_generate_examples(gs::default::<Date>());
}

#[hegel::test]
fn test_dates_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::dates());
    let hi = tc.draw(time_gs::dates().min_value(lo));
    let d = tc.draw(time_gs::dates().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_dates_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::dates()
            .min_value(date(2025, 1, 1))
            .max_value(date(2024, 1, 1)),
    );
}

#[test]
fn test_times_default() {
    check_can_generate_examples(time_gs::times());
}

#[test]
fn test_times_min_value() {
    let min = Time::from_hms(12, 0, 0).unwrap();
    assert_all_examples(time_gs::times().min_value(min), move |t| *t >= min);
}

#[test]
fn test_times_max_value() {
    let max = Time::from_hms(12, 0, 0).unwrap();
    assert_all_examples(time_gs::times().max_value(max), move |t| *t <= max);
}

#[test]
fn test_times_precision() {
    assert_all_examples(time_gs::times().precision(Precision::Minutes), |t| {
        t.second() == 0 && t.nanosecond() == 0
    });
    assert_all_examples(time_gs::times().precision(Precision::Milliseconds), |t| {
        t.nanosecond() % 1_000_000 == 0
    });
}

#[test]
fn test_times_in_vec() {
    let max = Time::from_hms(12, 0, 0).unwrap();
    assert_all_examples(
        gs::vecs(time_gs::times().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_time_default_generator() {
    check_can_generate_examples(gs::default::<Time>());
}

#[hegel::test]
fn test_times_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::times());
    let hi = tc.draw(time_gs::times().min_value(lo));
    let t = tc.draw(time_gs::times().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_times_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::times()
            .min_value(Time::from_hms(12, 0, 0).unwrap())
            .max_value(Time::MIDNIGHT),
    );
}

#[test]
fn test_primitive_datetimes_default() {
    check_can_generate_examples(time_gs::primitive_datetimes());
}

#[test]
fn test_primitive_datetimes_shrink_to_the_millennium() {
    assert_eq!(
        minimal(time_gs::primitive_datetimes(), |_| true),
        datetime(2000, 1, 1, 0, 0, 0)
    );
}

#[test]
fn test_primitive_datetimes_min_value() {
    let min = datetime(2024, 1, 1, 12, 0, 0);
    assert_all_examples(time_gs::primitive_datetimes().min_value(min), move |dt| {
        *dt >= min
    });
}

#[test]
fn test_primitive_datetimes_max_value() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(time_gs::primitive_datetimes().max_value(max), move |dt| {
        *dt <= max
    });
}

#[test]
fn test_primitive_datetimes_precision() {
    assert_all_examples(
        time_gs::primitive_datetimes().precision(Precision::Hours),
        |dt| dt.time() == Time::from_hms(dt.hour(), 0, 0).unwrap(),
    );
}

#[test]
fn test_primitive_datetimes_in_vec() {
    let max = datetime(1999, 12, 31, 12, 0, 0);
    assert_all_examples(
        gs::vecs(time_gs::primitive_datetimes().max_value(max)).max_size(5),
        move |v| v.iter().all(|dt| *dt <= max),
    );
}

#[test]
fn test_primitive_datetime_default_generator() {
    check_can_generate_examples(gs::default::<PrimitiveDateTime>());
}

#[test]
fn test_utc_datetime_default_generator() {
    assert_all_examples(gs::default::<UtcDateTime>(), |dt| {
        (UtcDateTime::MIN..=UtcDateTime::MAX).contains(dt)
    });
}

#[hegel::test]
fn test_primitive_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::primitive_datetimes());
    let hi = tc.draw(time_gs::primitive_datetimes().min_value(lo));
    let dt = tc.draw(time_gs::primitive_datetimes().min_value(lo).max_value(hi));
    assert!(dt >= lo && dt <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_primitive_datetimes_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::primitive_datetimes()
            .min_value(datetime(2025, 1, 1, 0, 0, 0))
            .max_value(datetime(2024, 1, 1, 0, 0, 0)),
    );
}
