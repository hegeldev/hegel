//! Pins how a failure driven by hidden state surfaces. A vanishing failure
//! — one that fails at discovery but never again on replay — is reported
//! unconfirmed under the default quiet strictness, re-raising the test's
//! own panic, and aborts with the flaky diagnostic under `Error`. A
//! failure that clears the confirmation bar before drying up keeps its
//! confirmation capture in the report.

mod common;

use common::utils::capture_hegel_output;
use hegel::generators as gs;
use hegel::{Hegel, NondeterminismStrictness, Phase, Settings, TestCase, Verbosity};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};

fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("")
}

#[test]
fn a_vanishing_failure_is_still_reported_under_quiet_strictness() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let body = |tc: TestCase| {
        tc.draw(gs::booleans());
        let i = CALLS.fetch_add(1, Ordering::SeqCst);
        assert!(i != 0, "fails only on the first call");
    };
    let panic = catch_unwind(AssertUnwindSafe(|| {
        Hegel::new(body)
            .settings(
                Settings::new()
                    .phases([Phase::Generate])
                    .database(None)
                    .verbosity(Verbosity::Quiet),
            )
            .run();
    }))
    .expect_err("a vanishing failure is still reported, re-raising its own panic");
    let msg = panic_message(&*panic);
    assert!(msg.contains("fails only on the first call"), "got: {msg:?}");
}

#[test]
fn error_strictness_aborts_a_vanishing_failure_as_flaky() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let body = |tc: TestCase| {
        tc.draw(gs::booleans());
        let i = CALLS.fetch_add(1, Ordering::SeqCst);
        assert!(i != 0, "fails only on the first call");
    };
    let panic = catch_unwind(AssertUnwindSafe(|| {
        Hegel::new(body)
            .settings(
                Settings::new()
                    .phases([Phase::Generate])
                    .database(None)
                    .verbosity(Verbosity::Quiet)
                    .nondeterminism_strictness(NondeterminismStrictness::Error),
            )
            .run();
    }))
    .expect_err("error strictness aborts a vanishing failure");
    let msg = panic_message(&*panic);
    assert!(msg.contains("Flaky test detected"), "got: {msg:?}");
    assert!(
        msg.contains("The failure that did not reproduce was: Panic at "),
        "the diagnostic names the failure the engine recorded, got: {msg:?}"
    );
    assert!(msg.contains("test_flaky_replay.rs"), "got: {msg:?}");
}

/// The hidden counter is calibrated to the engine's execution schedule:
/// the simplest-case probe passes (execution 0), the first generated case
/// fails (1, discovery), the shrink-entry verify passes (2, flipping the
/// run nondeterministic), the confirmation bar's four replays fail (3-6,
/// an early accept on the fourth failure), and everything from 8 on
/// passes. The one failing shrink probe (7) is rejected, shrinking is
/// otherwise all-reject, and the final replay is dry. The report must
/// print the stamped confirmation capture (draw lines and diagnostic)
/// with the dry-replay caveat, not the empty capture of that unstamped
/// probe.
#[test]
fn a_confirmed_but_dry_failure_prints_its_confirmation_capture() {
    static CALLS: AtomicUsize = AtomicUsize::new(0);
    let body = |tc: TestCase| {
        let x: i64 = tc.draw(gs::integers());
        let n = CALLS.fetch_add(1, Ordering::SeqCst);
        assert!(n == 0 || n == 2 || n >= 8, "boom: x = {x}");
    };
    let (lines, result) = capture_hegel_output(|| {
        Hegel::new(body)
            .settings(Settings::new().database(None))
            .run();
    });
    result.expect_err("a confirmed failure fails the run even when the final replay is dry");
    let text = lines.join("\n");
    assert!(
        text.contains("let draw"),
        "the confirmation capture's draw lines must be printed:\n{text}"
    );
    assert!(
        text.contains("panicked at"),
        "the confirmation capture's diagnostic must be printed:\n{text}"
    );
    assert!(
        text.contains("not reproduced at report time"),
        "the caveat must name the dry final replay:\n{text}"
    );
}

static VEC_COIN: AtomicUsize = AtomicUsize::new(0);

/// A flaky test over a collection. The collection's continue/stop draws
/// open no span of their own, so the counterexample graph must tell them
/// apart by their position in the collection's span: merged into one
/// state, a replay would be served the wrong one and the failure would
/// never confirm.
fn flaky_over_a_vec(tc: TestCase) {
    let v = tc.draw(gs::vecs(gs::integers::<u8>()));
    if v.len() >= 2 && VEC_COIN.fetch_add(1, Ordering::SeqCst) % 2 == 0 {
        panic!("boom: v = {v:?}");
    }
}

#[test]
fn a_flaky_failure_over_a_collection_is_confirmed_and_shrunk() {
    let (lines, result) = capture_hegel_output(|| {
        Hegel::new(flaky_over_a_vec)
            .settings(Settings::new().database(None).seed(Some(3)))
            .run();
    });
    result.expect_err("the failure is reported");
    let text = lines.join("\n");
    assert!(
        text.contains("note: nondeterministic failure, confirmed:"),
        "the failure must confirm by replay:\n{text}"
    );
    assert!(
        text.contains("let draw_1 = [0, 0];"),
        "the collection must shrink to the two elements the failure needs:\n{text}"
    );
}

static FLAKY_FIRED: AtomicUsize = AtomicUsize::new(0);

/// Every input fails: the first execution with one bug that never fires
/// again, every execution after it with another. The first bug's replays
/// flip the run nondeterministic; the second reproduces at every replay.
fn one_flaky_then_one_deterministic_bug(tc: TestCase) {
    let x: f64 = tc.draw(gs::floats::<f64>().min_value(10.0).max_value(10.0));
    if FLAKY_FIRED.fetch_add(1, Ordering::SeqCst) == 0 {
        panic!("flaky: x = {x}");
    }
    panic!("deterministic: x = {x}");
}

#[test]
fn a_failure_that_reproduced_at_every_replay_has_no_caveat_in_a_flipped_run() {
    let (lines, result) = capture_hegel_output(|| {
        Hegel::new(one_flaky_then_one_deterministic_bug)
            .settings(Settings::new().database(None))
            .run();
    });
    let panic = result.expect_err("the deterministic failure is reported");
    let msg = panic_message(&*panic);
    assert!(msg.contains("deterministic: x = "), "got: {msg:?}");
    let text = lines.join("\n");
    assert!(!text.contains("flaky: x = "), "{text}");
    assert!(
        !text.contains("note: nondeterministic") && !text.contains("note: unconfirmed"),
        "no caveat on a failure that reproduced at every replay:\n{text}"
    );
}

static PASSED_ONCE: AtomicUsize = AtomicUsize::new(0);

/// `true` passes the first time it is drawn and fails every time after:
/// the failure then reproduces at every replay, but the engine saw the
/// same input pass, so the report keeps its caveat.
#[test]
fn an_input_that_passed_before_it_failed_keeps_the_caveat() {
    let body = |tc: TestCase| {
        if tc.draw(gs::booleans()) && PASSED_ONCE.fetch_add(1, Ordering::SeqCst) > 0 {
            panic!("fails from the second time on");
        }
    };
    let (lines, result) = capture_hegel_output(|| {
        Hegel::new(body)
            .settings(Settings::new().database(None))
            .run();
    });
    let panic = result.expect_err("the failure is reported");
    let msg = panic_message(&*panic);
    assert!(
        msg.contains("fails from the second time on"),
        "got: {msg:?}"
    );
    let text = lines.join("\n");
    assert!(
        text.contains("note: nondeterministic failure, confirmed: failed 20 of 20 replays"),
        "{text}"
    );
}
