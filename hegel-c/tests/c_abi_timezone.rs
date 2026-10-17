//! In-process exercise of the `hegel_timezone_*` queries: argument
//! checking and the values a frontend reads back, against known facts about
//! a few zones. The evaluator itself is checked against jiff in the
//! embedded `timezone_tests`.

mod common;

use common::{last_error, ok};
use hegel_c::hegel_result_t::*;
use hegel_c::hegel_timezone_kind_t::*;
use hegel_c::{
    HEGEL_TIMEZONE_TAG_DST, HEGEL_TIMEZONE_TAG_FIXED, HEGEL_TIMEZONE_TAG_INVERTED_SIGN,
    HEGEL_TIMEZONE_TAG_LEGACY, HEGEL_TIMEZONE_TAG_LINK, HEGEL_TIMEZONE_TAG_UNDEFINED,
    HEGEL_TIMEZONE_TAG_WHOLE_HOUR, HegelContext, hegel_context_free, hegel_context_new,
    hegel_date_t, hegel_datetime_t, hegel_time_t, hegel_timestamp_t, hegel_timezone_info,
    hegel_timezone_info_t, hegel_timezone_offset_at, hegel_timezone_offset_t,
    hegel_timezone_resolution_t, hegel_timezone_resolve,
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

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

const MIN_INSTANT: i64 = -31_619_087_596_800;
const MAX_INSTANT: i64 = 31_494_784_780_799;

fn at(second: i64) -> hegel_timestamp_t {
    hegel_timestamp_t {
        second,
        nanosecond: 0,
    }
}

fn resolution() -> hegel_timezone_resolution_t {
    hegel_timezone_resolution_t {
        kind: HEGEL_TIMEZONE_KIND_UNIQUE,
        offset_before: 0,
        offset_after: 0,
        offset_seconds: 0,
        offset_known: false,
        timestamp: at(0),
    }
}

fn offset() -> hegel_timezone_offset_t {
    hegel_timezone_offset_t {
        offset_seconds: 0,
        offset_known: false,
        is_dst: false,
        abbreviation: ptr::null(),
    }
}

fn info() -> hegel_timezone_info_t {
    hegel_timezone_info_t {
        known: false,
        primary_name: ptr::null(),
        is_link: false,
        rank: 0,
        tags: 0,
    }
}

unsafe fn resolve(
    ctx: *mut HegelContext,
    name: &CStr,
    civil: hegel_datetime_t,
) -> hegel_timezone_resolution_t {
    let mut out = resolution();
    ok(unsafe { hegel_timezone_resolve(ctx, name.as_ptr(), civil, &mut out) });
    out
}

unsafe fn offset_at(
    ctx: *mut HegelContext,
    name: &CStr,
    timestamp: hegel_timestamp_t,
) -> hegel_timezone_offset_t {
    let mut out = offset();
    ok(unsafe { hegel_timezone_offset_at(ctx, name.as_ptr(), timestamp, &mut out) });
    out
}

unsafe fn info_of(ctx: *mut HegelContext, name: &CStr) -> hegel_timezone_info_t {
    let mut out = info();
    ok(unsafe { hegel_timezone_info(ctx, name.as_ptr(), &mut out) });
    out
}

fn string(p: *const c_char) -> &'static str {
    assert!(!p.is_null());
    unsafe { CStr::from_ptr(p) }.to_str().unwrap()
}

const NEW_YORK_2024_03_10: i64 = 1_710_054_000;
const NEW_YORK_2024_11_03: i64 = 1_730_613_600;

#[test]
fn resolve_classifies_a_wall_time() {
    let ctx = hegel_context_new();
    let name = CString::new("America/New_York").unwrap();
    let gap = unsafe { resolve(ctx, &name, civil(2024, 3, 10, 2, 30, 0)) };
    assert_eq!(gap.kind, HEGEL_TIMEZONE_KIND_GAP);
    assert_eq!((gap.offset_before, gap.offset_after), (-18000, -14400));
    assert_eq!(gap.offset_seconds, -14400);
    assert!(gap.offset_known);
    assert_eq!(gap.timestamp.second, NEW_YORK_2024_03_10 + 1800);
    assert_eq!(gap.timestamp.nanosecond, 0);
    let fold = unsafe { resolve(ctx, &name, civil(2024, 11, 3, 1, 30, 0)) };
    assert_eq!(fold.kind, HEGEL_TIMEZONE_KIND_FOLD);
    assert_eq!((fold.offset_before, fold.offset_after), (-14400, -18000));
    assert_eq!(fold.offset_seconds, -14400);
    assert_eq!(fold.timestamp.second, NEW_YORK_2024_11_03 - 1800);
    let mut unique = civil(2024, 7, 4, 12, 0, 0);
    unique.time.nanosecond = 42;
    let unique = unsafe { resolve(ctx, &name, unique) };
    assert_eq!(unique.kind, HEGEL_TIMEZONE_KIND_UNIQUE);
    assert_eq!(
        (unique.offset_before, unique.offset_after),
        (-14400, -14400)
    );
    assert_eq!(unique.timestamp.second, 1_720_108_800);
    assert_eq!(unique.timestamp.nanosecond, 42);
    let unknown = unsafe { resolve(ctx, c"Etc/Unknown", civil(2024, 7, 4, 12, 0, 0)) };
    assert_eq!(unknown.kind, HEGEL_TIMEZONE_KIND_UNIQUE);
    assert!(!unknown.offset_known);
    assert_eq!(unknown.offset_seconds, 0);
    assert_eq!(unknown.timestamp.second, 1_720_094_400);
    ok(unsafe { hegel_context_free(ctx) });
}

#[test]
fn offset_at_reports_the_type_in_force() {
    let ctx = hegel_context_new();
    let name = CString::new("America/New_York").unwrap();
    let winter = unsafe { offset_at(ctx, &name, at(NEW_YORK_2024_03_10 - 1)) };
    assert_eq!(winter.offset_seconds, -18000);
    assert!(winter.offset_known);
    assert!(!winter.is_dst);
    assert_eq!(string(winter.abbreviation), "EST");
    let summer = unsafe { offset_at(ctx, &name, at(NEW_YORK_2024_03_10)) };
    assert_eq!(summer.offset_seconds, -14400);
    assert!(summer.is_dst);
    assert_eq!(string(summer.abbreviation), "EDT");
    let far = unsafe { offset_at(ctx, &name, at(MAX_INSTANT)) };
    assert!([-18000, -14400].contains(&far.offset_seconds));
    let early = unsafe { offset_at(ctx, &name, at(MIN_INSTANT)) };
    assert_eq!(early.offset_seconds, -17762);
    assert_eq!(string(early.abbreviation), "LMT");
    let kolkata = unsafe { offset_at(ctx, c"Asia/Calcutta", at(0)) };
    assert_eq!(kolkata.offset_seconds, 19800);
    assert_eq!(string(kolkata.abbreviation), "IST");
    let unknown = unsafe { offset_at(ctx, c"Etc/Unknown", at(0)) };
    assert_eq!(unknown.offset_seconds, 0);
    assert!(!unknown.offset_known);
    assert!(!unknown.is_dst);
    assert_eq!(string(unknown.abbreviation), "-00");
    let with_nanos = unsafe {
        offset_at(
            ctx,
            &name,
            hegel_timestamp_t {
                second: NEW_YORK_2024_03_10 - 1,
                nanosecond: 999_999_999,
            },
        )
    };
    assert_eq!(with_nanos.offset_seconds, -18000);
    ok(unsafe { hegel_context_free(ctx) });
}

#[test]
fn info_describes_known_and_unknown_names() {
    let ctx = hegel_context_new();
    let utc = unsafe { info_of(ctx, c"UTC") };
    assert!(utc.known);
    assert_eq!(string(utc.primary_name), "UTC");
    assert!(!utc.is_link);
    assert_eq!(utc.rank, 0);
    assert_eq!(
        utc.tags,
        HEGEL_TIMEZONE_TAG_FIXED | HEGEL_TIMEZONE_TAG_WHOLE_HOUR
    );
    let calcutta = unsafe { info_of(ctx, c"Asia/Calcutta") };
    assert!(calcutta.known);
    assert_eq!(string(calcutta.primary_name), "Asia/Kolkata");
    assert!(calcutta.is_link);
    assert_ne!(calcutta.tags & HEGEL_TIMEZONE_TAG_LINK, 0);
    assert_ne!(calcutta.rank, 0);
    let gmt_plus_5 = unsafe { info_of(ctx, c"Etc/GMT+5") };
    assert_ne!(gmt_plus_5.tags & HEGEL_TIMEZONE_TAG_INVERTED_SIGN, 0);
    let est5edt = unsafe { info_of(ctx, c"EST5EDT") };
    assert_ne!(est5edt.tags & HEGEL_TIMEZONE_TAG_LEGACY, 0);
    assert_ne!(est5edt.tags & HEGEL_TIMEZONE_TAG_DST, 0);
    let unknown = unsafe { info_of(ctx, c"Etc/Unknown") };
    assert!(unknown.known);
    assert_eq!(string(unknown.primary_name), "Etc/Unknown");
    assert!(!unknown.is_link);
    assert_eq!(unknown.rank, u16::MAX);
    assert_eq!(unknown.tags, HEGEL_TIMEZONE_TAG_UNDEFINED);
    let nowhere = unsafe { info_of(ctx, c"Nowhere/Nope") };
    assert!(!nowhere.known);
    assert!(nowhere.primary_name.is_null());
    assert!(!nowhere.is_link);
    assert_eq!((nowhere.rank, nowhere.tags), (0, 0));
    assert!(last_error(ctx).is_empty());
    ok(unsafe { hegel_context_free(ctx) });
}

#[test]
fn arguments_are_checked() {
    let ctx = hegel_context_new();
    let name = CString::new("America/New_York").unwrap();
    let mut r = resolution();
    let mut o = offset();
    let mut i = info();
    assert_eq!(
        unsafe { hegel_timezone_resolve(ctx, ptr::null(), civil(2024, 1, 1, 0, 0, 0), &mut r) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(last_error(ctx), "hegel_timezone_resolve: name is null");
    assert_eq!(
        unsafe { hegel_timezone_offset_at(ctx, ptr::null(), at(0), &mut o) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(last_error(ctx), "hegel_timezone_offset_at: name is null");
    assert_eq!(
        unsafe { hegel_timezone_info(ctx, ptr::null(), &mut i) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(last_error(ctx), "hegel_timezone_info: name is null");
    let bad_utf8 = CString::new([0xffu8, 0xfe]).unwrap();
    assert_eq!(
        unsafe { hegel_timezone_info(ctx, bad_utf8.as_ptr(), &mut i) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(
        last_error(ctx),
        "hegel_timezone_info: name is not valid UTF-8"
    );
    assert_eq!(
        unsafe {
            hegel_timezone_resolve(
                ctx,
                name.as_ptr(),
                civil(2024, 1, 1, 0, 0, 0),
                ptr::null_mut(),
            )
        },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(
        last_error(ctx),
        "hegel_timezone_resolve: out parameter is null"
    );
    assert_eq!(
        unsafe { hegel_timezone_offset_at(ctx, name.as_ptr(), at(0), ptr::null_mut()) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(
        last_error(ctx),
        "hegel_timezone_offset_at: out parameter is null"
    );
    assert_eq!(
        unsafe { hegel_timezone_info(ctx, name.as_ptr(), ptr::null_mut()) },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(
        last_error(ctx),
        "hegel_timezone_info: out parameter is null"
    );
    let nowhere = CString::new("Nowhere/Nope").unwrap();
    assert_eq!(
        unsafe {
            hegel_timezone_resolve(ctx, nowhere.as_ptr(), civil(2024, 1, 1, 0, 0, 0), &mut r)
        },
        HEGEL_E_INVALID_ARG
    );
    assert_eq!(
        last_error(ctx),
        "hegel_timezone_resolve: unknown time zone \"Nowhere/Nope\"; names are matched \
         exactly as the IANA database spells them, plus Etc/Unknown"
    );
    assert_eq!(
        unsafe { hegel_timezone_offset_at(ctx, nowhere.as_ptr(), at(0), &mut o) },
        HEGEL_E_INVALID_ARG
    );
    assert!(last_error(ctx).starts_with("hegel_timezone_offset_at: unknown time zone"));
    assert_eq!(
        unsafe { hegel_timezone_resolve(ctx, name.as_ptr(), civil(2024, 2, 30, 0, 0, 0), &mut r) },
        HEGEL_E_INVALID_ARG
    );
    assert!(
        last_error(ctx).contains("civil.date is not a valid date"),
        "{}",
        last_error(ctx)
    );
    assert_eq!(
        unsafe { hegel_timezone_resolve(ctx, name.as_ptr(), civil(2024, 2, 29, 0, 60, 0), &mut r) },
        HEGEL_E_INVALID_ARG
    );
    assert!(
        last_error(ctx).contains("civil.time is not a valid time"),
        "{}",
        last_error(ctx)
    );
    let bad_nanos = hegel_timestamp_t {
        second: 0,
        nanosecond: 1_000_000_000,
    };
    assert_eq!(
        unsafe { hegel_timezone_offset_at(ctx, name.as_ptr(), bad_nanos, &mut o) },
        HEGEL_E_INVALID_ARG
    );
    assert!(
        last_error(ctx).contains("nanosecond"),
        "{}",
        last_error(ctx)
    );
    for second in [i64::MIN, MIN_INSTANT - 1, MAX_INSTANT + 1, i64::MAX] {
        assert_eq!(
            unsafe { hegel_timezone_offset_at(ctx, name.as_ptr(), at(second), &mut o) },
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx).contains("timestamp.second must be in"),
            "{}",
            last_error(ctx)
        );
        assert_eq!(
            unsafe { hegel_timezone_offset_at(ctx, c"Etc/Unknown".as_ptr(), at(second), &mut o) },
            HEGEL_E_INVALID_ARG
        );
    }
    ok(unsafe { hegel_context_free(ctx) });
}
