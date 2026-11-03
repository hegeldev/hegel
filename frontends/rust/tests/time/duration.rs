use crate::common::utils::{assert_all_examples, check_can_generate_examples, minimal};
use hegel::datetime::Precision;
use hegel::extras::time as time_gs;
use hegel::generators as gs;
use time::Duration;

#[test]
fn test_durations_default() {
    check_can_generate_examples(time_gs::durations());
    assert_eq!(minimal(time_gs::durations(), |_| true), Duration::ZERO);
}

#[test]
fn test_durations_min_value() {
    let min = Duration::ZERO;
    assert_all_examples(time_gs::durations().min_value(min), move |d| *d >= min);
}

#[test]
fn test_durations_max_value() {
    let max = Duration::seconds(60);
    assert_all_examples(time_gs::durations().max_value(max), move |d| *d <= max);
}

#[test]
fn test_durations_precision() {
    assert_all_examples(time_gs::durations().precision(Precision::Seconds), |d| {
        d.subsec_nanoseconds() == 0
    });
}

#[test]
fn test_durations_in_vec() {
    let max = Duration::seconds(60);
    assert_all_examples(
        gs::vecs(time_gs::durations().max_value(max)).max_size(5),
        move |v| v.iter().all(|d| *d <= max),
    );
}

#[test]
fn test_duration_default_generator() {
    check_can_generate_examples(gs::default::<Duration>());
}

#[hegel::test]
fn test_durations_property(tc: hegel::TestCase) {
    let lo = tc.draw(time_gs::durations());
    let hi = tc.draw(time_gs::durations().min_value(lo));
    let d = tc.draw(time_gs::durations().min_value(lo).max_value(hi));
    assert!(d >= lo && d <= hi);
    let oracle = jiff::SignedDuration::new(d.whole_seconds(), d.subsec_nanoseconds());
    assert_eq!(oracle.as_nanos(), d.whole_nanoseconds());
}

#[hegel::test]
#[should_panic(expected = "max_value < min_value")]
fn test_durations_min_greater_than_max(tc: hegel::TestCase) {
    tc.draw(
        time_gs::durations()
            .min_value(Duration::seconds(1))
            .max_value(Duration::ZERO),
    );
}
