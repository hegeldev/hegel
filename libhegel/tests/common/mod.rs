//! Shared helpers for the in-process C-ABI test binaries.

use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelRun, HegelSettings, HegelTestCase, HegelZoneSource,
    hegel_context_last_error, hegel_datetime_t, hegel_format_zoned_datetime, hegel_next_test_case,
    hegel_output_callback_t, hegel_owned_string_free, hegel_owned_string_t, hegel_run_start,
    hegel_settings_new, hegel_settings_set_database, hegel_string_t, hegel_timestamp_t,
    hegel_timezone_kind_t, hegel_timezone_offset_at_in, hegel_timezone_offset_t,
    hegel_timezone_resolution_t, hegel_timezone_resolve_in, hegel_zoned_datetime_t,
};
use std::os::raw::c_void;
use std::ptr;

/// Assert a call that should always succeed in these tests returned `HEGEL_OK`.
#[allow(dead_code)]
pub fn ok(rc: hegel_c::hegel_result_t) {
    assert_eq!(rc, HEGEL_OK);
}

#[allow(dead_code)]
pub fn last_error(ctx: *const HegelContext) -> String {
    let p = unsafe { hegel_context_last_error(ctx) };
    assert!(!p.data.is_null());
    assert_eq!(unsafe { *p.data.add(p.len) }, 0);
    String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(p.data.cast(), p.len) })
        .into_owned()
}

#[allow(dead_code)]
pub unsafe fn make_settings(ctx: *mut HegelContext) -> *mut HegelSettings {
    let mut s: *mut HegelSettings = ptr::null_mut();
    assert_eq!(unsafe { hegel_settings_new(ctx, &mut s) }, HEGEL_OK);
    assert!(!s.is_null());
    s
}

/// Like [`make_settings`], but with the failure database disabled.
#[allow(dead_code)]
pub unsafe fn make_settings_no_db(ctx: *mut HegelContext) -> *mut HegelSettings {
    let s = unsafe { make_settings(ctx) };
    assert_eq!(
        unsafe { hegel_settings_set_database(ctx, s, &"".into()) },
        HEGEL_OK
    );
    s
}

#[allow(dead_code)]
pub unsafe fn start(ctx: *mut HegelContext, s: *const HegelSettings) -> *mut HegelRun {
    unsafe { start_with_output(ctx, s, None, ptr::null_mut()) }
}

/// Like [`start`], but routing the run's engine output to `callback` /
/// `user_data` (see `hegel_run_start`).
#[allow(dead_code)]
pub unsafe fn start_with_output(
    ctx: *mut HegelContext,
    s: *const HegelSettings,
    callback: hegel_output_callback_t,
    user_data: *mut c_void,
) -> *mut HegelRun {
    let mut run: *mut HegelRun = ptr::null_mut();
    assert_eq!(
        unsafe { hegel_run_start(ctx, s, callback, user_data, &mut run) },
        HEGEL_OK
    );
    assert!(!run.is_null());
    run
}

/// The next case the run hands out, or null at completion (`HEGEL_OK` + null).
#[allow(dead_code)]
pub unsafe fn next_case(ctx: *mut HegelContext, run: *mut HegelRun) -> *mut HegelTestCase {
    let mut tc: *mut HegelTestCase = ptr::null_mut();
    assert_eq!(unsafe { hegel_next_test_case(ctx, run, &mut tc) }, HEGEL_OK);
    tc
}

/// The instant `civil` names read as UTC, in seconds since the epoch.
#[allow(dead_code)]
pub fn civil_second(civil: &hegel_datetime_t) -> i64 {
    jiff::civil::datetime(
        i16::try_from(civil.date.year).unwrap(),
        i8::try_from(civil.date.month).unwrap(),
        i8::try_from(civil.date.day).unwrap(),
        i8::try_from(civil.time.hour).unwrap(),
        i8::try_from(civil.time.minute).unwrap(),
        i8::try_from(civil.time.second).unwrap(),
        0,
    )
    .to_zoned(jiff::tz::TimeZone::UTC)
    .unwrap()
    .timestamp()
    .as_second()
}

/// The text `hegel_format_zoned_datetime` gives `value` in `zone`.
#[allow(dead_code)]
pub unsafe fn zoned_text(
    ctx: *mut HegelContext,
    value: &hegel_zoned_datetime_t,
    zone: &str,
) -> String {
    let mut out = hegel_owned_string_t {
        data: ptr::null(),
        len: 0,
    };
    let zone = hegel_string_t {
        data: zone.as_ptr().cast(),
        len: zone.len(),
    };
    ok(unsafe { hegel_format_zoned_datetime(ctx, value, zone, &mut out) });
    let text = String::from_utf8(
        unsafe { std::slice::from_raw_parts(out.data.cast::<u8>(), out.len) }.to_vec(),
    )
    .unwrap();
    ok(unsafe { hegel_owned_string_free(ctx, &mut out) });
    text
}

/// Check a drawn zoned datetime in the zone `name` of `source` (NULL for
/// the default source) against the queries a frontend could make of the
/// same zone: `hegel_timezone_resolve_in` of its wall time classifies it
/// the same way with the same offsets, its instant is its wall time less
/// the offset it reports, and `hegel_timezone_offset_at_in` has that
/// offset in force at its instant.
#[allow(dead_code)]
pub unsafe fn check_zoned_value(
    ctx: *mut HegelContext,
    source: *const HegelZoneSource,
    name: &str,
    value: &hegel_zoned_datetime_t,
) {
    use hegel_timezone_kind_t::*;
    let view = hegel_string_t {
        data: name.as_ptr().cast(),
        len: name.len(),
    };
    let mut r = hegel_timezone_resolution_t {
        kind: HEGEL_TIMEZONE_KIND_UNIQUE,
        offset_before: 0,
        offset_after: 0,
        offset_seconds: 0,
        offset_known: false,
        timestamp: hegel_timestamp_t {
            second: 0,
            nanosecond: 0,
        },
    };
    ok(unsafe { hegel_timezone_resolve_in(ctx, source, view, value.civil, &mut r) });
    let text = unsafe { zoned_text(ctx, value, name) };
    assert_eq!(value.kind, r.kind, "{text}");
    assert_eq!(value.offset_before, r.offset_before, "{text}");
    assert_eq!(value.offset_after, r.offset_after, "{text}");
    assert_eq!(value.offset_known, r.offset_known, "{text}");
    assert_eq!(
        value.timestamp.nanosecond, value.civil.time.nanosecond,
        "{text}"
    );
    let wall = civil_second(&value.civil);
    let suffix = match value.kind {
        HEGEL_TIMEZONE_KIND_UNIQUE => {
            assert!(!value.fold, "{text}");
            assert_eq!(value.offset_before, value.offset_after, "{text}");
            assert_eq!(value.offset_seconds, value.offset_before, "{text}");
            assert_eq!(
                value.timestamp.second,
                wall - i64::from(value.offset_seconds),
                "{text}"
            );
            ""
        }
        HEGEL_TIMEZONE_KIND_GAP => {
            assert!(!value.fold, "{text}");
            assert!(value.offset_before < value.offset_after, "{text}");
            assert_eq!(value.offset_seconds, value.offset_after, "{text}");
            assert_eq!(
                value.timestamp.second,
                wall - i64::from(value.offset_before),
                "{text}"
            );
            "[gap]"
        }
        HEGEL_TIMEZONE_KIND_FOLD => {
            assert!(value.offset_before > value.offset_after, "{text}");
            let offset = if value.fold {
                value.offset_after
            } else {
                value.offset_before
            };
            assert_eq!(value.offset_seconds, offset, "{text}");
            assert_eq!(value.timestamp.second, wall - i64::from(offset), "{text}");
            if value.fold { "[fold=1]" } else { "[fold=0]" }
        }
    };
    if !value.fold {
        assert_eq!(value.timestamp.second, r.timestamp.second, "{text}");
        assert_eq!(value.offset_seconds, r.offset_seconds, "{text}");
    }
    let mut offset = hegel_timezone_offset_t {
        offset_seconds: 0,
        offset_known: false,
        is_dst: false,
        abbreviation: hegel_string_t::default(),
    };
    ok(unsafe { hegel_timezone_offset_at_in(ctx, source, view, value.timestamp, &mut offset) });
    assert_eq!(offset.offset_known, value.offset_known, "{text}");
    assert_eq!(offset.offset_seconds, value.offset_seconds, "{text}");
    assert!(text.ends_with(&format!("[{name}]{suffix}")), "{text}");
}
