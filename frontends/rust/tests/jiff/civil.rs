use crate::common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::datetime::{Month, Precision};
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;
use jiff::civil::{Date, DateTime, Era, ISOWeekDate, Time, Weekday};

const ALL_WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];

#[test]
fn test_jiff_dates_default() {
    check_can_generate_examples(jiff_gs::dates());
}

#[test]
fn test_jiff_dates_year_in_range() {
    assert_all_examples(jiff_gs::dates(), |d| (1..=9999).contains(&d.year()));
}

#[test]
fn test_jiff_dates_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::dates()).max_size(5), |v| {
        v.iter().all(|d| (1..=9999).contains(&d.year()))
    });
}

#[test]
fn test_jiff_dates_min_value() {
    let min = Date::constant(2024, 6, 1);
    assert_all_examples(jiff_gs::dates().min_value(min), move |d| *d >= min);
}

#[test]
fn test_jiff_dates_max_value() {
    let max = Date::constant(2024, 6, 30);
    assert_all_examples(jiff_gs::dates().max_value(max), move |d| *d <= max);
}

#[test]
fn test_jiff_dates_bounds_can_extend_below_year_one() {
    let min = Date::constant(-9999, 1, 1);
    let max = Date::constant(-1, 12, 31);
    assert_all_examples(jiff_gs::dates().min_value(min).max_value(max), move |d| {
        *d >= min && *d <= max
    });
}

#[test]
fn test_jiff_dates_weekdays() {
    for weekday in ALL_WEEKDAYS {
        assert_all_examples(jiff_gs::dates().weekdays([weekday]), move |d| {
            d.weekday() == weekday
        });
    }
}

#[test]
fn test_jiff_dates_months() {
    assert_all_examples(
        jiff_gs::dates().months([Month::February, Month::October]),
        |d| d.month() == 2 || d.month() == 10,
    );
}

#[hegel::test]
fn test_jiff_dates_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::dates().min_value(Date::MIN).max_value(Date::MAX));
    let hi = tc.draw(jiff_gs::dates().min_value(lo).max_value(Date::MAX));
    let d = tc.draw(jiff_gs::dates().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_dates_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::dates()
            .min_value(Date::constant(2025, 1, 1))
            .max_value(Date::constant(2024, 1, 1)),
    );
}

#[test]
fn test_jiff_dates_shrink_to_the_millennium() {
    assert_eq!(
        minimal(jiff_gs::dates(), |_| true),
        Date::constant(2000, 1, 1)
    );
}

#[test]
fn test_jiff_date_default_generator() {
    check_can_generate_examples(gs::default::<Date>());
}

#[test]
fn test_jiff_times_default() {
    check_can_generate_examples(jiff_gs::times());
}

#[test]
fn test_jiff_times_components_in_range() {
    assert_all_examples(jiff_gs::times(), |t| {
        (0..=23).contains(&t.hour())
            && (0..=59).contains(&t.minute())
            && (0..=59).contains(&t.second())
    });
}

#[test]
fn test_jiff_times_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::times()).max_size(5), |v| {
        v.iter().all(|t| (0..=23).contains(&t.hour()))
    });
}

#[test]
fn test_jiff_times_min_value() {
    let min = Time::constant(12, 30, 0, 0);
    assert_all_examples(jiff_gs::times().min_value(min), move |t| *t >= min);
}

#[test]
fn test_jiff_times_max_value() {
    let max = Time::constant(6, 0, 0, 0);
    assert_all_examples(jiff_gs::times().max_value(max), move |t| *t <= max);
}

#[test]
fn test_jiff_times_precision() {
    assert_all_examples(jiff_gs::times().precision(Precision::Minutes), |t| {
        t.second() == 0 && t.subsec_nanosecond() == 0
    });
    find_any(jiff_gs::times(), |t| t.subsec_nanosecond() % 1_000 != 0);
}

#[hegel::test]
fn test_jiff_times_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::times());
    let hi = tc.draw(jiff_gs::times().min_value(lo));
    let t = tc.draw(jiff_gs::times().min_value(lo).max_value(hi));
    assert!(t >= lo && t <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_times_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::times()
            .min_value(Time::constant(13, 0, 0, 0))
            .max_value(Time::constant(12, 0, 0, 0)),
    );
}

#[test]
fn test_jiff_time_default_generator() {
    check_can_generate_examples(gs::default::<Time>());
}

#[test]
fn test_jiff_datetimes_default() {
    check_can_generate_examples(jiff_gs::datetimes());
}

#[test]
fn test_jiff_datetimes_reach_both_ends_of_the_range() {
    find_any(jiff_gs::datetimes(), |dt| dt.year() == -9999);
    find_any(jiff_gs::datetimes(), |dt| dt.year() == 9999);
}

#[test]
fn test_jiff_datetimes_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::datetimes()).max_size(5), |v| {
        v.iter().all(|dt| (-9999..=9999).contains(&dt.year()))
    });
}

#[test]
fn test_jiff_datetimes_min_value() {
    let min = DateTime::constant(2024, 1, 1, 0, 0, 0, 0);
    assert_all_examples(jiff_gs::datetimes().min_value(min), move |dt| *dt >= min);
}

#[test]
fn test_jiff_datetimes_max_value() {
    let max = DateTime::constant(2024, 12, 31, 23, 59, 59, 999_999_999);
    assert_all_examples(jiff_gs::datetimes().max_value(max), move |dt| *dt <= max);
}

#[test]
fn test_jiff_datetimes_precision() {
    assert_all_examples(jiff_gs::datetimes().precision(Precision::Seconds), |dt| {
        dt.subsec_nanosecond() == 0
    });
}

#[hegel::test]
fn test_jiff_datetimes_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::datetimes());
    let hi = tc.draw(jiff_gs::datetimes().min_value(lo));
    let dt = tc.draw(jiff_gs::datetimes().min_value(lo).max_value(hi));
    assert!(dt >= lo && dt <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_datetimes_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::datetimes()
            .min_value(DateTime::constant(2025, 1, 1, 0, 0, 0, 0))
            .max_value(DateTime::constant(2024, 1, 1, 0, 0, 0, 0)),
    );
}

#[test]
fn test_jiff_datetime_default_generator() {
    check_can_generate_examples(gs::default::<DateTime>());
}

#[test]
fn test_jiff_iso_week_dates_default() {
    check_can_generate_examples(jiff_gs::iso_week_dates());
}

#[test]
fn test_jiff_iso_week_dates_are_the_week_dates_of_dates() {
    assert_all_examples(jiff_gs::iso_week_dates(), |w| {
        (0..=9999).contains(&w.year()) && w.date().iso_week_date() == *w
    });
}

#[test]
fn test_jiff_iso_week_dates_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::iso_week_dates()).max_size(5), |v| {
        v.iter().all(|w| (1..=53).contains(&w.week()))
    });
}

#[test]
fn test_jiff_iso_week_date_default_generator() {
    check_can_generate_examples(gs::default::<ISOWeekDate>());
}

#[test]
fn test_jiff_weekdays_default() {
    check_can_generate_examples(jiff_gs::weekdays());
}

#[test]
fn test_jiff_weekdays_reach_every_day_and_shrink_to_monday() {
    for weekday in ALL_WEEKDAYS {
        find_any(jiff_gs::weekdays(), move |w| *w == weekday);
    }
    assert_eq!(minimal(jiff_gs::weekdays(), |_| true), Weekday::Monday);
}

#[test]
fn test_jiff_weekdays_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::weekdays()).max_size(7), |v| {
        v.iter().all(|w| ALL_WEEKDAYS.contains(w))
    });
}

#[test]
fn test_jiff_weekday_default_generator() {
    check_can_generate_examples(gs::default::<Weekday>());
}

#[test]
fn test_jiff_eras_default() {
    check_can_generate_examples(jiff_gs::eras());
}

#[test]
fn test_jiff_eras_reach_both_and_shrink_to_ce() {
    find_any(jiff_gs::eras(), |e| *e == Era::BCE);
    find_any(jiff_gs::eras(), |e| *e == Era::CE);
    assert_eq!(minimal(jiff_gs::eras(), |_| true), Era::CE);
}

#[test]
fn test_jiff_eras_in_vec() {
    assert_all_examples(gs::vecs(jiff_gs::eras()).max_size(5), |v| {
        v.iter().all(|e| *e == Era::BCE || *e == Era::CE)
    });
}

#[test]
fn test_jiff_era_default_generator() {
    check_can_generate_examples(gs::default::<Era>());
}
