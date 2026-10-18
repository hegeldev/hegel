use crate::common::utils::{assert_all_examples, check_can_generate_examples, find_any, minimal};
use hegel::datetime::Precision;
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;
use jiff::{SignedDuration, Span, Timestamp};

#[test]
fn test_jiff_timestamps_default() {
    check_can_generate_examples(jiff_gs::timestamps());
}

#[test]
fn test_jiff_timestamps_reach_both_ends_of_the_range() {
    find_any(jiff_gs::timestamps(), |t| *t == Timestamp::MIN);
    find_any(jiff_gs::timestamps(), |t| *t == Timestamp::MAX);
}

#[test]
fn test_jiff_timestamps_min_value() {
    let min = Timestamp::UNIX_EPOCH;
    assert_all_examples(jiff_gs::timestamps().min_value(min), move |t| *t >= min);
}

#[test]
fn test_jiff_timestamps_max_value() {
    let max = Timestamp::UNIX_EPOCH;
    assert_all_examples(jiff_gs::timestamps().max_value(max), move |t| *t <= max);
}

#[test]
fn test_jiff_timestamps_fractional_bounds_below_the_epoch() {
    let min = Timestamp::new(-2, 500_000_000).unwrap();
    let max = Timestamp::new(-1, 250_000_000).unwrap();
    assert_all_examples(
        jiff_gs::timestamps().min_value(min).max_value(max),
        move |t| *t >= min && *t <= max,
    );
    find_any(
        jiff_gs::timestamps().min_value(min).max_value(max),
        move |t| *t == min,
    );
}

#[test]
fn test_jiff_timestamps_precision() {
    assert_all_examples(
        jiff_gs::timestamps().precision(Precision::Milliseconds),
        |t| t.subsec_nanosecond() % 1_000_000 == 0,
    );
}

#[test]
fn test_jiff_timestamps_in_vec() {
    let max = Timestamp::UNIX_EPOCH;
    assert_all_examples(
        gs::vecs(jiff_gs::timestamps().max_value(max)).max_size(5),
        move |v| v.iter().all(|t| *t <= max),
    );
}

#[test]
fn test_jiff_timestamp_default_generator() {
    check_can_generate_examples(gs::default::<Timestamp>());
}

#[hegel::test]
fn test_jiff_timestamps_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::timestamps());
    let hi = tc.draw(jiff_gs::timestamps().min_value(lo));
    let v = tc.draw(jiff_gs::timestamps().min_value(lo).max_value(hi));
    assert!(v >= lo && v <= hi);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_timestamps_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::timestamps()
            .min_value(Timestamp::from_second(10).unwrap())
            .max_value(Timestamp::from_second(5).unwrap()),
    );
}

#[test]
fn test_jiff_spans_default() {
    check_can_generate_examples(jiff_gs::spans());
}

#[test]
fn test_jiff_spans_default_reach_every_unit() {
    find_any(jiff_gs::spans(), |s| {
        s.get_years() != 0 && s.get_nanoseconds() != 0
    });
    find_any(jiff_gs::spans(), |s| s.is_negative());
    find_any(jiff_gs::spans(), |s| s.is_positive());
}

#[test]
fn test_jiff_spans_every_unit_builder() {
    assert_all_examples(
        jiff_gs::spans()
            .years(1, 2)
            .months(1, 2)
            .weeks(1, 2)
            .days(1, 2)
            .hours(1, 2)
            .minutes(1, 2)
            .seconds(1, 2)
            .milliseconds(1, 2)
            .microseconds(1, 2)
            .nanoseconds(1, 2),
        |s| {
            [
                i64::from(s.get_years()),
                i64::from(s.get_months()),
                i64::from(s.get_weeks()),
                i64::from(s.get_days()),
                i64::from(s.get_hours()),
                s.get_minutes(),
                s.get_seconds(),
                s.get_milliseconds(),
                s.get_microseconds(),
                s.get_nanoseconds(),
            ]
            .into_iter()
            .all(|unit| unit == 1 || unit == 2)
        },
    );
}

#[test]
fn test_jiff_spans_enabled_units_only() {
    assert_all_examples(jiff_gs::spans().days(0, 30).hours(0, 23), |s| {
        (0..=30).contains(&s.get_days())
            && (0..=23).contains(&s.get_hours())
            && s.get_years() == 0
            && s.get_nanoseconds() == 0
    });
}

#[test]
fn test_jiff_spans_min_nanoseconds() {
    assert_all_examples(jiff_gs::spans().min_nanoseconds(0), |s| {
        s.get_nanoseconds() >= 0 && s.get_years() == 0
    });
}

#[test]
fn test_jiff_spans_max_nanoseconds() {
    assert_all_examples(jiff_gs::spans().max_nanoseconds(0), |s| {
        s.get_nanoseconds() <= 0 && s.get_years() == 0
    });
}

#[test]
fn test_jiff_spans_non_negative() {
    assert_all_examples(jiff_gs::spans().non_negative(true), |s| !s.is_negative());
    assert_all_examples(
        jiff_gs::spans().non_negative(true).max_nanoseconds(5),
        |s| (0..=5).contains(&s.get_nanoseconds()),
    );
}

#[test]
fn test_jiff_spans_in_vec() {
    assert_all_examples(
        gs::vecs(jiff_gs::spans().min_nanoseconds(0).max_nanoseconds(1_000)).max_size(5),
        |v| v.iter().all(|s| (0..=1_000).contains(&s.get_nanoseconds())),
    );
}

#[test]
fn test_jiff_spans_shrink_to_zero() {
    assert_eq!(minimal(jiff_gs::spans(), |_| true), Span::new().fieldwise());
}

#[test]
fn test_jiff_span_default_generator() {
    check_can_generate_examples(gs::default::<Span>());
}

#[hegel::test]
fn test_jiff_spans_property(tc: hegel::TestCase) {
    let lo = tc.draw(gs::integers::<i64>().min_value(-19_998).max_value(19_998));
    let hi = tc.draw(gs::integers::<i64>().min_value(lo).max_value(19_998));
    let s = tc.draw(jiff_gs::spans().years(lo, hi));
    let n = i64::from(s.get_years());
    assert!(n >= lo && n <= hi && s.get_months() == 0);
}

#[hegel::test]
#[should_panic(expected = "max_nanoseconds < min_nanoseconds")]
fn test_jiff_spans_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(jiff_gs::spans().min_nanoseconds(10).max_nanoseconds(5));
}

#[hegel::test]
#[should_panic(expected = "nanoseconds must be within")]
fn test_jiff_spans_min_below_span_limit(tc: hegel::TestCase) {
    tc.draw(jiff_gs::spans().min_nanoseconds(i64::MIN));
}

#[hegel::test]
#[should_panic(expected = "years must be within ±19998")]
fn test_jiff_spans_max_above_unit_limit(tc: hegel::TestCase) {
    tc.draw(jiff_gs::spans().years(0, 19_999));
}

#[test]
fn test_jiff_signed_durations_default() {
    check_can_generate_examples(jiff_gs::signed_durations());
}

#[test]
fn test_jiff_signed_durations_min_value() {
    assert_all_examples(
        jiff_gs::signed_durations().min_value(SignedDuration::ZERO),
        |d| d.as_nanos() >= 0,
    );
}

#[test]
fn test_jiff_signed_durations_max_value() {
    assert_all_examples(
        jiff_gs::signed_durations().max_value(SignedDuration::ZERO),
        |d| d.as_nanos() <= 0,
    );
}

#[test]
fn test_jiff_signed_durations_precision() {
    assert_all_examples(
        jiff_gs::signed_durations().precision(Precision::Seconds),
        |d| d.subsec_nanos() == 0,
    );
}

#[test]
fn test_jiff_signed_durations_in_vec() {
    let max = SignedDuration::from_nanos(1_000);
    assert_all_examples(
        gs::vecs(
            jiff_gs::signed_durations()
                .min_value(SignedDuration::ZERO)
                .max_value(max),
        )
        .max_size(5),
        move |v| {
            v.iter()
                .all(|d| d.as_nanos() >= 0 && d.as_nanos() <= max.as_nanos())
        },
    );
}

#[test]
fn test_jiff_signed_duration_default_generator() {
    check_can_generate_examples(gs::default::<SignedDuration>());
}

#[hegel::test]
fn test_jiff_signed_durations_property(tc: hegel::TestCase) {
    let lo = tc.draw(jiff_gs::signed_durations());
    let hi = tc.draw(jiff_gs::signed_durations().min_value(lo));
    let d = tc.draw(jiff_gs::signed_durations().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
}

#[test]
fn test_jiff_signed_durations_reach_both_ends_of_the_range() {
    find_any(jiff_gs::signed_durations(), |d| *d == SignedDuration::MIN);
    find_any(jiff_gs::signed_durations(), |d| *d == SignedDuration::MAX);
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_jiff_signed_durations_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        jiff_gs::signed_durations()
            .min_value(SignedDuration::from_nanos(10))
            .max_value(SignedDuration::from_nanos(5)),
    );
}
