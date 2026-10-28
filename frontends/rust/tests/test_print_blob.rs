//! The `print_blob` setting controls whether a failing run prints a
//! copy-pasteable `#[hegel::reproduce_failure("…")]` reproducer line.
//!
//! The reproducer line is written straight to stderr at the catch site, so
//! these run an `#[ignore]`d failing fixture test in a subprocess (this same
//! binary, via `exec::self_test`) and assert the line is present by default
//! and absent when `print_blob` is turned off.

mod common;

use common::exec::self_test;
use common::utils::capture_hegel_output;
use hegel::TestCase;
use hegel::generators as gs;

/// Marker printed by the reproducer line (see `run_lifecycle::reproducer_line`).
const REPRODUCER_MARKER: &str = "To reproduce this failure";

#[hegel::test]
#[ignore = "fixture: run via exec::self_test"]
fn print_blob_default_fixture(tc: TestCase) {
    let x: i32 = tc.draw(gs::integers());
    assert!(x < 5, "x was {x}");
}

#[hegel::test(print_blob = false)]
#[ignore = "fixture: run via exec::self_test"]
fn print_blob_false_fixture(tc: TestCase) {
    let x: i32 = tc.draw(gs::integers());
    assert!(x < 5, "x was {x}");
}

#[test]
fn print_blob_default_prints_reproducer_line() {
    self_test("print_blob_default_fixture")
        .env("HEGEL_DEFAULT_PROFILE", "base")
        .expect_failure(REPRODUCER_MARKER)
        .run();
}

#[test]
fn print_blob_false_suppresses_reproducer_line() {
    let out = self_test("print_blob_false_fixture")
        .expect_failure("x was")
        .run();
    let combined = format!("{}\n{}", out.stdout, out.stderr);
    assert!(
        !combined.contains(REPRODUCER_MARKER),
        "reproducer line should be suppressed with print_blob = false:\n{combined}"
    );
}

#[test]
fn ci_profile_prints_reproducer_line() {
    self_test("print_blob_default_fixture")
        .env("HEGEL_DEFAULT_PROFILE", "ci")
        .expect_failure(REPRODUCER_MARKER)
        .run();
}

fn reproducer_output(f: impl FnOnce() + std::panic::UnwindSafe) -> String {
    let (lines, result) = capture_hegel_output(f);
    assert!(result.is_err());
    lines.join("\n")
}

#[test]
fn standalone_function_reproducer_names_its_attribute() {
    #[hegel::standalone_function(print_blob = true, database = None)]
    fn fails(tc: TestCase) {
        let x: i32 = tc.draw(gs::integers());
        assert!(x < 5, "x was {x}");
    }
    let output = reproducer_output(fails);
    assert!(
        output.contains(
            "To reproduce this failure, add the attribute below #[hegel::standalone_function]:"
        ),
        "{output}"
    );
}

#[test]
fn builder_reproducer_points_at_the_builder() {
    let output = reproducer_output(|| {
        hegel::Hegel::new(|tc: TestCase| {
            let x: i32 = tc.draw(gs::integers());
            assert!(x < 5, "x was {x}");
        })
        .settings(hegel::Settings::new().print_blob(true).database(None))
        .run();
    });
    assert!(
        output.contains("To reproduce this failure, call this on the Hegel builder before run():"),
        "{output}"
    );
    assert!(output.contains("    .reproduce_failure(\""), "{output}");
}
