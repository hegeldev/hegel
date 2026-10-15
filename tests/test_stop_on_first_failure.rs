//! The `stop_on_first_failure` setting as the frontend exposes it: the
//! `Settings` builder and the `#[hegel::test]` attribute argument.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};

use hegel::generators as gs;
use hegel::{Hegel, Phase, Settings, TestCase, Verbosity};

/// Run a property that fails on every odd input with a 50-case budget and
/// return how many cases it executed.
fn cases_run(stop_on_first_failure: bool) -> u64 {
    let calls = AtomicU64::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        Hegel::new(|tc: TestCase| {
            calls.fetch_add(1, Ordering::SeqCst);
            let n = tc.draw(gs::integers::<u8>());
            assert!(n % 2 == 0, "odd input: {n}");
        })
        .settings(
            Settings::new()
                .database(None)
                .derandomize(true)
                .verbosity(Verbosity::Quiet)
                .phases([Phase::Generate])
                .test_cases(50)
                .stop_on_first_failure(stop_on_first_failure),
        )
        .run();
    }));
    result.expect_err("the property fails on odd inputs");
    calls.load(Ordering::SeqCst)
}

#[test]
fn test_stop_on_first_failure_decides_whether_the_budget_runs_out() {
    let stopped = cases_run(true);
    assert!(stopped < 50, "expected an early stop, ran {stopped} cases");
    let whole = cases_run(false);
    assert!(whole > 50, "expected the whole budget, ran {whole} cases");
}

#[hegel::test(
    stop_on_first_failure = false,
    derandomize = true,
    test_cases = 30u64,
    verbosity = hegel::Verbosity::Quiet,
    database = None
)]
#[should_panic(expected = "odd input")]
fn test_macro_accepts_stop_on_first_failure(tc: TestCase) {
    let n = tc.draw(gs::integers::<u8>());
    assert!(n % 2 == 0, "odd input: {n}");
}
