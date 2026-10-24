mod common;

use common::utils::expect_panic;
use hegel::generators as gs;
use hegel::{HealthCheck, Hegel, Settings, TestCase};

/// Regression for issue #78: a test whose only input is rejected by
/// `assume(false)` before any draw must error instead of passing
/// vacuously, as Hypothesis does with `Unsatisfiable`.
#[hegel::test(database = None)]
#[should_panic(expected = "Unsatisfiable")]
fn test_assume_false_without_draws_is_unsatisfiable(tc: TestCase) {
    tc.assume(false);
}

#[test]
fn reject_without_draws_is_unsatisfiable() {
    expect_panic(
        || {
            Hegel::new(|tc: TestCase| tc.reject())
                .settings(Settings::new().database(None))
                .run();
        },
        "Unsatisfiable",
    );
}

/// Unlike FilterTooMuch, Unsatisfiable is not a health check: no amount
/// of retrying can produce a valid input, so suppression does not apply.
#[test]
fn unsatisfiable_is_not_suppressible() {
    expect_panic(
        || {
            Hegel::new(|tc: TestCase| tc.assume(false))
                .settings(
                    Settings::new()
                        .database(None)
                        .suppress_health_check(HealthCheck::all()),
                )
                .run();
        },
        "Unsatisfiable",
    );
}

/// A test that draws data and rejects every input is Unsatisfiable even
/// with every health check suppressed: suppression lets generation spend
/// its whole invalid budget, but a run with no valid input must not pass.
#[test]
fn always_rejecting_test_with_draws_is_unsatisfiable() {
    expect_panic(
        || {
            Hegel::new(|tc: TestCase| {
                tc.draw(gs::booleans());
                tc.reject();
            })
            .settings(
                Settings::new()
                    .database(None)
                    .suppress_health_check(HealthCheck::all()),
            )
            .run();
        },
        "Unsatisfiable",
    );
}

/// A test accepting one of seven equally likely values keeps generating
/// until it has found it: it never ends early as passed, nor as
/// Unsatisfiable, on a run of rejections.
#[test]
fn rare_valid_input_is_found_under_suppression() {
    for seed in 0..50 {
        let valid = std::cell::Cell::new(0u64);
        Hegel::new(|tc: TestCase| {
            let value = tc.draw(gs::integers::<i32>().min_value(0).max_value(6));
            tc.assume(value == 6);
            valid.set(valid.get() + 1);
        })
        .settings(
            Settings::new()
                .database(None)
                .seed(Some(seed))
                .test_cases(5)
                .suppress_health_check(HealthCheck::all()),
        )
        .run();
        assert_eq!(valid.get(), 5, "seed {seed}");
    }
}

#[test]
fn trivial_passing_test_is_not_unsatisfiable() {
    Hegel::new(|_tc: TestCase| {})
        .settings(Settings::new().database(None))
        .run();
}

#[test]
fn unsatisfiable_panic_message_starts_with_its_kind() {
    let panic = std::panic::catch_unwind(|| {
        Hegel::new(|tc: TestCase| tc.reject())
            .settings(Settings::new().database(None))
            .run();
    })
    .unwrap_err();
    let msg = panic.downcast_ref::<String>().unwrap();
    assert!(msg.starts_with("Unsatisfiable: "), "{msg:?}");
}
