//! In-process exercise of the `hegel_printer_*` / `hegel_note` C ABI: the
//! layout primitives, deferred slots, speculative regions, the per-family
//! document, and every argument-validation path.

mod common;

use common::{Printed, last_error, make_settings_no_db, next_case, ok, start, verbose_case};
use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelPrinter, HegelPrinterOptions, hegel_context_free, hegel_context_new,
    hegel_generate_integer, hegel_mark_complete, hegel_note, hegel_printer_abort_speculative,
    hegel_printer_begin_group, hegel_printer_begin_speculative, hegel_printer_breakable,
    hegel_printer_comment, hegel_printer_commit_speculative, hegel_printer_deferred,
    hegel_printer_end_group, hegel_printer_free, hegel_printer_hard_break, hegel_printer_if_break,
    hegel_printer_is_live, hegel_printer_new, hegel_printer_options_free,
    hegel_printer_options_new, hegel_printer_options_set_max_width, hegel_printer_reflow,
    hegel_printer_resolve, hegel_printer_shift_indent, hegel_printer_text, hegel_printer_value,
    hegel_printer_value_result_free, hegel_printer_value_result_t, hegel_reflow_options_free,
    hegel_reflow_options_new, hegel_run_free, hegel_settings_free, hegel_status_t,
    hegel_test_case_block, hegel_test_case_clone, hegel_test_case_free, hegel_test_case_printer,
    hegel_test_case_set_worker,
};
use std::ptr;

unsafe fn max_width_options(ctx: *mut HegelContext, max_width: u64) -> *mut HegelPrinterOptions {
    let mut options: *mut HegelPrinterOptions = ptr::null_mut();
    ok(unsafe { hegel_printer_options_new(ctx, &mut options) });
    assert!(!options.is_null());
    ok(unsafe { hegel_printer_options_set_max_width(ctx, options, max_width) });
    options
}

unsafe fn new_printer(ctx: *mut HegelContext, max_width: u64) -> *mut HegelPrinter {
    let options = unsafe { max_width_options(ctx, max_width) };
    let mut p: *mut HegelPrinter = ptr::null_mut();
    ok(unsafe { hegel_printer_new(ctx, options, &mut p) });
    ok(unsafe { hegel_printer_options_free(ctx, options) });
    assert!(!p.is_null());
    p
}

unsafe fn text(ctx: *mut HegelContext, p: *mut HegelPrinter, s: &str) {
    ok(unsafe {
        hegel_printer_text(
            ctx,
            p,
            hegel_c::hegel_string_t {
                data: (s.as_ptr()).cast(),
                len: s.len(),
            },
        )
    });
}

unsafe fn value(ctx: *mut HegelContext, p: *mut HegelPrinter) -> String {
    let mut result = hegel_printer_value_result_t {
        data: ptr::null_mut(),
        len: 0,
    };
    ok(unsafe { hegel_printer_value(ctx, p, &mut result) });
    assert!(!result.data.is_null());
    assert_eq!(unsafe { *result.data.add(result.len) }, 0);
    let bytes =
        unsafe { std::slice::from_raw_parts(result.data.cast::<u8>(), result.len) }.to_vec();
    ok(unsafe { hegel_printer_value_result_free(ctx, &mut result) });
    assert!(result.data.is_null());
    assert_eq!(result.len, 0);
    String::from_utf8(bytes).unwrap()
}

#[test]
fn groups_lay_out_inline_or_broken() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 20);
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "1,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "2");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        assert_eq!(value(ctx, p), "[1, 2]");
        ok(hegel_printer_free(ctx, p));

        let p = new_printer(ctx, 6);
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "1,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "2,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "3");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        assert_eq!(value(ctx, p), "[1,\n 2,\n 3]");
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn shift_indent_applies_to_hard_breaks_and_clamps() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        text(ctx, p, "a");
        ok(hegel_printer_shift_indent(ctx, p, 4));
        ok(hegel_printer_hard_break(ctx, p));
        text(ctx, p, "b");
        ok(hegel_printer_shift_indent(ctx, p, i64::MIN));
        ok(hegel_printer_hard_break(ctx, p));
        text(ctx, p, "c");
        assert_eq!(value(ctx, p), "a\n    b\nc");
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn deferred_slot_roundtrip_and_death() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        text(ctx, p, "a");
        let mut slot: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_printer_deferred(ctx, p, &mut slot));
        assert!(!slot.is_null());
        text(ctx, p, "c");

        let mut live = false;
        ok(hegel_printer_is_live(ctx, p, &mut live));
        assert!(live);
        ok(hegel_printer_is_live(ctx, slot, &mut live));
        assert!(live);

        let mut result = hegel_printer_value_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            hegel_printer_value(ctx, p, &mut result),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unresolved deferred"));

        text(ctx, slot, "b");
        ok(hegel_printer_resolve(ctx, p));
        assert_eq!(value(ctx, p), "abc");

        ok(hegel_printer_is_live(ctx, slot, &mut live));
        assert!(!live);
        ok(hegel_printer_is_live(ctx, p, &mut live));
        assert!(!live, "resolve seals the main output too");
        assert_eq!(
            hegel_printer_text(ctx, slot, "x".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("session ended"));
        assert_eq!(
            hegel_printer_breakable(ctx, slot, " ".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_if_break(ctx, slot, ",".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(hegel_printer_hard_break(ctx, slot), HEGEL_E_INVALID_HANDLE);
        assert_eq!(
            hegel_printer_begin_group(ctx, slot, 0, "".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_end_group(ctx, slot, "".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_shift_indent(ctx, slot, 1),
            HEGEL_E_INVALID_HANDLE
        );
        let mut grandchild: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_printer_deferred(ctx, slot, &mut grandchild),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_begin_speculative(ctx, slot),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_commit_speculative(ctx, slot),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_abort_speculative(ctx, slot),
            HEGEL_E_INVALID_HANDLE
        );

        ok(hegel_printer_free(ctx, slot));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn resolve_and_value_reject_slot_handles() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        let mut slot: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_printer_deferred(ctx, p, &mut slot));
        assert_eq!(hegel_printer_resolve(ctx, slot), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("root handle"));
        let mut result = hegel_printer_value_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            hegel_printer_value(ctx, slot, &mut result),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("root handle"));
        ok(hegel_printer_resolve(ctx, p));
        assert_eq!(hegel_printer_resolve(ctx, p), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("no outstanding"));
        ok(hegel_printer_free(ctx, slot));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn speculative_regions_commit_and_abort() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        text(ctx, p, "a");
        assert_eq!(
            hegel_printer_commit_speculative(ctx, p),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("without an open speculative region"));
        assert_eq!(hegel_printer_abort_speculative(ctx, p), HEGEL_E_INVALID_ARG);
        ok(hegel_printer_begin_speculative(ctx, p));
        text(ctx, p, "b");
        ok(hegel_printer_abort_speculative(ctx, p));
        ok(hegel_printer_begin_speculative(ctx, p));
        text(ctx, p, "c");
        ok(hegel_printer_commit_speculative(ctx, p));
        assert_eq!(value(ctx, p), "ac");

        assert_eq!(
            hegel_printer_commit_speculative(ctx, p),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("printing session ended"));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn comments_attach_to_line_ends_and_break_open_groups() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "1,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "2");
        let comment = "  // or any other generated value";
        ok(hegel_printer_comment(
            ctx,
            p,
            hegel_c::hegel_string_t {
                data: (comment.as_ptr()).cast(),
                len: comment.len(),
            },
        ));
        text(ctx, p, ",");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "3");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        assert_eq!(
            value(ctx, p),
            "[1,\n 2,  // or any other generated value\n 3\n]"
        );
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn comment_arguments_are_validated() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        assert_eq!(
            hegel_printer_comment(ctx, p, "a\nb".into()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("must not contain newlines"));
        assert_eq!(
            hegel_printer_comment(
                ctx,
                p,
                hegel_c::hegel_string_t {
                    data: ptr::null(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("text is null"));
        ok(hegel_printer_comment(ctx, p, "".into()));

        let mut slot: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_printer_deferred(ctx, p, &mut slot));
        ok(hegel_printer_resolve(ctx, p));
        assert_eq!(
            hegel_printer_comment(ctx, slot, "x".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("session ended"));

        let null: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_printer_comment(ctx, null, "x".into()),
            HEGEL_E_INVALID_HANDLE
        );
        ok(hegel_printer_free(ctx, slot));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn text_arguments_are_validated() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        assert_eq!(
            hegel_printer_text(
                ctx,
                p,
                hegel_c::hegel_string_t {
                    data: ([0xff_u8].as_ptr()).cast(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("not valid UTF-8"));
        assert_eq!(
            hegel_printer_text(ctx, p, "a\nb".into()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("must not contain newlines"));
        assert_eq!(
            hegel_printer_text(
                ctx,
                p,
                hegel_c::hegel_string_t {
                    data: ptr::null(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("text is null"));
        ok(hegel_printer_text(ctx, p, "".into()));
        assert_eq!(
            hegel_printer_breakable(ctx, p, "\n".into()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_begin_group(ctx, p, 0, "\n".into()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_end_group(ctx, p, "\n".into()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_end_group(ctx, p, "".into()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("without a matching begin_group"));
        assert_eq!(value(ctx, p), "");
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

// The gofmt-style layout the if_break primitive exists for: broken form
// with a trailing comma and the close brace on its own line, flat form
// without either.
#[test]
fn if_break_emits_trailing_text_only_in_the_broken_form() {
    let ctx = hegel_context_new();
    unsafe {
        for (max_width, expected) in [(79, "{1, 2}"), (5, "{\n    1,\n    2,\n}")] {
            let p = new_printer(ctx, max_width);
            ok(hegel_printer_begin_group(ctx, p, 0, "{".into()));
            ok(hegel_printer_shift_indent(ctx, p, 4));
            ok(hegel_printer_breakable(ctx, p, "".into()));
            text(ctx, p, "1,");
            ok(hegel_printer_breakable(ctx, p, " ".into()));
            text(ctx, p, "2");
            ok(hegel_printer_shift_indent(ctx, p, -4));
            ok(hegel_printer_if_break(ctx, p, ",".into()));
            ok(hegel_printer_breakable(ctx, p, "".into()));
            ok(hegel_printer_end_group(ctx, p, "}".into()));
            assert_eq!(value(ctx, p), expected);
            ok(hegel_printer_free(ctx, p));
        }
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn if_break_rejects_bad_text() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 79);
        assert_eq!(
            hegel_printer_if_break(
                ctx,
                p,
                hegel_c::hegel_string_t {
                    data: ([0xff_u8].as_ptr()).cast(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_printer_if_break"));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn null_handles_and_out_parameters_are_rejected() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_printer_new(ctx, ptr::null(), ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_options_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_options_set_max_width(ctx, ptr::null_mut(), 10),
            HEGEL_E_INVALID_HANDLE
        );
        ok(hegel_printer_options_free(ctx, ptr::null_mut()));
        let null: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_printer_text(ctx, null, "a".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_breakable(ctx, null, " ".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_if_break(ctx, null, ",".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(hegel_printer_hard_break(ctx, null), HEGEL_E_INVALID_HANDLE);
        assert_eq!(
            hegel_printer_begin_group(ctx, null, 0, "".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_end_group(ctx, null, "".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_shift_indent(ctx, null, 1),
            HEGEL_E_INVALID_HANDLE
        );
        let mut slot: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_printer_deferred(ctx, null, &mut slot),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_begin_speculative(ctx, null),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_commit_speculative(ctx, null),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_abort_speculative(ctx, null),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(hegel_printer_resolve(ctx, null), HEGEL_E_INVALID_HANDLE);
        let mut live = false;
        assert_eq!(
            hegel_printer_is_live(ctx, null, &mut live),
            HEGEL_E_INVALID_HANDLE
        );
        let mut result = hegel_printer_value_result_t {
            data: ptr::null_mut(),
            len: 0,
        };
        assert_eq!(
            hegel_printer_value(ctx, null, &mut result),
            HEGEL_E_INVALID_HANDLE
        );

        let p = new_printer(ctx, 79);
        assert_eq!(
            hegel_printer_deferred(ctx, p, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_is_live(ctx, p, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_printer_value(ctx, p, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        ok(hegel_printer_free(ctx, p));
        ok(hegel_printer_free(ctx, ptr::null_mut()));
        ok(hegel_printer_value_result_free(ctx, ptr::null_mut()));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn family_document_is_shared_and_survives_completion() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        ok(hegel_note(ctx, tc, "note one\nsecond line".into()));

        let mut p1: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, ptr::null(), &mut p1));
        text(ctx, p1, "let x = 1;");
        ok(hegel_printer_hard_break(ctx, p1));

        let restated = max_width_options(ctx, 79);
        let mut p_restated: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, restated, &mut p_restated));
        ok(hegel_printer_free(ctx, p_restated));
        ok(hegel_printer_options_free(ctx, restated));

        let conflicting = max_width_options(ctx, 40);
        let mut p_conflicting: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_test_case_printer(ctx, tc, conflicting, &mut p_conflicting),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("max_width 79 and cannot change to 40"));
        ok(hegel_printer_options_free(ctx, conflicting));

        let mut clone: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut clone));
        let mut p2: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, clone, ptr::null(), &mut p2));
        text(ctx, p2, "from clone");

        ok(hegel_note(ctx, tc, "".into()));

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));

        assert_eq!(
            printed.take(),
            "note one\nsecond line\nlet x = 1;\nfrom clone\n"
        );

        ok(hegel_printer_free(ctx, p1));
        ok(hegel_printer_free(ctx, p2));
        ok(hegel_test_case_free(ctx, clone));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn note_during_a_speculative_draw_lands_after_the_value() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, ptr::null(), &mut p));
        ok(hegel_printer_begin_speculative(ctx, p));
        text(ctx, p, "let x = ");
        ok(hegel_note(ctx, tc, "mid-draw".into()));
        text(ctx, p, "5;");
        ok(hegel_printer_hard_break(ctx, p));
        ok(hegel_printer_commit_speculative(ctx, p));

        ok(hegel_printer_begin_speculative(ctx, p));
        text(ctx, p, "let y = ");
        ok(hegel_note(ctx, tc, "rejected".into()));
        ok(hegel_printer_abort_speculative(ctx, p));

        ok(hegel_printer_free(ctx, p));
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(printed.take(), "let x = 5;\nmid-draw\nrejected\n");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn clone_regions_anchor_where_the_clone_was_made() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        ok(hegel_note(ctx, tc, "before".into()));
        let mut clone: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut clone));
        ok(hegel_note(ctx, tc, "after".into()));
        ok(hegel_note(ctx, clone, "inside".into()));

        let mut grandchild: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, clone, &mut grandchild));
        ok(hegel_note(ctx, clone, "inside two".into()));
        ok(hegel_note(ctx, grandchild, "nested".into()));

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(
            printed.take(),
            "before\ninside\nnested\ninside two\nafter\n"
        );
        ok(hegel_test_case_free(ctx, grandchild));
        ok(hegel_test_case_free(ctx, clone));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// A printer fetched from a clone keeps writing into the clone's region
/// after the clone itself is freed.
#[test]
fn a_printer_outlives_the_test_case_it_was_fetched_from() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let mut clone: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut clone));
        let mut printer: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(
            ctx,
            clone,
            ptr::null(),
            &mut printer,
        ));
        ok(hegel_test_case_free(ctx, clone));
        text(ctx, printer, "late");
        ok(hegel_printer_free(ctx, printer));

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(printed.take(), "late\n");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn first_explicit_width_configures_the_family_document() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let mut unconfigured: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(
            ctx,
            tc,
            ptr::null(),
            &mut unconfigured,
        ));
        text(ctx, unconfigured, "x = ");
        ok(hegel_printer_free(ctx, unconfigured));

        let narrow = max_width_options(ctx, 10);
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, narrow, &mut p));
        ok(hegel_printer_options_free(ctx, narrow));

        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "aaaa,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "bbbb");
        ok(hegel_printer_end_group(ctx, p, "]".into()));

        let conflicting = max_width_options(ctx, 79);
        let mut p2: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_test_case_printer(ctx, tc, conflicting, &mut p2),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("max_width 10 and cannot change to 79"));
        ok(hegel_printer_options_free(ctx, conflicting));

        ok(hegel_printer_free(ctx, p));
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(printed.take(), "x = [aaaa,\n bbbb]\n");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn the_width_is_fixed_once_the_engine_has_printed() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        ok(hegel_note(ctx, tc, "printed at the default width".into()));
        assert_eq!(printed.take(), "printed at the default width\n");

        let narrow = max_width_options(ctx, 10);
        let mut p: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_test_case_printer(ctx, tc, narrow, &mut p),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("max_width 79 and cannot change to 10"));
        ok(hegel_printer_options_free(ctx, narrow));

        let restated = max_width_options(ctx, 79);
        ok(hegel_test_case_printer(ctx, tc, restated, &mut p));
        ok(hegel_printer_options_free(ctx, restated));
        ok(hegel_printer_free(ctx, p));

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn note_and_test_case_printer_validate_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_note(ctx, ptr::null_mut(), "x".into()),
            HEGEL_E_INVALID_HANDLE
        );
        let mut p: *mut HegelPrinter = ptr::null_mut();
        assert_eq!(
            hegel_test_case_printer(ctx, ptr::null_mut(), ptr::null(), &mut p),
            HEGEL_E_INVALID_HANDLE
        );

        let s = make_settings_no_db(ctx);
        let run = start(ctx, s);
        let tc = next_case(ctx, run);
        assert!(!tc.is_null());
        assert_eq!(
            hegel_test_case_printer(ctx, tc, ptr::null(), ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_note(
                ctx,
                tc,
                hegel_c::hegel_string_t {
                    data: ([0xff_u8].as_ptr()).cast(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("not valid UTF-8"));
        assert_eq!(
            hegel_note(
                ctx,
                tc,
                hegel_c::hegel_string_t {
                    data: ptr::null(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("text is null"));

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn null_options_lay_out_to_the_default_width_of_79() {
    let ctx = hegel_context_new();
    unsafe {
        for (element_width, expected_break) in [(74, false), (75, true)] {
            let mut p: *mut HegelPrinter = ptr::null_mut();
            ok(hegel_printer_new(ctx, ptr::null(), &mut p));
            ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
            let element = "a".repeat(element_width);
            text(ctx, p, &element);
            text(ctx, p, ",");
            ok(hegel_printer_breakable(ctx, p, " ".into()));
            text(ctx, p, "b");
            ok(hegel_printer_end_group(ctx, p, "]".into()));
            assert_eq!(value(ctx, p).contains('\n'), expected_break);
            ok(hegel_printer_free(ctx, p));
        }
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn reflow_lays_out_a_flat_representation_through_the_groups() {
    let ctx = hegel_context_new();
    unsafe {
        let p = new_printer(ctx, 12);
        text(ctx, p, "let p = ");
        let repr = "Point { x: 100, y: 200 }";
        ok(hegel_printer_reflow(
            ctx,
            p,
            ptr::null(),
            hegel_c::hegel_string_t {
                data: (repr.as_ptr()).cast(),
                len: repr.len(),
            },
        ));
        text(ctx, p, ";");
        assert_eq!(value(ctx, p), "let p = Point {\n    x: 100,\n    y: 200 };");
        ok(hegel_printer_free(ctx, p));

        let mut options: *mut hegel_c::HegelReflowOptions = ptr::null_mut();
        ok(hegel_reflow_options_new(ctx, &mut options));
        assert!(!options.is_null());
        let p = new_printer(ctx, 79);
        let repr = "line one\nline two";
        ok(hegel_printer_reflow(
            ctx,
            p,
            options,
            hegel_c::hegel_string_t {
                data: (repr.as_ptr()).cast(),
                len: repr.len(),
            },
        ));
        ok(hegel_printer_reflow(ctx, p, options, "".into()));
        assert_eq!(value(ctx, p), "line one\nline two");
        ok(hegel_printer_free(ctx, p));
        ok(hegel_reflow_options_free(ctx, options));
        ok(hegel_reflow_options_free(ctx, ptr::null_mut()));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn reflow_validates_its_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_reflow_options_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("out parameter is null"));
        assert_eq!(
            hegel_printer_reflow(ctx, ptr::null_mut(), ptr::null(), "x".into()),
            HEGEL_E_INVALID_HANDLE
        );
        let p = new_printer(ctx, 79);
        assert_eq!(
            hegel_printer_reflow(
                ctx,
                p,
                ptr::null(),
                hegel_c::hegel_string_t {
                    data: ([0xff_u8].as_ptr()).cast(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("not valid UTF-8"));
        assert_eq!(
            hegel_printer_reflow(
                ctx,
                p,
                ptr::null(),
                hegel_c::hegel_string_t {
                    data: ptr::null(),
                    len: 1
                }
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("text is null"));

        let mut slot: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_printer_deferred(ctx, p, &mut slot));
        ok(hegel_printer_resolve(ctx, p));
        assert_eq!(
            hegel_printer_reflow(ctx, slot, ptr::null(), "[1, 2]".into()),
            HEGEL_E_INVALID_HANDLE
        );
        ok(hegel_printer_free(ctx, slot));
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn options_reject_a_zero_max_width() {
    let ctx = hegel_context_new();
    unsafe {
        let mut options: *mut HegelPrinterOptions = ptr::null_mut();
        ok(hegel_printer_options_new(ctx, &mut options));
        assert_eq!(
            hegel_printer_options_set_max_width(ctx, options, 0),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("max_width must be positive"));
        ok(hegel_printer_options_set_max_width(ctx, options, 5));
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_printer_new(ctx, options, &mut p));
        ok(hegel_printer_options_free(ctx, options));
        text(ctx, p, "aaa,");
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "b");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        assert_eq!(value(ctx, p), "aaa,[\n b]");
        ok(hegel_printer_free(ctx, p));
        ok(hegel_context_free(ctx));
    }
}

/// Completing a test case seals its document: a straggler's later writes
/// are rejected and never printed.
#[test]
fn writes_after_completion_are_rejected() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let mut straggler: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut straggler));
        let mut late: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(
            ctx,
            straggler,
            ptr::null(),
            &mut late,
        ));
        let mut root: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, ptr::null(), &mut root));
        text(ctx, root, "report");

        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(printed.take(), "report\n");
        assert_eq!(
            hegel_printer_text(ctx, late, "late".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_printer_text(ctx, root, "later".into()),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(printed.take(), "");

        ok(hegel_printer_free(ctx, late));
        ok(hegel_printer_free(ctx, root));
        ok(hegel_test_case_free(ctx, straggler));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

/// A clone made inside a speculative region that is then aborted has a
/// dead region; the clones and blocks made from it share it, and their
/// writes are rejected.
#[test]
fn clones_of_a_dead_region_share_it() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let mut root: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, ptr::null(), &mut root));
        ok(hegel_printer_begin_speculative(ctx, root));
        let mut aborted: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut aborted));
        ok(hegel_printer_abort_speculative(ctx, root));

        let mut clone: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, aborted, &mut clone));
        let mut block: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_block(ctx, aborted, 2, &mut block));
        for handle in [aborted, clone, block] {
            assert_eq!(
                hegel_note(ctx, handle, "lost".into()),
                HEGEL_E_INVALID_HANDLE
            );
        }
        ok(hegel_test_case_free(ctx, aborted));
        ok(hegel_test_case_free(ctx, clone));
        ok(hegel_test_case_free(ctx, block));
        ok(hegel_note(ctx, tc, "kept".into()));

        ok(hegel_printer_free(ctx, root));
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(printed.take(), "kept\n");
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn block_handles_indent_their_region_and_share_the_stream() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let narrow = max_width_options(ctx, 10);
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, narrow, &mut p));
        ok(hegel_printer_free(ctx, p));

        ok(hegel_note(ctx, tc, "Step 1: add {".into()));
        let mut block: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_block(ctx, tc, 2, &mut block));
        assert!(!block.is_null());
        ok(hegel_note(ctx, block, "inside".into()));

        let mut drawn: i64 = 7;
        ok(hegel_generate_integer(ctx, block, 0, 0, &mut drawn));
        assert_eq!(drawn, 0);

        ok(hegel_test_case_printer(ctx, block, narrow, &mut p));
        ok(hegel_printer_options_free(ctx, narrow));
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "aaaa,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "bbbb");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        ok(hegel_printer_hard_break(ctx, p));

        let mut nested: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_block(ctx, block, 2, &mut nested));
        ok(hegel_note(ctx, nested, "deep".into()));
        let mut clone: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, block, &mut clone));
        ok(hegel_note(ctx, clone, "cloned".into()));
        ok(hegel_note(ctx, tc, "}".into()));

        assert_eq!(
            hegel_test_case_block(ctx, ptr::null(), 2, &mut nested),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_test_case_block(ctx, tc, 2, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("out parameter is null"));

        ok(hegel_printer_free(ctx, p));
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        assert_eq!(
            printed.take(),
            "Step 1: add {\n  inside\n  [aaaa,\n   bbbb]\n    deep\n  cloned\n}\n"
        );
        ok(hegel_test_case_free(ctx, clone));
        ok(hegel_test_case_free(ctx, nested));
        ok(hegel_test_case_free(ctx, block));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn worker_attribution_stamps_lines_recorded_through_the_handle() {
    let ctx = hegel_context_new();
    unsafe {
        let s = make_settings_no_db(ctx);
        let printed = Printed::default();
        let (run, tc) = verbose_case(ctx, s, &printed);

        let narrow = max_width_options(ctx, 30);
        let mut root: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, tc, narrow, &mut root));
        ok(hegel_printer_options_free(ctx, narrow));

        let mut worker: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, tc, &mut worker));
        ok(hegel_test_case_set_worker(ctx, worker, 3));
        ok(hegel_note(ctx, worker, "Rule: boom {\nsecond".into()));

        let mut block: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_block(ctx, worker, 2, &mut block));
        let mut p: *mut HegelPrinter = ptr::null_mut();
        ok(hegel_test_case_printer(ctx, block, ptr::null(), &mut p));
        ok(hegel_printer_begin_group(ctx, p, 1, "[".into()));
        text(ctx, p, "aaaa,");
        ok(hegel_printer_breakable(ctx, p, " ".into()));
        text(ctx, p, "bbbb");
        ok(hegel_printer_end_group(ctx, p, "]".into()));
        ok(hegel_printer_hard_break(ctx, p));
        let mut grandchild: *mut hegel_c::HegelTestCase = ptr::null_mut();
        ok(hegel_test_case_clone(ctx, block, &mut grandchild));
        ok(hegel_note(ctx, grandchild, "nested".into()));
        ok(hegel_note(ctx, worker, "}".into()));

        ok(hegel_note(ctx, tc, "unattributed".into()));
        ok(hegel_test_case_set_worker(ctx, tc, 0));
        text(ctx, root, "root line");
        ok(hegel_printer_hard_break(ctx, root));

        assert_eq!(
            hegel_test_case_set_worker(ctx, ptr::null(), 0),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(hegel_test_case_set_worker(ctx, tc, -1), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("worker_index must be non-negative"));

        ok(hegel_printer_free(ctx, p));
        ok(hegel_printer_free(ctx, root));
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
            ptr::null(),
            ptr::null(),
        ));
        let rendered = printed.take();
        let stamp = r"\[worker 3 \+\d+\.\d{3}ms\] ";
        let expected = format!(
            "^{stamp}Rule: boom \\{{\n{stamp}second\n{stamp}  \\[aaaa,\n   bbbb\\]\n{stamp}  nested\n{stamp}\\}}\nunattributed\n\\[worker 0 \\+\\d+\\.\\d{{3}}ms\\] root line\n$"
        );
        assert!(
            regex::Regex::new(&expected).unwrap().is_match(&rendered),
            "unexpected document:\n{rendered}"
        );
        ok(hegel_test_case_free(ctx, grandchild));
        ok(hegel_test_case_free(ctx, block));
        ok(hegel_test_case_free(ctx, worker));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, s));
        ok(hegel_context_free(ctx));
    }
}
