//! In-process exercise of how the engine prints a test case's document:
//! streaming a verbose run's lines as they become final, the failure report
//! kept as data on each failure, and the test-case calls that feed it.

mod common;

use common::{Printed, make_settings_no_db, next_case, ok, start_with_output, verbose_case};
use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelFailure, HegelPrinter, HegelRun, HegelRunResult, HegelSettings,
    HegelTestCase, hegel_context_free, hegel_context_new, hegel_failure_diagnostic,
    hegel_failure_free, hegel_failure_origin, hegel_failure_output,
    hegel_failure_reproduction_blob, hegel_generate_integer, hegel_mark_complete,
    hegel_next_test_case, hegel_note, hegel_output_stderr, hegel_output_stderr_callback,
    hegel_printer_end_group, hegel_printer_free, hegel_printer_resolve, hegel_printer_value,
    hegel_printer_value_result_t, hegel_run_free, hegel_run_result, hegel_run_result_failure,
    hegel_run_result_failure_count, hegel_run_result_free, hegel_run_start, hegel_settings_free,
    hegel_settings_set_seed, hegel_settings_set_test_cases, hegel_settings_set_verbosity,
    hegel_status_t, hegel_string_t, hegel_test_case_clone, hegel_test_case_free,
    hegel_test_case_from_blob, hegel_test_case_printer, hegel_test_case_wants_detail,
    hegel_test_case_wants_output, hegel_verbosity_t,
};
use std::ptr;

unsafe fn complete(ctx: *mut HegelContext, tc: *mut HegelTestCase, status: hegel_status_t) {
    ok(unsafe {
        hegel_mark_complete(
            ctx,
            tc,
            status as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        )
    });
}

/// Complete `tc` as failing at origin `o` with the given summary and detail.
unsafe fn fail(
    ctx: *mut HegelContext,
    tc: *mut HegelTestCase,
    summary: &str,
    detail: &str,
) -> hegel_c::hegel_result_t {
    let origin: hegel_string_t = "o".into();
    let summary: hegel_string_t = summary.into();
    let detail: hegel_string_t = detail.into();
    unsafe {
        hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_INTERESTING as u32,
            &origin,
            &summary,
            &detail,
        )
    }
}

unsafe fn string(s: *const hegel_string_t) -> Option<String> {
    let s = unsafe { s.as_ref() }?;
    let bytes = unsafe { std::slice::from_raw_parts(s.data.cast::<u8>(), s.len) };
    Some(String::from_utf8(bytes.to_vec()).unwrap())
}

/// Run a property that fails on every test case, each of which notes
/// `drawn` and fails with summary `it failed` and detail `it failed` /
/// `in detail`, and return the
/// result with whether any test case wanted output.
unsafe fn failing_run(
    ctx: *mut HegelContext,
    s: *mut HegelSettings,
    printed: &Printed,
) -> (*mut HegelRun, *mut HegelRunResult, bool) {
    unsafe {
        ok(hegel_settings_set_test_cases(ctx, s, 5));
        ok(hegel_settings_set_seed(ctx, s, 1, true));
        let (callback, user_data) = printed.sink();
        let run = start_with_output(ctx, s, callback, user_data);
        let mut any_wanted = false;
        loop {
            let tc = next_case(ctx, run);
            if tc.is_null() {
                break;
            }
            let mut wanted = false;
            ok(hegel_test_case_wants_output(ctx, tc, &mut wanted));
            any_wanted |= wanted;
            let mut value = 0i64;
            if hegel_generate_integer(ctx, tc, 0, 100, &mut value) != HEGEL_OK {
                complete(ctx, tc, hegel_status_t::HEGEL_STATUS_OVERRUN);
            } else {
                ok(hegel_note(ctx, tc, "drawn".into()));
                ok(fail(ctx, tc, "it failed", "it failed\nin detail"));
            }
            ok(hegel_test_case_free(ctx, tc));
        }
        let mut res: *mut HegelRunResult = ptr::null_mut();
        ok(hegel_run_result(ctx, run, &mut res));
        (run, res, any_wanted)
    }
}

unsafe fn first_failure(ctx: *mut HegelContext, res: *mut HegelRunResult) -> *mut HegelFailure {
    let mut f: *mut HegelFailure = ptr::null_mut();
    ok(unsafe { hegel_run_result_failure(ctx, res, 0, &mut f) });
    assert!(!f.is_null());
    f
}

unsafe fn failure_diagnostic(ctx: *mut HegelContext, f: *mut HegelFailure) -> Option<String> {
    let mut diagnostic: *const hegel_string_t = ptr::null();
    ok(unsafe { hegel_failure_diagnostic(ctx, f, &mut diagnostic) });
    unsafe { string(diagnostic) }
}

unsafe fn failure_output(ctx: *mut HegelContext, f: *mut HegelFailure) -> Option<String> {
    let mut output: *const hegel_string_t = ptr::null();
    ok(unsafe { hegel_failure_output(ctx, f, &mut output) });
    unsafe { string(output) }
}

#[test]
fn a_verbose_run_streams_lines_once_nothing_can_change_them() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);
        let mut wanted = false;
        ok(hegel_test_case_wants_output(ctx, tc, &mut wanted));
        assert!(wanted);

        ok(hegel_note(ctx, tc, "first".into()));
        assert_eq!(printed.take(), "first\n");

        let mut clone: *mut HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut clone));
        ok(hegel_note(ctx, clone, "in the clone".into()));
        ok(hegel_note(ctx, tc, "after".into()));
        assert_eq!(printed.take(), "", "the clone may still print");

        ok(hegel_test_case_free(ctx, clone));
        ok(hegel_note(ctx, tc, "end".into()));
        assert_eq!(printed.take(), "in the clone\nafter\nend\n");

        complete(ctx, tc, hegel_status_t::HEGEL_STATUS_VALID);
        assert_eq!(printed.take(), "");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn a_verbose_run_says_why_a_test_case_stopped() {
    for (status, reason) in [
        (
            hegel_status_t::HEGEL_STATUS_INVALID,
            "Test case stopped: failed assumption\n",
        ),
        (
            hegel_status_t::HEGEL_STATUS_OVERRUN,
            "Test case stopped: out of data\n",
        ),
    ] {
        let ctx = hegel_context_new();
        unsafe {
            let s = make_settings_no_db(ctx);
            let printed = Printed::default();
            let (run, tc) = verbose_case(ctx, s, &printed);
            complete(ctx, tc, status);
            assert_eq!(printed.take(), reason);
            ok(hegel_test_case_free(ctx, tc));
            ok(hegel_run_free(ctx, run));
            ok(hegel_settings_free(ctx, s));
            ok(hegel_context_free(ctx));
        }
    }
}

#[test]
fn a_failure_carries_what_its_test_case_printed() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, res, any_wanted) = failing_run(ctx, s, &printed);
        assert!(any_wanted, "the final replay is stamped for capture");
        assert_eq!(printed.take(), "", "a normal run prints no test case");

        let f = first_failure(ctx, res);
        assert_eq!(failure_output(ctx, f).as_deref(), Some("drawn"));
        assert_eq!(
            failure_diagnostic(ctx, f).as_deref(),
            Some("it failed\nin detail")
        );

        ok(hegel_failure_free(ctx, f));
        ok(hegel_run_result_free(ctx, res));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn a_quiet_run_prints_and_keeps_nothing() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        ok(hegel_settings_set_verbosity(
            ctx,
            s,
            hegel_verbosity_t::HEGEL_VERBOSITY_QUIET as u32,
        ));
        let printed = Printed::default();
        let (run, res, any_wanted) = failing_run(ctx, s, &printed);
        assert!(any_wanted, "the final replay still represents the failure");
        assert_eq!(printed.take(), "");
        let f = first_failure(ctx, res);
        assert_eq!(failure_output(ctx, f), None);
        assert_eq!(failure_diagnostic(ctx, f), None);

        ok(hegel_failure_free(ctx, f));
        ok(hegel_run_result_free(ctx, res));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn a_replay_from_a_blob_prints_to_its_callback() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let (run, res, _) = failing_run(ctx, s, &Printed::default());
        let f = first_failure(ctx, res);
        let mut blob: *const hegel_string_t = ptr::null();
        ok(hegel_failure_reproduction_blob(ctx, f, &mut blob));
        let blob = string(blob).unwrap();

        let printed = Printed::default();
        let (callback, user_data) = printed.sink();
        let mut tc: *mut HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_from_blob(
            ctx,
            s,
            blob.as_str().into(),
            callback,
            user_data,
            &mut tc,
        ));
        ok(hegel_note(ctx, tc, "replayed".into()));
        ok(fail(ctx, tc, "it failed", "it failed\nagain"));
        assert_eq!(printed.take(), "replayed\nit failed\nagain\n");

        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_failure_free(ctx, f));
        ok(hegel_run_result_free(ctx, res));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn a_layout_error_is_printed_in_place_of_the_document() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);
        let mut clone: *mut HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut clone));
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, clone, ptr::null(), &mut p));
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_test_case_free(ctx, clone));
        complete(ctx, tc, hegel_status_t::HEGEL_STATUS_VALID);
        let output = printed.take();
        assert!(
            output.starts_with("Failed to render this test case's printed output"),
            "{output}"
        );
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn a_test_cases_document_cannot_be_read_back() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, ptr::null(), &mut p));
        assert_eq!(hegel_printer_resolve(ctx, p), HEGEL_E_INVALID_ARG);
        assert!(common::last_error(ctx).contains("printed by the engine"));
        let mut value = hegel_printer_value_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(hegel_printer_value(ctx, p, &mut value), HEGEL_E_INVALID_ARG);
        ok(hegel_printer_free(ctx, p));
        complete(ctx, tc, hegel_status_t::HEGEL_STATUS_VALID);
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn output_calls_validate_their_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);
        assert_eq!(
            hegel_test_case_wants_output(ctx, tc, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_test_case_wants_detail(ctx, tc, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        let mut wants = false;
        assert_eq!(
            hegel_test_case_wants_detail(ctx, ptr::null(), &mut wants),
            HEGEL_E_INVALID_HANDLE
        );
        complete(ctx, tc, hegel_status_t::HEGEL_STATUS_VALID);
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));

        let mut output: *const hegel_string_t = ptr::null();
        assert_eq!(
            hegel_failure_output(ctx, ptr::null(), &mut output),
            HEGEL_E_INVALID_HANDLE
        );
        let (run, res, _) = failing_run(ctx, s, &Printed::default());
        let f = first_failure(ctx, res);
        let mut count = 0usize;
        ok(hegel_run_result_failure_count(ctx, res, &mut count));
        let mut past_end: *mut HegelFailure = ptr::null_mut();
        assert_eq!(
            hegel_run_result_failure(ctx, res, count, &mut past_end),
            HEGEL_E_INVALID_ARG
        );
        assert!(past_end.is_null());
        assert_eq!(
            hegel_failure_origin(ctx, f, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_failure_reproduction_blob(ctx, f, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_c::hegel_failure_caveat(ctx, f, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_failure_output(ctx, f, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_failure_diagnostic(ctx, f, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_failure_diagnostic(ctx, ptr::null(), &mut output),
            HEGEL_E_INVALID_HANDLE
        );
        ok(hegel_failure_free(ctx, f));
        ok(hegel_run_result_free(ctx, res));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// `hegel_mark_complete` refuses a failure without what the engine needs
/// for it, naming the argument, and a refused call completes nothing.
#[test]
fn mark_complete_rejects_incomplete_failures() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        ok(hegel_settings_set_verbosity(
            ctx,
            s,
            hegel_verbosity_t::HEGEL_VERBOSITY_DEBUG as u32,
        ));
        let printed = Printed::default();
        let (callback, user_data) = printed.sink();
        let run = start_with_output(ctx, s, callback, user_data);
        let tc = next_case(ctx, run);
        let mut wants_detail = false;
        ok(hegel_test_case_wants_detail(ctx, tc, &mut wants_detail));
        assert!(wants_detail, "a debug run wants every failure's detail");

        let interesting = hegel_status_t::HEGEL_STATUS_INTERESTING as u32;
        let origin: hegel_string_t = "o".into();
        let empty: hegel_string_t = "".into();
        let line: hegel_string_t = "boom".into();
        let lines: hegel_string_t = "boom\nagain".into();
        let invalid = [0xffu8];
        let invalid = hegel_string_t {
            data: invalid.as_ptr().cast(),
            len: 1,
        };
        let cases: [(
            *const hegel_string_t,
            *const hegel_string_t,
            *const hegel_string_t,
            &str,
        ); 7] = [
            (ptr::null(), &line, &line, "origin is required"),
            (&empty, &line, &line, "origin is required"),
            (&origin, ptr::null(), &line, "summary is required"),
            (&origin, &empty, &line, "summary is required"),
            (&origin, &lines, &line, "summary must be a single line"),
            (&origin, &line, ptr::null(), "detail is required"),
            (&origin, &invalid, &line, "summary is not valid UTF-8"),
        ];
        for (o, sm, d, message) in cases {
            assert_eq!(
                hegel_mark_complete(ctx, tc, interesting, o, sm, d),
                HEGEL_E_INVALID_ARG,
                "{message}"
            );
            assert!(
                common::last_error(ctx).contains(message),
                "{}",
                common::last_error(ctx)
            );
        }
        assert_eq!(
            hegel_mark_complete(
                ctx,
                tc,
                hegel_status_t::HEGEL_STATUS_VALID as u32,
                &origin,
                ptr::null(),
                ptr::null()
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(common::last_error(ctx).contains("must be NULL unless"));
        assert_eq!(
            hegel_mark_complete(ctx, tc, 99, ptr::null(), ptr::null(), ptr::null()),
            HEGEL_E_INVALID_ARG
        );
        let mut next: *mut HegelTestCase = ptr::null_mut();
        assert_eq!(
            hegel_next_test_case(ctx, run, &mut next),
            HEGEL_E_NOT_COMPLETE,
            "a refused call completes nothing"
        );

        printed.take();
        ok(fail(ctx, tc, "boom", "boom\nwith its trace"));
        assert_eq!(
            printed.take(),
            "boom\nwith its trace\n",
            "debug prints the detail"
        );
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// A verbose run prints a failing test case's summary, and does not want
/// its detail unless the case is stamped for capture.
#[test]
fn a_verbose_run_prints_the_summary_of_a_failure() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);
        let mut wants_detail = true;
        ok(hegel_test_case_wants_detail(ctx, tc, &mut wants_detail));
        assert!(!wants_detail);
        let origin: hegel_string_t = "o".into();
        let summary: hegel_string_t = "t.rs:1:2: boom".into();
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_INTERESTING as u32,
            &origin,
            &summary,
            ptr::null(),
        ));
        assert_eq!(printed.take(), "t.rs:1:2: boom\n");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// With no output destination, a verbose run prints nothing and renders as
/// a normal run does, but the failure report keeps its data.
#[test]
fn a_run_without_a_destination_wants_only_what_the_report_needs() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        ok(hegel_settings_set_verbosity(
            ctx,
            s,
            hegel_verbosity_t::HEGEL_VERBOSITY_VERBOSE as u32,
        ));
        ok(hegel_settings_set_test_cases(ctx, s, 5));
        ok(hegel_settings_set_seed(ctx, s, 1, true));
        let mut run: *mut HegelRun = ptr::null_mut();
        ok(hegel_run_start(ctx, s, None, ptr::null_mut(), &mut run));
        let mut wanted = Vec::new();
        loop {
            let tc = next_case(ctx, run);
            if tc.is_null() {
                break;
            }
            let mut w = false;
            ok(hegel_test_case_wants_output(ctx, tc, &mut w));
            wanted.push(w);
            let mut value = 0i64;
            if hegel_generate_integer(ctx, tc, 0, 100, &mut value) != HEGEL_OK {
                complete(ctx, tc, hegel_status_t::HEGEL_STATUS_OVERRUN);
            } else {
                ok(hegel_note(ctx, tc, "drawn".into()));
                ok(fail(ctx, tc, "it failed", "it failed\nin detail"));
            }
            ok(hegel_test_case_free(ctx, tc));
        }
        assert!(
            !wanted[0],
            "an ordinary test case of a run with no destination"
        );
        assert!(wanted.iter().any(|w| *w), "the stamped final replay");
        let mut res: *mut HegelRunResult = ptr::null_mut();
        ok(hegel_run_result(ctx, run, &mut res));
        let f = first_failure(ctx, res);
        assert_eq!(failure_output(ctx, f).as_deref(), Some("drawn"));
        assert_eq!(
            failure_diagnostic(ctx, f).as_deref(),
            Some("it failed\nin detail")
        );
        ok(hegel_failure_free(ctx, f));
        ok(hegel_run_result_free(ctx, res));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// `hegel_output_stderr` is the explicit stderr destination, and the
/// callback getter hands out the same function.
#[test]
fn stderr_is_an_explicit_destination() {
    let getter = hegel_output_stderr_callback().unwrap();
    assert!(std::ptr::fn_addr_eq(
        getter,
        hegel_output_stderr as unsafe extern "C" fn(_, _, _)
    ));
    let line = "written by hegel_output_stderr";
    unsafe {
        hegel_output_stderr(ptr::null_mut(), line.as_ptr().cast(), line.len());
        hegel_output_stderr(ptr::null_mut(), ptr::null(), 0);
    }
}

/// Each failure the run reports is the last test case of its origin to run,
/// and that test case wanted output: a frontend raising a failure's own
/// exception can keep, per origin, the one from the latest failing test case
/// that wanted output and be sure it is the one the report describes.
#[test]
fn a_reported_failure_is_the_last_test_case_of_its_origin() {
    for (verbosity, name) in [
        (hegel_verbosity_t::HEGEL_VERBOSITY_QUIET, "quiet"),
        (hegel_verbosity_t::HEGEL_VERBOSITY_NORMAL, "normal"),
        (hegel_verbosity_t::HEGEL_VERBOSITY_VERBOSE, "verbose"),
        (hegel_verbosity_t::HEGEL_VERBOSITY_DEBUG, "debug"),
    ] {
        let ctx = hegel_context_new();
        unsafe {
            let s = make_settings_no_db(ctx);
            ok(hegel_settings_set_verbosity(ctx, s, verbosity as u32));
            ok(hegel_settings_set_test_cases(ctx, s, 100));
            ok(hegel_settings_set_seed(ctx, s, 3, true));
            ok(hegel_c::hegel_settings_set_report_multiple_failures(
                ctx, s, true,
            ));
            let printed = Printed::default();
            let (callback, user_data) = printed.sink();
            let run = start_with_output(ctx, s, callback, user_data);
            let mut last = std::collections::HashMap::<String, (i64, bool, usize)>::new();
            loop {
                let tc = next_case(ctx, run);
                if tc.is_null() {
                    break;
                }
                let mut wanted = false;
                ok(hegel_test_case_wants_output(ctx, tc, &mut wanted));
                let mut x = 0i64;
                if hegel_generate_integer(ctx, tc, 0, 1000, &mut x) != HEGEL_OK {
                    complete(ctx, tc, hegel_status_t::HEGEL_STATUS_OVERRUN);
                } else if x < 10 {
                    complete(ctx, tc, hegel_status_t::HEGEL_STATUS_VALID);
                } else {
                    let origin = if x % 2 == 0 { "even" } else { "odd" };
                    ok(hegel_note(ctx, tc, format!("x = {x}").as_str().into()));
                    let origin_s: hegel_string_t = origin.into();
                    let text = format!("{origin} {x}");
                    let summary: hegel_string_t = text.as_str().into();
                    let detail: hegel_string_t = text.as_str().into();
                    ok(hegel_mark_complete(
                        ctx,
                        tc,
                        hegel_status_t::HEGEL_STATUS_INTERESTING as u32,
                        &origin_s,
                        &summary,
                        &detail,
                    ));
                    let runs = last.get(origin).map_or(0, |entry| entry.2);
                    last.insert(origin.to_string(), (x, wanted, runs + 1));
                }
                ok(hegel_test_case_free(ctx, tc));
            }
            let mut res: *mut HegelRunResult = ptr::null_mut();
            ok(hegel_run_result(ctx, run, &mut res));
            let mut count = 0usize;
            ok(hegel_run_result_failure_count(ctx, res, &mut count));
            assert_eq!(count, 2, "{name}: both origins are reported");
            for index in 0..count {
                let mut f: *mut HegelFailure = ptr::null_mut();
                ok(hegel_run_result_failure(ctx, res, index, &mut f));
                let mut origin = hegel_string_t::default();
                ok(hegel_failure_origin(ctx, f, &mut origin));
                let origin = string(&origin).unwrap();
                let (x, wanted, runs) = last[&origin];
                assert!(runs > 1, "{name}: several {origin} test cases ran");
                assert!(wanted, "{name}: the last {origin} test case wanted output");
                if name != "quiet" {
                    assert_eq!(
                        failure_output(ctx, f),
                        Some(format!("x = {x}")),
                        "{name}: the report is the last {origin} test case"
                    );
                    assert_eq!(failure_diagnostic(ctx, f), Some(format!("{origin} {x}")));
                }
                ok(hegel_failure_free(ctx, f));
            }
            ok(hegel_run_result_free(ctx, res));
            ok(hegel_run_free(ctx, run));
            ok(hegel_settings_free(ctx, s));
            ok(hegel_context_free(ctx));
        }
    }
}
