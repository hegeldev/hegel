//! In-process exercise of the time zone and zoned datetime draws over the C
//! ABI: spec construction and argument checking, and the values a frontend
//! reads back. The draws themselves are checked against jiff in the
//! embedded `timezone_tests` and `zoned_tests`.

mod common;

use common::{
    check_zoned_value, civil_second, last_error, make_settings_no_db, next_case, ok, start,
    zoned_text,
};
use hegel_c::hegel_result_t::*;
use hegel_c::hegel_timezone_kind_t::*;
use hegel_c::{
    HegelContext, HegelRun, HegelTestCase, HegelTimezoneSpec, HegelZonedSpec, hegel_ambiguous_t,
    hegel_context_free, hegel_context_new, hegel_date_t, hegel_datetime_t,
    hegel_format_zoned_datetime, hegel_generate_timezone, hegel_generate_zoned_datetime,
    hegel_generate_zoned_datetime_for_zone, hegel_imaginary_t, hegel_mark_complete,
    hegel_owned_string_free, hegel_owned_string_t, hegel_precision_t,
    hegel_settings_set_test_cases, hegel_status_t, hegel_string_t, hegel_time_t, hegel_timestamp_t,
    hegel_timezone_spec_free, hegel_timezone_spec_new, hegel_timezone_spec_set_legacy,
    hegel_timezone_spec_set_links, hegel_timezone_spec_set_names, hegel_timezone_spec_set_opaque,
    hegel_timezone_spec_set_unknown, hegel_timezone_spec_validate, hegel_zone_source_bundled,
    hegel_zone_source_free, hegel_zoned_datetime_t, hegel_zoned_spec_free, hegel_zoned_spec_new,
    hegel_zoned_spec_set_ambiguous, hegel_zoned_spec_set_civil_bounds,
    hegel_zoned_spec_set_imaginary, hegel_zoned_spec_set_instant_bounds,
    hegel_zoned_spec_set_precision, hegel_zoned_spec_validate,
};
use std::collections::BTreeSet;
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

fn empty_result() -> hegel_owned_string_t {
    hegel_owned_string_t {
        data: ptr::null(),
        len: 0,
    }
}

fn s(text: &str) -> hegel_string_t {
    hegel_string_t {
        data: text.as_ptr().cast(),
        len: text.len(),
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

unsafe fn take_string(ctx: *mut HegelContext, result: &mut hegel_owned_string_t) -> String {
    assert!(!result.data.is_null());
    let s = String::from_utf8(
        unsafe { std::slice::from_raw_parts(result.data.cast::<u8>(), result.len) }.to_vec(),
    )
    .unwrap();
    unsafe { ok(hegel_owned_string_free(ctx, result)) };
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
    let views: Vec<hegel_string_t> = names.iter().map(|n| s(n)).collect();
    unsafe { hegel_timezone_spec_set_names(ctx, spec, views.as_ptr(), views.len()) }
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
        let null_name = [hegel_string_t {
            data: ptr::null(),
            len: 1,
        }];
        assert_eq!(
            hegel_timezone_spec_set_names(ctx, spec, null_name.as_ptr(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("names[0] is null"));
        let bad = [0xffu8];
        let bad_name = [hegel_string_t {
            data: bad.as_ptr().cast(),
            len: 1,
        }];
        assert_eq!(
            hegel_timezone_spec_set_names(ctx, spec, bad_name.as_ptr(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("not valid UTF-8"));
        assert_eq!(set_names(ctx, spec, &[]), HEGEL_E_INVALID_ARG);
        assert_eq!(
            last_error(ctx),
            "hegel_timezone_spec_set_names: a time zone draw needs at least one allowed name"
        );
        assert_eq!(
            hegel_timezone_spec_validate(ctx, ptr::null()),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("hegel_timezone_spec_validate: spec pointer is null"));
        ok(hegel_timezone_spec_validate(ctx, spec));
        ok(set_names(ctx, spec, &["Mars/Olympus_Mons"]));
        assert_eq!(hegel_timezone_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        assert!(
            last_error(ctx).starts_with(
                "hegel_timezone_spec_validate: unknown time zone \"Mars/Olympus_Mons\""
            ),
            "{}",
            last_error(ctx)
        );
        ok(hegel_timezone_spec_set_opaque(ctx, spec, true));
        ok(hegel_timezone_spec_validate(ctx, spec));
        ok(set_names(ctx, spec, &["Mars/Olympus_Mons", "UTC"]));
        ok(hegel_timezone_spec_set_opaque(ctx, spec, false));
        assert_eq!(hegel_timezone_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        ok(set_names(ctx, spec, &["Etc/Unknown"]));
        ok(hegel_timezone_spec_validate(ctx, spec));
        ok(hegel_timezone_spec_set_links(ctx, spec, false));
        ok(hegel_timezone_spec_set_legacy(ctx, spec, true));
        ok(hegel_timezone_spec_set_unknown(ctx, spec, true));
        assert_eq!(set_names(ctx, spec, &[]), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("at least one allowed name"));
        ok(hegel_timezone_spec_validate(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

unsafe fn names_drawn(ctx: *mut HegelContext, spec: *const HegelTimezoneSpec) -> BTreeSet<String> {
    let run = unsafe { run_of(ctx, 30) };
    let mut seen = BTreeSet::new();
    let mut result = empty_result();
    let mut tc = unsafe { next_case(ctx, run) };
    while !tc.is_null() {
        for _ in 0..10 {
            unsafe {
                ok(hegel_generate_timezone(ctx, tc, spec, &mut result));
                seen.insert(take_string(ctx, &mut result));
            }
        }
        unsafe { complete_valid(ctx, tc) };
        tc = unsafe { next_case(ctx, run) };
    }
    unsafe { ok(hegel_c::hegel_run_free(ctx, run)) };
    seen
}

fn set_of(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[test]
fn a_rejected_timezone_spec_change_leaves_the_spec_usable() {
    let ctx = hegel_context_new();
    unsafe {
        let spec = tz_spec(ctx);
        ok(set_names(ctx, spec, &["UTC", "Europe/London"]));
        assert_eq!(set_names(ctx, spec, &[]), HEGEL_E_INVALID_ARG);
        ok(hegel_timezone_spec_set_links(ctx, spec, false));
        assert_eq!(last_error(ctx), "");
        assert_eq!(names_drawn(ctx, spec), set_of(&["UTC", "Europe/London"]));
        ok(hegel_timezone_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn a_timezone_spec_that_fails_validation_fails_its_draws_until_it_is_fixed() {
    let ctx = hegel_context_new();
    unsafe {
        let spec = tz_spec(ctx);
        ok(set_names(ctx, spec, &["Mars/Olympus_Mons", "UTC"]));
        let run = run_of(ctx, 1);
        let tc = next_case(ctx, run);
        let mut result = empty_result();
        for _ in 0..2 {
            assert_eq!(
                hegel_generate_timezone(ctx, tc, spec, &mut result),
                HEGEL_E_INVALID_ARG
            );
            assert!(
                last_error(ctx).starts_with(
                    "hegel_generate_timezone: unknown time zone \"Mars/Olympus_Mons\""
                ),
                "{}",
                last_error(ctx)
            );
            assert!(result.data.is_null());
        }
        let zspec = zoned_spec(ctx);
        let mut value = zoned();
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, spec, zspec, &mut value, &mut result),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx).starts_with(
                "hegel_generate_zoned_datetime: unknown time zone \"Mars/Olympus_Mons\""
            ),
            "{}",
            last_error(ctx)
        );
        ok(hegel_zoned_spec_free(ctx, zspec));
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_timezone_spec_set_opaque(ctx, spec, true));
        assert_eq!(
            names_drawn(ctx, spec),
            set_of(&["Mars/Olympus_Mons", "UTC"])
        );
        ok(hegel_timezone_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn timezone_spec_setters_can_be_called_in_any_order() {
    let ctx = hegel_context_new();
    unsafe {
        let names_first = tz_spec(ctx);
        ok(set_names(
            ctx,
            names_first,
            &["Mars/Olympus_Mons", "Etc/Unknown", "Asia/Calcutta"],
        ));
        ok(hegel_timezone_spec_set_opaque(ctx, names_first, true));
        ok(hegel_timezone_spec_set_unknown(ctx, names_first, true));
        ok(hegel_timezone_spec_set_links(ctx, names_first, false));
        ok(hegel_timezone_spec_set_legacy(ctx, names_first, false));
        let names_last = tz_spec(ctx);
        ok(hegel_timezone_spec_set_legacy(ctx, names_last, false));
        ok(hegel_timezone_spec_set_links(ctx, names_last, false));
        ok(hegel_timezone_spec_set_unknown(ctx, names_last, true));
        ok(hegel_timezone_spec_set_opaque(ctx, names_last, true));
        ok(set_names(
            ctx,
            names_last,
            &["Mars/Olympus_Mons", "Etc/Unknown", "Asia/Calcutta"],
        ));
        let expected = set_of(&["Mars/Olympus_Mons", "Etc/Unknown", "Asia/Calcutta"]);
        for spec in [names_first, names_last] {
            ok(hegel_timezone_spec_validate(ctx, spec));
            assert_eq!(names_drawn(ctx, spec), expected);
            ok(hegel_timezone_spec_free(ctx, spec));
        }
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
        assert_eq!(
            hegel_zoned_spec_validate(ctx, ptr::null()),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("hegel_zoned_spec_validate: spec pointer is null"));
        ok(hegel_zoned_spec_validate(ctx, spec));
        ok(hegel_zoned_spec_set_precision(
            ctx,
            spec,
            hegel_precision_t::HEGEL_PRECISION_MINUTES as u32,
        ));
        ok(hegel_zoned_spec_set_civil_bounds(
            ctx,
            spec,
            civil(2000, 1, 1, 0, 0, 1),
            civil(2000, 1, 1, 0, 0, 59),
        ));
        assert_eq!(hegel_zoned_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        assert!(
            last_error(ctx).starts_with(
                "hegel_zoned_spec_validate: a zoned datetime draw has no value at precision \
                 minutes in [2000-01-01T00:00:01, 2000-01-01T00:00:59]"
            ),
            "{}",
            last_error(ctx)
        );
        ok(hegel_zoned_spec_set_civil_bounds(
            ctx, spec, whole.0, whole.1,
        ));
        ok(hegel_zoned_spec_validate(ctx, spec));
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
                    check_zoned_value(ctx, ptr::null(), &name, &value);
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

        let instant = zoned_spec(ctx);
        ok(hegel_zoned_spec_set_precision(
            ctx,
            instant,
            hegel_precision_t::HEGEL_PRECISION_SECONDS as u32,
        ));
        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            instant,
            at(1_710_054_000 - 60),
            at(1_710_054_000 + 60),
        ));
        ok(hegel_zoned_spec_validate(ctx, instant));
        let run = run_of(ctx, 5);
        let tc = next_case(ctx, run);
        ok(hegel_generate_zoned_datetime(
            ctx, tc, tz, instant, &mut value, &mut zone,
        ));
        take_string(ctx, &mut zone);
        assert!((1_710_054_000 - 60..=1_710_054_000 + 60).contains(&value.timestamp.second));
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));

        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            instant,
            at(1),
            at(59),
        ));
        ok(hegel_zoned_spec_set_precision(
            ctx,
            instant,
            hegel_precision_t::HEGEL_PRECISION_MINUTES as u32,
        ));
        ok(hegel_zoned_spec_validate(ctx, instant));
        let run = run_of(ctx, 5);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, instant, &mut value, &mut zone),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx)
                .starts_with("hegel_generate_zoned_datetime: a zoned datetime draw has no value"),
            "{}",
            last_error(ctx)
        );
        assert!(last_error(ctx).ends_with("in any allowed zone"));
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, instant));

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
            check_zoned_value(ctx, ptr::null(), "America/New_York", &value);
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

unsafe fn complete_invalid(ctx: *mut HegelContext, tc: *mut HegelTestCase) {
    unsafe {
        ok(hegel_mark_complete(
            ctx,
            tc,
            hegel_status_t::HEGEL_STATUS_INVALID as u32,
            ptr::null(),
        ));
        ok(hegel_c::hegel_test_case_free(ctx, tc));
    }
}

unsafe fn seconds_spec(
    ctx: *mut HegelContext,
    min: hegel_datetime_t,
    max: hegel_datetime_t,
) -> *mut HegelZonedSpec {
    let spec = unsafe { zoned_spec(ctx) };
    unsafe {
        ok(hegel_zoned_spec_set_precision(
            ctx,
            spec,
            hegel_precision_t::HEGEL_PRECISION_SECONDS as u32,
        ));
        ok(hegel_zoned_spec_set_civil_bounds(ctx, spec, min, max));
    }
    spec
}

#[test]
fn zoned_draws_agree_with_resolving_their_wall_time() {
    let ctx = hegel_context_new();
    let names = [
        "America/New_York",
        "Europe/London",
        "Australia/Lord_Howe",
        "Asia/Kolkata",
        "Pacific/Apia",
        "UTC",
    ];
    unsafe {
        let tz = tz_spec(ctx);
        ok(set_names(ctx, tz, &names));
        let spec = seconds_spec(
            ctx,
            civil(1990, 1, 1, 0, 0, 0),
            civil(2030, 12, 31, 23, 59, 59),
        );
        ok(hegel_zoned_spec_validate(ctx, spec));
        let mut value = zoned();
        let mut zone = empty_result();
        let mut seen = BTreeSet::new();
        let run = run_of(ctx, 100);
        let mut tc = next_case(ctx, run);
        while !tc.is_null() {
            for _ in 0..10 {
                ok(hegel_generate_zoned_datetime(
                    ctx, tc, tz, spec, &mut value, &mut zone,
                ));
                let name = take_string(ctx, &mut zone);
                check_zoned_value(ctx, ptr::null(), &name, &value);
                if value.kind == HEGEL_TIMEZONE_KIND_UNIQUE {
                    let text = zoned_text(ctx, &value, &name);
                    let theirs: jiff::Zoned = text.parse().unwrap();
                    assert_eq!(
                        theirs.timestamp().as_second(),
                        value.timestamp.second,
                        "{text}"
                    );
                    assert_eq!(theirs.to_string(), text);
                }
                seen.insert(name);
            }
            for name in names {
                ok(hegel_generate_zoned_datetime_for_zone(
                    ctx,
                    tc,
                    ptr::null(),
                    s(name),
                    spec,
                    &mut value,
                ));
                check_zoned_value(ctx, ptr::null(), name, &value);
                assert!((1990..=2030).contains(&value.civil.date.year));
            }
            complete_valid(ctx, tc);
            tc = next_case(ctx, run);
        }
        assert_eq!(seen, set_of(&names));
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn unknown_and_opaque_zones_are_drawn_with_an_unknown_offset() {
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        ok(set_names(ctx, tz, &["Mars/Base", "Etc/Unknown"]));
        ok(hegel_timezone_spec_set_opaque(ctx, tz, true));
        ok(hegel_timezone_spec_validate(ctx, tz));
        let spec = seconds_spec(
            ctx,
            civil(1990, 1, 1, 0, 0, 0),
            civil(2030, 12, 31, 23, 59, 59),
        );
        let mut value = zoned();
        let mut zone = empty_result();
        let mut seen = BTreeSet::new();
        let run = run_of(ctx, 40);
        let mut tc = next_case(ctx, run);
        while !tc.is_null() {
            for _ in 0..5 {
                ok(hegel_generate_zoned_datetime(
                    ctx, tc, tz, spec, &mut value, &mut zone,
                ));
                let name = take_string(ctx, &mut zone);
                assert!(!value.offset_known);
                assert_eq!(value.offset_seconds, 0);
                assert_eq!((value.offset_before, value.offset_after), (0, 0));
                assert_eq!(value.kind, HEGEL_TIMEZONE_KIND_UNIQUE);
                assert!(!value.fold);
                assert_eq!(value.timestamp.second, civil_second(&value.civil));
                let text = zoned_text(ctx, &value, &name);
                if name == "Etc/Unknown" {
                    check_zoned_value(ctx, ptr::null(), &name, &value);
                    assert!(text.ends_with("Z[Etc/Unknown]"), "{text}");
                } else {
                    assert!(text.ends_with("-00:00[Mars/Base]"), "{text}");
                }
                seen.insert(name);
            }
            ok(hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                s("Etc/Unknown"),
                spec,
                &mut value,
            ));
            assert!(!value.offset_known);
            check_zoned_value(ctx, ptr::null(), "Etc/Unknown", &value);
            assert_eq!(
                hegel_generate_zoned_datetime_for_zone(
                    ctx,
                    tc,
                    ptr::null(),
                    s("Mars/Base"),
                    spec,
                    &mut value,
                ),
                HEGEL_E_INVALID_ARG
            );
            assert_eq!(
                last_error(ctx),
                "hegel_generate_zoned_datetime_for_zone: unknown time zone \"Mars/Base\"; names \
                 are matched exactly as the zone source spells them (the bundled database as IANA \
                 does), plus Etc/Unknown"
            );
            complete_valid(ctx, tc);
            tc = next_case(ctx, run);
        }
        assert_eq!(seen, set_of(&["Mars/Base", "Etc/Unknown"]));
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn a_drawn_zone_without_a_value_rejects_the_test_case() {
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        ok(set_names(ctx, tz, &["America/New_York", "UTC"]));
        let spec = seconds_spec(
            ctx,
            civil(2024, 3, 10, 2, 0, 0),
            civil(2024, 3, 10, 2, 59, 59),
        );
        ok(hegel_zoned_spec_set_imaginary(
            ctx,
            spec,
            hegel_imaginary_t::HEGEL_IMAGINARY_AVOID as u32,
        ));
        ok(hegel_zoned_spec_validate(ctx, spec));
        let mut value = zoned();
        let mut zone = empty_result();
        let (mut drawn, mut rejected) = (0, 0);
        let run = run_of(ctx, 100);
        let mut tc = next_case(ctx, run);
        while !tc.is_null() {
            match hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, &mut zone) {
                HEGEL_OK => {
                    assert_eq!(take_string(ctx, &mut zone), "UTC");
                    check_zoned_value(ctx, ptr::null(), "UTC", &value);
                    drawn += 1;
                    complete_valid(ctx, tc);
                }
                HEGEL_E_ASSUME => {
                    assert!(zone.data.is_null());
                    rejected += 1;
                    complete_invalid(ctx, tc);
                }
                other => panic!("{other:?}: {}", last_error(ctx)),
            }
            tc = next_case(ctx, run);
        }
        assert!(drawn > 0);
        assert!(rejected > 0);
        ok(hegel_c::hegel_run_free(ctx, run));

        let run = run_of(ctx, 5);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                s("America/New_York"),
                spec,
                &mut value,
            ),
            HEGEL_E_ASSUME
        );
        complete_invalid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));

        ok(set_names(ctx, tz, &["America/New_York"]));
        let run = run_of(ctx, 5);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, &mut zone),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx).ends_with("in any allowed zone"),
            "{}",
            last_error(ctx)
        );
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn instant_bounds_without_a_whole_unit_fail_validation_and_every_draw() {
    let nanos = |nanosecond| hegel_timestamp_t {
        second: 0,
        nanosecond,
    };
    let message = "a zoned datetime draw has no value at precision microseconds in \
                   [1970-01-01T00:00:00.000000001Z, 1970-01-01T00:00:00.000000999Z]";
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        ok(set_names(ctx, tz, &["UTC"]));
        let spec = zoned_spec(ctx);
        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            spec,
            nanos(1),
            nanos(999),
        ));
        ok(hegel_zoned_spec_validate(ctx, spec));
        ok(hegel_zoned_spec_set_precision(
            ctx,
            spec,
            hegel_precision_t::HEGEL_PRECISION_MICROSECONDS as u32,
        ));
        assert_eq!(hegel_zoned_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        assert_eq!(
            last_error(ctx),
            format!("hegel_zoned_spec_validate: {message}")
        );
        let mut value = zoned();
        let mut zone = empty_result();
        let run = run_of(ctx, 1);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, &mut zone),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            format!("hegel_generate_zoned_datetime: {message}")
        );
        assert!(zone.data.is_null());
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                s("Europe/London"),
                spec,
                &mut value,
            ),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            format!("hegel_generate_zoned_datetime_for_zone: {message}")
        );

        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            spec,
            nanos(1),
            nanos(1000),
        ));
        ok(hegel_zoned_spec_validate(ctx, spec));
        ok(hegel_generate_zoned_datetime(
            ctx, tc, tz, spec, &mut value, &mut zone,
        ));
        assert_eq!(take_string(ctx, &mut zone), "UTC");
        assert_eq!(
            (value.timestamp.second, value.timestamp.nanosecond),
            (0, 1000)
        );
        ok(hegel_generate_zoned_datetime_for_zone(
            ctx,
            tc,
            ptr::null(),
            s("Europe/London"),
            spec,
            &mut value,
        ));
        assert_eq!(
            (value.timestamp.second, value.timestamp.nanosecond),
            (0, 1000)
        );
        assert_eq!(value.offset_seconds, 3600);

        ok(hegel_zoned_spec_set_instant_bounds(
            ctx,
            spec,
            nanos(1),
            nanos(999),
        ));
        for precision in [
            hegel_precision_t::HEGEL_PRECISION_MINUTES,
            hegel_precision_t::HEGEL_PRECISION_HOURS,
        ] {
            ok(hegel_zoned_spec_set_precision(ctx, spec, precision as u32));
            ok(hegel_zoned_spec_validate(ctx, spec));
        }
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                s("Europe/London"),
                spec,
                &mut value,
            ),
            HEGEL_E_ASSUME
        );
        complete_invalid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn a_spec_bounded_on_both_the_wall_clock_and_the_instant_fails_validation() {
    let ctx = hegel_context_new();
    unsafe {
        let tz = tz_spec(ctx);
        let (min, max) = (civil(2000, 1, 1, 0, 0, 0), civil(2001, 1, 1, 0, 0, 0));
        for civil_first in [true, false] {
            let spec = zoned_spec(ctx);
            if civil_first {
                ok(hegel_zoned_spec_set_civil_bounds(ctx, spec, min, max));
                ok(hegel_zoned_spec_set_instant_bounds(
                    ctx,
                    spec,
                    at(0),
                    at(60),
                ));
            } else {
                ok(hegel_zoned_spec_set_instant_bounds(
                    ctx,
                    spec,
                    at(0),
                    at(60),
                ));
                ok(hegel_zoned_spec_set_civil_bounds(ctx, spec, min, max));
            }
            assert_eq!(hegel_zoned_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
            assert_eq!(
                last_error(ctx),
                "hegel_zoned_spec_validate: a zoned datetime draw is bounded on the wall clock \
                 or on the instant, not both: civil bounds and instant bounds were both set"
            );
            let run = run_of(ctx, 1);
            let tc = next_case(ctx, run);
            let mut value = zoned();
            let mut zone = empty_result();
            assert_eq!(
                hegel_generate_zoned_datetime(ctx, tc, tz, spec, &mut value, &mut zone),
                HEGEL_E_INVALID_ARG
            );
            assert!(
                last_error(ctx).starts_with(
                    "hegel_generate_zoned_datetime: a zoned datetime draw is bounded on the wall"
                ),
                "{}",
                last_error(ctx)
            );
            assert_eq!(
                hegel_generate_zoned_datetime_for_zone(
                    ctx,
                    tc,
                    ptr::null(),
                    s("UTC"),
                    spec,
                    &mut value
                ),
                HEGEL_E_INVALID_ARG
            );
            assert!(
                last_error(ctx).starts_with(
                    "hegel_generate_zoned_datetime_for_zone: a zoned datetime draw is bounded"
                ),
                "{}",
                last_error(ctx)
            );
            complete_valid(ctx, tc);
            drain(ctx, run);
            ok(hegel_c::hegel_run_free(ctx, run));
            ok(hegel_zoned_spec_free(ctx, spec));
        }
        ok(hegel_timezone_spec_free(ctx, tz));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn a_draw_in_a_named_zone_validates_its_arguments() {
    let ctx = hegel_context_new();
    unsafe {
        let spec = seconds_spec(
            ctx,
            civil(2000, 1, 1, 0, 0, 0),
            civil(2030, 12, 31, 23, 59, 59),
        );
        let mut value = zoned();
        let name = s("Europe/London");
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                ptr::null_mut(),
                ptr::null(),
                name,
                spec,
                &mut value
            ),
            HEGEL_E_INVALID_HANDLE
        );
        let run = run_of(ctx, 1);
        let tc = next_case(ctx, run);
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                name,
                ptr::null(),
                &mut value
            ),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            last_error(ctx),
            "hegel_generate_zoned_datetime_for_zone: zoned spec handle is null"
        );
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                name,
                spec,
                ptr::null_mut()
            ),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_generate_zoned_datetime_for_zone: out parameter is null"
        );
        let null_name = hegel_string_t {
            data: ptr::null(),
            len: 1,
        };
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                null_name,
                spec,
                &mut value
            ),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_generate_zoned_datetime_for_zone: name is null"
        );
        let bad = [0xffu8];
        let bad_name = hegel_string_t {
            data: bad.as_ptr().cast(),
            len: 1,
        };
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                bad_name,
                spec,
                &mut value
            ),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_generate_zoned_datetime_for_zone: name is not valid UTF-8"
        );
        assert_eq!(
            hegel_generate_zoned_datetime_for_zone(
                ctx,
                tc,
                ptr::null(),
                s("Nowhere/Nope"),
                spec,
                &mut value
            ),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx).starts_with(
                "hegel_generate_zoned_datetime_for_zone: unknown time zone \"Nowhere/Nope\""
            ),
            "{}",
            last_error(ctx)
        );
        let mut bundled = ptr::null_mut();
        ok(hegel_zone_source_bundled(ctx, &mut bundled));
        ok(hegel_generate_zoned_datetime_for_zone(
            ctx, tc, bundled, name, spec, &mut value,
        ));
        check_zoned_value(ctx, bundled, "Europe/London", &value);
        ok(hegel_zone_source_free(ctx, bundled));
        complete_valid(ctx, tc);
        drain(ctx, run);
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_zoned_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn zoned_datetimes_format_with_their_gap_or_fold_marked() {
    let ctx = hegel_context_new();
    unsafe {
        let base = hegel_zoned_datetime_t {
            civil: civil(2024, 11, 3, 1, 30, 0),
            offset_seconds: -18_000,
            offset_known: true,
            fold: true,
            kind: HEGEL_TIMEZONE_KIND_FOLD,
            offset_before: -14_400,
            offset_after: -18_000,
            timestamp: at(1_730_615_400),
        };
        assert_eq!(
            zoned_text(ctx, &base, "America/New_York"),
            "2024-11-03T01:30:00-05:00[America/New_York][fold=1]"
        );
        let earlier = hegel_zoned_datetime_t {
            fold: false,
            offset_seconds: -14_400,
            ..base
        };
        assert_eq!(
            zoned_text(ctx, &earlier, "America/New_York"),
            "2024-11-03T01:30:00-04:00[America/New_York][fold=0]"
        );
        let gap = hegel_zoned_datetime_t {
            civil: civil(2024, 3, 10, 2, 30, 0),
            offset_seconds: -14_400,
            fold: false,
            kind: HEGEL_TIMEZONE_KIND_GAP,
            ..base
        };
        assert_eq!(
            zoned_text(ctx, &gap, "America/New_York"),
            "2024-03-10T02:30:00-04:00[America/New_York][gap]"
        );
        let unknown = hegel_zoned_datetime_t {
            civil: civil(2000, 1, 1, 0, 0, 0),
            offset_seconds: 0,
            offset_known: false,
            fold: false,
            kind: HEGEL_TIMEZONE_KIND_UNIQUE,
            ..base
        };
        assert_eq!(
            zoned_text(ctx, &unknown, "Etc/Unknown"),
            "2000-01-01T00:00:00Z[Etc/Unknown]"
        );
        assert_eq!(
            zoned_text(ctx, &unknown, "Mars/Base"),
            "2000-01-01T00:00:00-00:00[Mars/Base]"
        );
        let mut out = empty_result();
        assert_eq!(
            hegel_format_zoned_datetime(ctx, &base, s("UTC"), ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_format_zoned_datetime: out parameter is null"
        );
        assert_eq!(
            hegel_format_zoned_datetime(ctx, ptr::null(), s("UTC"), &mut out),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_format_zoned_datetime: value is null"
        );
        let bad = [0xffu8];
        let bad_zone = hegel_string_t {
            data: bad.as_ptr().cast(),
            len: 1,
        };
        assert_eq!(
            hegel_format_zoned_datetime(ctx, &base, bad_zone, &mut out),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_format_zoned_datetime: zone is not valid UTF-8"
        );
        let mut raw = std::mem::MaybeUninit::new(base);
        ptr::addr_of_mut!((*raw.as_mut_ptr()).kind)
            .cast::<u32>()
            .write(7);
        assert_eq!(
            hegel_format_zoned_datetime(ctx, raw.as_ptr(), s("UTC"), &mut out),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            last_error(ctx),
            "hegel_format_zoned_datetime: value.kind 7 is not a hegel_timezone_kind_t"
        );
        assert!(out.data.is_null());
    }
    unsafe { ok(hegel_context_free(ctx)) };
}
