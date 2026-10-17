//! In-process exercise of the time zone and zoned datetime draws over the C
//! ABI: spec construction and argument checking, and the values a frontend
//! reads back. The draws themselves are checked against jiff in the
//! embedded `timezone_tests` and `zoned_tests`.

mod common;

use common::{last_error, make_settings_no_db, next_case, ok, start};
use hegel_c::hegel_result_t::*;
use hegel_c::hegel_timezone_kind_t::*;
use hegel_c::{
    HegelContext, HegelRun, HegelTestCase, HegelTimezoneSpec, HegelZonedSpec, hegel_ambiguous_t,
    hegel_context_free, hegel_context_new, hegel_date_t, hegel_datetime_t,
    hegel_generate_string_result_free, hegel_generate_string_result_t, hegel_generate_timezone,
    hegel_generate_zoned_datetime, hegel_imaginary_t, hegel_mark_complete, hegel_precision_t,
    hegel_settings_set_test_cases, hegel_status_t, hegel_time_t, hegel_timestamp_t,
    hegel_timezone_spec_free, hegel_timezone_spec_new, hegel_timezone_spec_set_legacy,
    hegel_timezone_spec_set_links, hegel_timezone_spec_set_names, hegel_timezone_spec_set_opaque,
    hegel_timezone_spec_set_unknown, hegel_zoned_datetime_t, hegel_zoned_spec_free,
    hegel_zoned_spec_new, hegel_zoned_spec_set_ambiguous, hegel_zoned_spec_set_civil_bounds,
    hegel_zoned_spec_set_imaginary, hegel_zoned_spec_set_instant_bounds,
    hegel_zoned_spec_set_precision,
};
use std::collections::BTreeSet;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr;

type Call<'a> = (&'a str, Box<dyn Fn() -> hegel_c::hegel_result_t + 'a>);

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> hegel_datetime_t {
    hegel_datetime_t {
        date: hegel_date_t { year, month, day },
        time: hegel_time_t {
            hour,
            minute,
            second,
            nanosecond: 0,
        },
    }
}

fn at(second: i64) -> hegel_timestamp_t {
    hegel_timestamp_t {
        second,
        nanosecond: 0,
    }
}

fn empty_result() -> hegel_generate_string_result_t {
    hegel_generate_string_result_t {
        data: ptr::null_mut(),
        len: 0,
    }
}

fn zoned() -> hegel_zoned_datetime_t {
    hegel_zoned_datetime_t {
        civil: civil(0, 0, 0, 0, 0, 0),
        offset_seconds: 0,
        offset_known: false,
        fold: false,
        kind: HEGEL_TIMEZONE_KIND_UNIQUE,
        offset_before: 0,
        offset_after: 0,
        timestamp: at(0),
    }
}

unsafe fn take_string(
    ctx: *mut HegelContext,
    result: &mut hegel_generate_string_result_t,
) -> String {
    assert!(!result.data.is_null());
    let s = String::from_utf8(
        unsafe { std::slice::from_raw_parts(result.data.cast::<u8>(), result.len) }.to_vec(),
    )
    .unwrap();
    unsafe { ok(hegel_generate_string_result_free(ctx, result)) };
    assert!(result.data.is_null());
    s
}

unsafe fn tz_spec(ctx: *mut HegelContext) -> *mut HegelTimezoneSpec {
    let mut spec: *mut HegelTimezoneSpec = ptr::null_mut();
    unsafe { ok(hegel_timezone_spec_new(ctx, &mut spec)) };
    assert!(!spec.is_null());
    spec
}

unsafe fn zoned_spec(ctx: *mut HegelContext) -> *mut HegelZonedSpec {
    let mut spec: *mut HegelZonedSpec = ptr::null_mut();
    unsafe { ok(hegel_zoned_spec_new(ctx, &mut spec)) };
    assert!(!spec.is_null());
    spec
}

unsafe fn set_names(
    ctx: *mut HegelContext,
    spec: *mut HegelTimezoneSpec,
    names: &[&str],
) -> hegel_c::hegel_result_t {
    let owned: Vec<CString> = names.iter().map(|n| CString::new(*n).unwrap()).collect();
    let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
    unsafe { hegel_timezone_spec_set_names(ctx, spec, ptrs.as_ptr(), ptrs.len()) }
}

unsafe fn complete_valid(ctx: *mut HegelContext, tc: *mut HegelTestCase) {
    unsafe {
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_VALID as u32,
            ptr::null(),
        ));
        ok(hegel_c::hegel_test_case_free(ctx, tc));
    }
}

unsafe fn drain(ctx: *mut HegelContext, run: *mut HegelRun) {
    loop {
        let tc = unsafe { next_case(ctx, run) };
        if tc.is_null() {
            break;
        }
        unsafe { complete_valid(ctx, tc) };
    }
}

unsafe fn run_of(ctx: *mut HegelContext, cases: u64) -> *mut HegelRun {
    let s = unsafe { make_settings_no_db(ctx) };
    unsafe { ok(hegel_settings_set_test_cases(ctx, s, cases)) };
    let run = unsafe { start(ctx, s) };
    unsafe { ok(hegel_c::hegel_settings_free(ctx, s)) };
    run
}

#[test]
fn timezone_spec_setters_validate_their_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_timezone_spec_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_timezone_spec_new"));
        ok(hegel_timezone_spec_free(ctx, ptr::null_mut()));

        let null_handle: [Call<'_>; 5] = [
            (
                "hegel_timezone_spec_set_links",
                Box::new(|| hegel_timezone_spec_set_links(ctx, ptr::null_mut(), true)),
            ),
            (
                "hegel_timezone_spec_set_legacy",
                Box::new(|| hegel_timezone_spec_set_legacy(ctx, ptr::null_mut(), true)),
            ),
            (
                "hegel_timezone_spec_set_unknown",
                Box::new(|| hegel_timezone_spec_set_unknown(ctx, ptr::null_mut(), true)),
            ),
            (
                "hegel_timezone_spec_set_opaque",
                Box::new(|| hegel_timezone_spec_set_opaque(ctx, ptr::null_mut(), true)),
            ),
            (
                "hegel_timezone_spec_set_names",
                Box::new(|| set_names(ctx, ptr::null_mut(), &["UTC"])),
            ),
        ];
        for (name, call) in &null_handle {
            assert_eq!(call(), HEGEL_E_INVALID_HANDLE, "{name}");
            assert!(last_error(ctx).contains(name), "{}", last_error(ctx));
        }

        let spec = tz_spec(ctx);
        assert_eq!(
            hegel_timezone_spec_set_names(ctx, spec, ptr::null(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("names pointer is null"));
        let null_name: [*const c_char; 1] = [ptr::null()];
        assert_eq!(
            hegel_timezone_spec_set_names(ctx, spec, null_name.as_ptr(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("names[0] is null"));
        let bad = [0xffu8, 0];
        let bad_name: [*const c_char; 1] = [bad.as_ptr().cast()];
        assert_eq!(
            hegel_timezone_spec_set_names(ctx, spec, bad_name.as_ptr(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("not valid UTF-8"));
        assert_eq!(set_names(ctx, spec, &[]), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("at least one allowed name"));
        assert_eq!(
            set_names(ctx, spec, &["Mars/Olympus_Mons"]),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown time zone \"Mars/Olympus_Mons\""));
        ok(hegel_timezone_spec_set_opaque(ctx, spec, true));
        ok(set_names(ctx, spec, &["Mars/Olympus_Mons", "UTC"]));
        assert_eq!(
            hegel_timezone_spec_set_opaque(ctx, spec, false),
            HEGEL_E_INVALID_ARG
        );
        ok(set_names(ctx, spec, &["Etc/Unknown"]));
        ok(hegel_timezone_spec_set_links(ctx, spec, false));
        ok(hegel_timezone_spec_set_legacy(ctx, spec, true));
        ok(hegel_timezone_spec_set_unknown(ctx, spec, true));
        assert_eq!(set_names(ctx, spec, &[]), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("at least one allowed name"));
        ok(hegel_timezone_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn generate_timezone_draws_allowed_names_and_validates_handles() {
    let ctx = hegel_context_new();
    unsafe {
        let spec = tz_spec(ctx);
        let mut result = empty_result();
        assert_eq!(
            hegel_generate_timezone(ctx, ptr::null_mut(), spec, &mut result),
            HEGEL_E_INVALID_HANDLE
        );
        let run = run_of(ctx, 30);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_timezone(ctx, tc, ptr::null(), &mut result),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("spec handle is null"));
        assert_eq!(
            hegel_generate_timezone(ctx, tc, spec, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("out parameter is null"));
        ok(set_names(
            ctx,
            spec,
            &["UTC", "Europe/London", "Asia/Kolkata"],
        ));
        let mut seen = BTreeSet::new();
        let mut tc = tc;
        while !tc.is_null() {
            for _ in 0..10 {
                ok(hegel_generate_timezone(ctx, tc, spec, &mut result));
                seen.insert(take_string(ctx, &mut result));
            }
            complete_valid(ctx, tc);
            tc = next_case(ctx, run);
        }
        assert_eq!(
            seen,
            ["UTC", "Europe/London", "Asia/Kolkata"]
                .into_iter()
                .map(String::from)
                .collect()
        );
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_timezone_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn zoned_spec_setters_validate_their_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_zoned_spec_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_zoned_spec_new"));
        ok(hegel_zoned_spec_free(ctx, ptr::null_mut()));
        let whole = (civil(2000, 1, 1, 0, 0, 0), civil(2001, 1, 1, 0, 0, 0));
        let null_handle: [Call<'_>; 5] = [
            (
                "hegel_zoned_spec_set_civil_bounds",
                Box::new(|| {
                    hegel_zoned_spec_set_civil_bounds(ctx, ptr::null_mut(), whole.0, whole.1)
                }),
            ),
            (
                "hegel_zoned_spec_set_instant_bounds",
                Box::new(|| {
                    hegel_zoned_spec_set_instant_bounds(ctx, ptr::null_mut(), at(0), at(1))
                }),
            ),
            (
                "hegel_zoned_spec_set_precision",
                Box::new(|| hegel_zoned_spec_set_precision(ctx, ptr::null_mut(), 0)),
            ),
            (
                "hegel_zoned_spec_set_imaginary",
                Box::new(|| hegel_zoned_spec_set_imaginary(ctx, ptr::null_mut(), 0)),
            ),
            (
                "hegel_zoned_spec_set_ambiguous",
                Box::new(|| hegel_zoned_spec_set_ambiguous(ctx, ptr::null_mut(), 0)),
            ),
        ];
        for (name, call) in &null_handle {
            assert_eq!(call(), HEGEL_E_INVALID_HANDLE, "{name}");
            assert!(last_error(ctx).contains(name), "{}", last_error(ctx));
        }

        let spec = zoned_spec(ctx);
        assert_eq!(
            hegel_zoned_spec_set_civil_bounds(ctx, spec, whole.1, whole.0),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("min_value <= max_value"));
        assert_eq!(
            hegel_zoned_spec_set_civil_bounds(ctx, spec, civil(2000, 2, 30, 0, 0, 0), whole.1),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_zoned_spec_set_instant_bounds(ctx, spec, at(1), at(0)),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_zoned_spec_set_instant_bounds(ctx, spec, at(i64::MIN), at(0)),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_zoned_spec_set_precision(ctx, spec, 6),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown precision 6"));
        assert_eq!(
            hegel_zoned_spec_set_imaginary(ctx, spec, 2),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown policy 2"));
        assert_eq!(
            hegel_zoned_spec_set_ambiguous(ctx, spec, 3),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown policy 3"));
        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            spec,
            at(1),
            at(59),
        ));
        assert_eq!(
            hegel_zoned_spec_set_precision(
                ctx,
                spec,
                hegel_precision_t::HEGEL_PRECISION_MINUTES as u32
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("no value at precision"));
        ok(hegel_zoned_spec_set_civil_bounds(
            ctx, spec, whole.0, whole.1,
        ));
        for precision in [
            hegel_precision_t::HEGEL_PRECISION_NANOSECONDS,
            hegel_precision_t::HEGEL_PRECISION_MICROSECONDS,
            hegel_precision_t::HEGEL_PRECISION_MILLISECONDS,
            hegel_precision_t::HEGEL_PRECISION_SECONDS,
            hegel_precision_t::HEGEL_PRECISION_MINUTES,
            hegel_precision_t::HEGEL_PRECISION_HOURS,
        ] {
            ok(hegel_zoned_spec_set_precision(ctx, spec, precision as u32));
        }
        for imaginary in [
            hegel_imaginary_t::HEGEL_IMAGINARY_ALLOW,
            hegel_imaginary_t::HEGEL_IMAGINARY_AVOID,
        ] {
            ok(hegel_zoned_spec_set_imaginary(ctx, spec, imaginary as u32));
        }
        for ambiguous in [
            hegel_ambiguous_t::HEGEL_AMBIGUOUS_BOTH,
            hegel_ambiguous_t::HEGEL_AMBIGUOUS_EARLIER,
            hegel_ambiguous_t::HEGEL_AMBIGUOUS_LATER,
        ] {
            ok(hegel_zoned_spec_set_ambiguous(ctx, spec, ambiguous as u32));
        }
        ok(hegel_zoned_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn generate_zoned_datetime_draws_consistent_values_and_validates_handles() {
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        ok(set_names(ctx, tz, &["America/New_York", "UTC"]));
        let spec = zoned_spec(ctx);
        ok(hegel_zoned_spec_set_civil_bounds(
            ctx,
            spec,
            civil(2000, 1, 1, 0, 0, 0),
            civil(2030, 12, 31, 23, 59, 59),
        ));
        ok(hegel_zoned_spec_set_precision(
            ctx,
            spec,
            hegel_precision_t::HEGEL_PRECISION_SECONDS as u32,
        ));
        let mut value = zoned();
        let mut zone = empty_result();
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, ptr::null_mut(), tz, spec, &mut value, &mut zone),
            HEGEL_E_INVALID_HANDLE
        );
        let run = run_of(ctx, 40);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, ptr::null(), spec, &mut value, &mut zone),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("time zone spec handle is null"));
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, ptr::null(), &mut value, &mut zone),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("zoned spec handle is null"));
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, spec, ptr::null_mut(), &mut zone),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("out parameter is null"));
        let mut zones = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        let mut tc = tc;
        while !tc.is_null() {
            for _ in 0..10 {
                ok(hegel_generate_zoned_datetime(
                    ctx, tc, tz, spec, &mut value, &mut zone,
                ));
                {
                    let name = take_string(ctx, &mut zone);
                    assert!((2000..=2030).contains(&value.civil.date.year));
                    assert_eq!(value.civil.time.nanosecond, 0);
                    let expected_offset = if name == "UTC" {
                        0
                    } else if value.kind == HEGEL_TIMEZONE_KIND_GAP {
                        value.offset_after
                    } else {
                        value.offset_seconds
                    };
                    assert!(value.offset_known);
                    assert_eq!(value.offset_seconds, expected_offset);
                    if value.kind == HEGEL_TIMEZONE_KIND_UNIQUE {
                        assert_eq!(value.offset_before, value.offset_seconds);
                        assert_eq!(value.offset_after, value.offset_seconds);
                        assert!(!value.fold);
                    } else {
                        assert_ne!(value.offset_before, value.offset_after);
                    }
                    zones.insert(name);
                    kinds.insert(value.kind as u32);
                }
            }
            complete_valid(ctx, tc);
            tc = next_case(ctx, run);
        }
        assert_eq!(zones.len(), 2);
        assert!(kinds.contains(&(HEGEL_TIMEZONE_KIND_UNIQUE as u32)));
        ok(hegel_c::hegel_run_free(ctx, run));

        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            spec,
            at(1_710_054_000 - 60),
            at(1_710_054_000 + 60),
        ));
        let run = run_of(ctx, 5);
        let tc = next_case(ctx, run);
        ok(hegel_generate_zoned_datetime(
            ctx, tc, tz, spec, &mut value, &mut zone,
        ));
        take_string(ctx, &mut zone);
        assert!((1_710_054_000 - 60..=1_710_054_000 + 60).contains(&value.timestamp.second));
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));

        ok(set_names(ctx, tz, &["America/New_York"]));
        for (min, max, kind) in [
            (
                civil(2024, 3, 10, 2, 0, 0),
                civil(2024, 3, 10, 2, 59, 59),
                HEGEL_TIMEZONE_KIND_GAP,
            ),
            (
                civil(2024, 11, 3, 1, 0, 0),
                civil(2024, 11, 3, 1, 59, 59),
                HEGEL_TIMEZONE_KIND_FOLD,
            ),
        ] {
            ok(hegel_zoned_spec_set_civil_bounds(ctx, spec, min, max));
            let run = run_of(ctx, 5);
            let tc = next_case(ctx, run);
            ok(hegel_generate_zoned_datetime(
                ctx, tc, tz, spec, &mut value, &mut zone,
            ));
            assert_eq!(take_string(ctx, &mut zone), "America/New_York");
            assert_eq!(value.kind, kind);
            assert_ne!(value.offset_before, value.offset_after);
            complete_valid(ctx, tc);
            drain(ctx, run);
            ok(hegel_c::hegel_run_free(ctx, run));
        }
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn draws_after_overrun_report_stop_test() {
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        let spec = zoned_spec(ctx);
        let run = run_of(ctx, 1);
        let tc = next_case(ctx, run);
        let mut filler = false;
        loop {
            match hegel_c::hegel_generate_boolean(ctx, tc, 0.5, false, false, &mut filler) {
                HEGEL_OK => {}
                HEGEL_E_STOP_TEST => break,
                other => panic!("unexpected rc: {other:?}"),
            }
        }
        let mut result = empty_result();
        assert_eq!(
            hegel_generate_timezone(ctx, tc, tz, &mut result),
            HEGEL_E_STOP_TEST
        );
        let mut value = zoned();
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, &mut result),
            HEGEL_E_STOP_TEST
        );
        assert!(result.data.is_null());
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}
