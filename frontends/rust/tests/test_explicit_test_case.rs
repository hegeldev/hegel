mod common;

use common::exec::self_test;
use common::utils::{assert_matches_regex, capture_hegel_output, expect_panic};
use hegel::TestCase;
use hegel::generators;

// The compile-error cases — the attribute on a bare function, above
// `#[hegel::test]`, with bad syntax, with empty arguments, or without an
// argument list — live in `tests/ui/explicit_test_case_*.rs`, where trybuild
// pins their diagnostics.

// The explicit test-case wrapper can never be smuggled onto another thread
// (the old `rejects_threading_body` compile-failure test): its handle must
// not become `Send` or `Sync`. `TestCase` is deliberately `Send` (worker
// threads may draw from clones — see `fixture_threaded_rejects`), but must
// never become `Sync`.
static_assertions::assert_not_impl_any!(hegel::ExplicitTestCase: Send, Sync);
static_assertions::assert_not_impl_any!(hegel::TestCase: Sync);

#[hegel::test]
#[hegel::explicit_test_case(x = true)]
fn test_single_explicit_case(tc: TestCase) {
    let x = tc.draw(generators::booleans());
    let _ = x;
}

#[hegel::test]
#[hegel::explicit_test_case(x = true)]
#[hegel::explicit_test_case(x = false)]
fn test_multiple_explicit_cases(tc: TestCase) {
    let x = tc.draw(generators::booleans());
    let _ = x;
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(x = 42u32)]
fn test_explicit_case_type_annotated_draw_uses_name(tc: TestCase) {
    let x: u32 = tc.draw(generators::integers());
    let _ = x;
}

#[derive(Debug, Clone, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(p = Point { x: 3, y: 4 }, q = Point { x: -1, y: 0 })]
fn test_explicit_case_with_multiple_structs(tc: TestCase) {
    let p: Point = tc.draw(generators::just(Point { x: 0, y: 0 }));
    let q: Point = tc.draw(generators::just(Point { x: 0, y: 0 }));
    let _ = (p, q);
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(s = ["hello", "world"].join(" "))]
fn test_explicit_case_with_method_chain(tc: TestCase) {
    let s: String = tc.draw(generators::text());
    let _ = s;
}

#[test]
fn test_explicit_draw_unnamed() {
    let etc = hegel::ExplicitTestCase::new().with_value("draw_1", "42", 42i32);
    etc.run(|tc: &hegel::ExplicitTestCase| {
        let x: i32 = tc.draw(generators::integers());
        assert_eq!(x, 42);
    });
}

#[test]
fn test_explicit_note() {
    let etc = hegel::ExplicitTestCase::new().with_value("x", "true", true);
    etc.run(|tc: &hegel::ExplicitTestCase| {
        let _: bool = tc.__draw_named(generators::booleans(), "x", false);
        tc.note("some note");
    });
}

#[test]
fn test_explicit_notes_printed_on_panic_inline() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "42", 42i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: i32 = tc.__draw_named(generators::integers(), "x", false);
                tc.note("a note");
                panic!("intentional");
            });
        },
        "intentional",
    );
}

#[test]
fn test_explicit_assume_passes() {
    let etc = hegel::ExplicitTestCase::new();
    etc.assume(true);
}

#[test]
fn test_explicit_assume_false_before_any_draw_is_an_error() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "1", 1i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                tc.assume(false);
                tc.__draw_named::<i32>(generators::integers(), "x", false);
            });
        },
        "Explicit test cases must not be rejected",
    );
}

#[test]
fn test_explicit_assume_false_after_a_draw_is_an_error() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "1", 1i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let x: i32 = tc.__draw_named(generators::integers(), "x", false);
                tc.assume(x < 0);
            });
        },
        "Explicit test cases must not be rejected",
    );
}

#[test]
fn test_explicit_reject_is_an_error() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new();
            etc.run(|tc: &hegel::ExplicitTestCase| {
                tc.reject();
            });
        },
        "Explicit test cases must not be rejected",
    );
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(x = -1i32)]
#[should_panic(expected = "Explicit test cases must not be rejected")]
fn test_macro_explicit_case_assume_is_an_error(tc: TestCase) {
    let x: i32 = tc.draw(generators::integers().min_value(0));
    tc.assume(x >= 0);
}

#[test]
fn test_explicit_draw_named() {
    let etc = hegel::ExplicitTestCase::new().with_value("index_1", "3", 3usize);
    etc.run(|tc: &hegel::ExplicitTestCase| {
        let index: usize = tc.draw_named("index", generators::integers());
        assert_eq!(index, 3);
    });
}

#[test]
fn test_explicit_events_are_noops() {
    let etc = hegel::ExplicitTestCase::new();
    etc.run(|tc: &hegel::ExplicitTestCase| {
        tc.event("label");
        tc.event_value("label", 1.0);
    });
}

#[test]
fn test_explicit_repeat_runs_the_body_once() {
    let etc = hegel::ExplicitTestCase::new().with_value("x_1", "5", 5i32);
    let mut runs = 0;
    etc.run(|tc: &hegel::ExplicitTestCase| {
        tc.repeat(|| {
            runs += 1;
            let x: i32 = tc.draw_named("x", generators::integers());
            assert_eq!(x, 5);
        });
    });
    assert_eq!(runs, 1);
}

#[test]
fn test_explicit_repeat_rejected_after_a_draw_is_an_error() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x_1", "5", 5i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                tc.repeat(|| {
                    let x: i32 = tc.draw_named("x", generators::integers());
                    tc.assume(x < 0);
                });
            });
        },
        "Explicit test cases must not be rejected",
    );
}

#[test]
fn test_explicit_repeat_rejected_before_drawing_is_an_error() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x_1", "5", 5i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                tc.repeat(|| {
                    tc.reject();
                });
            });
        },
        "Explicit test cases must not be rejected",
    );
}

#[test]
fn test_explicit_repeat_checks_unused_values() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x_1", "5", 5i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                tc.repeat(|| {});
            });
        },
        "never drawn",
    );
}

#[test]
fn test_explicit_repeat_propagates_failures() {
    let (lines, result) = capture_hegel_output(|| {
        let etc = hegel::ExplicitTestCase::new();
        etc.run(|tc: &hegel::ExplicitTestCase| {
            tc.repeat(|| panic!("failed in repeat"));
        });
    });
    assert!(result.is_err());
    assert_eq!(lines, vec!["// Repetition #1".to_string()]);
}

#[test]
fn test_explicit_target_is_noop() {
    let etc = hegel::ExplicitTestCase::new();
    etc.target(42.0);
    etc.target_labelled(42.0, "label");
    etc.target_labelled(0.0, "");
}

#[test]
fn test_explicit_start_span_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new();
            etc.start_span(0);
        },
        "start_span is not supported in explicit test cases",
    );
}

#[test]
fn test_explicit_stop_span_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new();
            etc.stop_span(false);
        },
        "stop_span is not supported in explicit test cases",
    );
}

#[test]
fn test_explicit_draw_silent_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.draw_silent(generators::booleans());
            });
        },
        "draw_silent is not supported in explicit test cases",
    );
}

#[test]
fn test_explicit_type_mismatch_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "42", 42i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: String = tc.__draw_named(generators::text(), "x", false);
            });
        },
        "type mismatch",
    );
}

#[test]
fn test_explicit_unused_values_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new()
                .with_value("x", "true", true)
                .with_value("y", "false", false);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "x", false);
            });
        },
        "never drawn",
    );
}

#[test]
fn test_explicit_unknown_name_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "nonexistent", false);
            });
        },
        "no value provided for.*nonexistent",
    );
}

#[test]
fn test_explicit_non_repeatable_reuse_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "x", false);
                let _: bool = tc.__draw_named(generators::booleans(), "x", false);
            });
        },
        "__draw_named.*used more than once but repeatable is false",
    );
}

#[test]
fn test_explicit_mixed_repeatable_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new()
                .with_value("x", "true", true)
                .with_value("x_1", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "x", false);
                let _: bool = tc.__draw_named(generators::booleans(), "x", true);
            });
        },
        "__draw_named.*inconsistent.*repeatable",
    );
}

#[test]
fn test_explicit_mixed_repeatable_reverse_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new()
                .with_value("x", "true", true)
                .with_value("x_1", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "x", true);
                let _: bool = tc.__draw_named(generators::booleans(), "x", false);
            });
        },
        "__draw_named.*inconsistent.*repeatable",
    );
}

#[test]
fn test_explicit_double_consume_panics() {
    expect_panic(
        || {
            let etc = hegel::ExplicitTestCase::new().with_value("x_1", "true", true);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: bool = tc.__draw_named(generators::booleans(), "x_1", false);
                let _: bool = tc.__draw_named(generators::booleans(), "x", true);
            });
        },
        "already consumed",
    );
}

#[test]
fn test_explicit_output_format_with_comment() {
    let (lines, result) = capture_hegel_output(|| {
        let etc = hegel::ExplicitTestCase::new().with_value("x", "compute()", 42i32);
        etc.run(|tc: &hegel::ExplicitTestCase| {
            let _: i32 = tc.__draw_named(generators::integers(), "x", false);
            panic!("intentional");
        });
    });
    assert!(result.is_err(), "expected the explicit case to panic");
    assert_matches_regex(&lines.join("\n"), r"let x = compute\(\); // = 42");
}

#[test]
fn test_explicit_output_format_without_comment() {
    let (lines, result) = capture_hegel_output(|| {
        let etc = hegel::ExplicitTestCase::new().with_value("x", "42", 42i32);
        etc.run(|tc: &hegel::ExplicitTestCase| {
            let _: i32 = tc.__draw_named(generators::integers(), "x", false);
            panic!("intentional");
        });
    });
    assert!(result.is_err(), "expected the explicit case to panic");
    let output = lines.join("\n");
    assert_matches_regex(&output, r"let x = 42;");
    assert!(
        !output.contains("// ="),
        "Should not have comment when source matches debug. Actual: {}",
        output
    );
}

/// Fixture for `test_explicit_notes_printed_on_panic`, run via self-exec:
/// explicit-test-case notes are printed straight to stderr on panic, so a
/// real subprocess is needed to observe them.
#[test]
#[ignore = "fixture: run via exec::self_test"]
fn explicit_notes_fixture() {
    let etc = hegel::ExplicitTestCase::new().with_value("x", "42", 42i32);
    etc.run(|tc: &hegel::ExplicitTestCase| {
        let _: i32 = tc.__draw_named(generators::integers(), "x", false);
        tc.note("important debug info");
        panic!("intentional");
    });
}

#[test]
fn test_explicit_notes_printed_on_panic() {
    let output = self_test("explicit_notes_fixture")
        .expect_failure("intentional")
        .run();
    assert_matches_regex(&output.stderr, "important debug info");
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(x = 42i32)]
#[should_panic(expected = "fail: 42")]
fn test_macro_explicit_case_output(tc: TestCase) {
    let x: i32 = tc.draw(generators::integers());
    panic!("fail: {}", x);
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(p = Point { x: 3, y: 4 })]
#[should_panic(expected = "fail: Point { x: 3, y: 4 }")]
fn test_macro_explicit_case_with_struct(tc: TestCase) {
    let p: Point = tc.draw(generators::just(Point { x: 0, y: 0 }));
    panic!("fail: {:?}", p);
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(n = vec![10i32, 20, 30].into_iter().sum::<i32>())]
#[should_panic(expected = "fail: 60")]
fn test_macro_explicit_case_with_computed_expression(tc: TestCase) {
    let n: i32 = tc.draw(generators::integers());
    panic!("fail: {}", n);
}

#[test]
fn test_explicit_output_goes_to_the_output_override() {
    use std::sync::{Arc, Mutex};
    let buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let buf_writer = buf.clone();
    let sink: Arc<dyn Fn(&str) + Send + Sync> =
        Arc::new(move |s: &str| buf_writer.lock().unwrap().push(s.to_string()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hegel::with_output_override(sink, || {
            let etc = hegel::ExplicitTestCase::new().with_value("x", "42", 42i32);
            etc.run(|tc: &hegel::ExplicitTestCase| {
                let _: i32 = tc.__draw_named(generators::integers(), "x", false);
                tc.note("a captured note");
                panic!("intentional");
            });
        });
    }));
    assert!(result.is_err());
    let lines = buf.lock().unwrap().clone();
    assert!(
        lines.iter().any(|l| l == "let x = 42;"),
        "expected the drawn-value line in the sink, got {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l == "a captured note"),
        "expected the note in the sink, got {lines:?}"
    );
}

#[hegel::test(test_cases = 1)]
#[hegel::explicit_test_case(x_1 = 1, x_2 = 2, x_3 = 4)]
fn test_macro_explicit_case_numbers_repeated_draws(tc: TestCase) {
    let mut seen = Vec::new();
    for _ in 0..3 {
        let x: i32 = tc.draw(generators::integers().min_value(1));
        seen.push(x);
    }
    assert!(seen.iter().all(|&x| x >= 1));
}

#[test]
fn test_explicit_repeated_draws_are_numbered() {
    let etc = hegel::ExplicitTestCase::new()
        .with_value("draw_1", "1", 1i32)
        .with_value("draw_2", "2", 2i32);
    etc.run(|tc: &hegel::ExplicitTestCase| {
        let first: i32 = tc.draw(generators::integers());
        let second: i32 = tc.draw(generators::integers());
        assert_eq!((first, second), (1, 2));
    });
}
