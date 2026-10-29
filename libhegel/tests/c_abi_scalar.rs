//! In-process exercise of the timestamp, offset, duration and span draws,
//! the date options and the precision arguments over the C ABI: handle
//! construction and argument checking, and the values a frontend reads
//! back. The draws themselves are checked in the embedded `scalar_tests`
//! and `special_tests`.

mod common;

use common::{last_error, make_settings_no_db, next_case, ok, start};
use hegel_c::hegel_offset_granularity_t::*;
use hegel_c::hegel_precision_t::*;
use hegel_c::hegel_result_t::*;
use hegel_c::hegel_span_sign_t::*;
use hegel_c::hegel_span_unit_t::*;
use hegel_c::{
    HegelContext, HegelDateOptions, HegelRun, HegelSpanSpec, HegelTestCase, hegel_context_free,
    hegel_context_new, hegel_date_options_free, hegel_date_options_new,
    hegel_date_options_set_months, hegel_date_options_set_weekdays, hegel_date_t, hegel_datetime_t,
    hegel_duration_t, hegel_generate_date, hegel_generate_datetime, hegel_generate_duration,
    hegel_generate_offset, hegel_generate_span, hegel_generate_time, hegel_generate_timestamp,
    hegel_mark_complete, hegel_offset_t, hegel_settings_set_test_cases, hegel_span_spec_free,
    hegel_span_spec_new, hegel_span_spec_set_sign, hegel_span_spec_set_unit_bounds, hegel_span_t,
    hegel_status_t, hegel_time_t, hegel_timestamp_t,
};
use std::collections::BTreeSet;
use std::ptr;

type Call<'a> = (&'a str, Box<dyn Fn() -> hegel_c::hegel_result_t + 'a>);

const NANOSECONDS: u32 = HEGEL_PRECISION_NANOSECONDS as u32;
const SECONDS: u32 = HEGEL_PRECISION_SECONDS as u32;

fn date(year: i32, month: u8, day: u8) -> hegel_date_t {
    hegel_date_t { year, month, day }
}

fn time(hour: u8, minute: u8, second: u8, nanosecond: u32) -> hegel_time_t {
    hegel_time_t {
        hour,
        minute,
        second,
        nanosecond,
    }
}

fn at(second: i64) -> hegel_timestamp_t {
    hegel_timestamp_t {
        second,
        nanosecond: 0,
    }
}

fn duration(seconds: i64, nanoseconds: i32) -> hegel_duration_t {
    hegel_duration_t {
        seconds,
        nanoseconds,
    }
}

fn span() -> hegel_span_t {
    hegel_span_t {
        years: 0,
        months: 0,
        weeks: 0,
        days: 0,
        hours: 0,
        minutes: 0,
        seconds: 0,
        milliseconds: 0,
        microseconds: 0,
        nanoseconds: 0,
    }
}

unsafe fn date_options(ctx: *mut HegelContext) -> *mut HegelDateOptions {
    let mut options: *mut HegelDateOptions = ptr::null_mut();
    unsafe { ok(hegel_date_options_new(ctx, &mut options)) };
    assert!(!options.is_null());
    options
}

unsafe fn span_spec(ctx: *mut HegelContext) -> *mut HegelSpanSpec {
    let mut spec: *mut HegelSpanSpec = ptr::null_mut();
    unsafe { ok(hegel_span_spec_new(ctx, &mut spec)) };
    assert!(!spec.is_null());
    spec
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

unsafe fn run_of(ctx: *mut HegelContext, cases: u64) -> *mut HegelRun {
    let s = unsafe { make_settings_no_db(ctx) };
    unsafe { ok(hegel_settings_set_test_cases(ctx, s, cases)) };
    let run = unsafe { start(ctx, s) };
    unsafe { ok(hegel_c::hegel_settings_free(ctx, s)) };
    run
}

/// Run `draw` on every test case of a fresh run of `cases`, collecting
/// what it returns.
unsafe fn each_case<T>(
    ctx: *mut HegelContext,
    cases: u64,
    mut draw: impl FnMut(*mut HegelTestCase) -> T,
) -> Vec<T> {
    let run = unsafe { run_of(ctx, cases) };
    let mut out = Vec::new();
    loop {
        let tc = unsafe { next_case(ctx, run) };
        if tc.is_null() {
            break;
        }
        out.push(draw(tc));
        unsafe { complete_valid(ctx, tc) };
    }
    unsafe { ok(hegel_c::hegel_run_free(ctx, run)) };
    out
}

fn jiff_date(d: &hegel_date_t) -> jiff::civil::Date {
    jiff::civil::date(
        i16::try_from(d.year).unwrap(),
        i8::try_from(d.month).unwrap(),
        i8::try_from(d.day).unwrap(),
    )
}

#[test]
fn date_options_validate_their_arguments_and_mask_the_draw() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_date_options_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_date_options_new"));
        ok(hegel_date_options_free(ctx, ptr::null_mut()));
        let null_handle: [Call<'_>; 2] = [
            (
                "hegel_date_options_set_weekdays",
                Box::new(|| hegel_date_options_set_weekdays(ctx, ptr::null_mut(), 1)),
            ),
            (
                "hegel_date_options_set_months",
                Box::new(|| hegel_date_options_set_months(ctx, ptr::null_mut(), 1)),
            ),
        ];
        for (name, call) in &null_handle {
            assert_eq!(call(), HEGEL_E_INVALID_HANDLE, "{name}");
            assert!(last_error(ctx).contains(name), "{}", last_error(ctx));
        }
        let options = date_options(ctx);
        for mask in [0, 0x80, 0x100, u32::MAX] {
            assert_eq!(
                hegel_date_options_set_weekdays(ctx, options, mask),
                HEGEL_E_INVALID_ARG,
                "{mask:#x}"
            );
            assert!(last_error(ctx).contains("weekdays mask"));
        }
        for mask in [0, 0x1000, 0x10000, u32::MAX] {
            assert_eq!(
                hegel_date_options_set_months(ctx, options, mask),
                HEGEL_E_INVALID_ARG,
                "{mask:#x}"
            );
            assert!(last_error(ctx).contains("months mask"));
        }
        ok(hegel_date_options_set_weekdays(ctx, options, 1));
        ok(hegel_date_options_set_months(ctx, options, 1 << 1));
        let (min, max) = (date(1990, 1, 1), date(2010, 12, 31));
        let dates = each_case(ctx, 50, |tc| {
            let mut d = date(0, 0, 0);
            ok(hegel_generate_date(ctx, tc, min, max, options, &mut d));
            let mut any = date(0, 0, 0);
            ok(hegel_generate_date(
                ctx,
                tc,
                min,
                max,
                ptr::null(),
                &mut any,
            ));
            assert!(
                hegel_generate_date(
                    ctx,
                    tc,
                    date(2000, 3, 1),
                    date(2001, 1, 31),
                    options,
                    &mut any
                ) == HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("allowed weekday and month"));
            (d, any)
        });
        for (d, any) in &dates {
            assert_eq!(jiff_date(d).weekday(), jiff::civil::Weekday::Monday);
            assert_eq!(d.month, 2);
            assert!((1990..=2010).contains(&any.year));
        }
        assert!(dates.iter().any(|(_, any)| any.month != 2));
        ok(hegel_date_options_free(ctx, options));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn times_and_datetimes_take_a_precision() {
    let ctx = hegel_context_new();
    unsafe {
        let (min, max) = (time(0, 0, 0, 0), time(23, 59, 59, 999_999_999));
        let civil = |t: hegel_time_t| hegel_datetime_t {
            date: date(2000, 1, 1),
            time: t,
        };
        let values = each_case(ctx, 30, |tc| {
            let mut t = time(0, 0, 0, 0);
            assert_eq!(
                hegel_generate_time(ctx, tc, min, max, 6, &mut t),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("unknown precision 6"));
            assert_eq!(
                hegel_generate_time(
                    ctx,
                    tc,
                    time(0, 0, 0, 500),
                    time(0, 0, 0, 900),
                    SECONDS,
                    &mut t
                ),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("no value at precision"));
            ok(hegel_generate_time(ctx, tc, min, max, SECONDS, &mut t));
            let mut dt = civil(t);
            assert_eq!(
                hegel_generate_datetime(ctx, tc, civil(min), civil(max), 7, &mut dt),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("unknown precision 7"));
            assert_eq!(
                hegel_generate_datetime(
                    ctx,
                    tc,
                    civil(time(0, 0, 0, 500)),
                    civil(time(0, 0, 0, 900)),
                    SECONDS,
                    &mut dt
                ),
                HEGEL_E_INVALID_ARG
            );
            ok(hegel_generate_datetime(
                ctx,
                tc,
                civil(min),
                civil(max),
                HEGEL_PRECISION_MINUTES as u32,
                &mut dt,
            ));
            let mut fine = time(0, 0, 0, 0);
            ok(hegel_generate_time(
                ctx,
                tc,
                min,
                max,
                NANOSECONDS,
                &mut fine,
            ));
            (t, dt, fine)
        });
        for (t, dt, _) in &values {
            assert_eq!(t.nanosecond, 0);
            assert_eq!((dt.time.second, dt.time.nanosecond), (0, 0));
            assert_eq!((dt.date.year, dt.date.month, dt.date.day), (2000, 1, 1));
        }
        assert!(values.iter().any(|(_, _, fine)| fine.nanosecond != 0));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn timestamps_offsets_and_durations_validate_and_draw() {
    let ctx = hegel_context_new();
    unsafe {
        let mut t = at(0);
        let mut o = hegel_offset_t {
            seconds: 0,
            known: false,
        };
        let mut d = duration(0, 0);
        assert_eq!(
            hegel_generate_timestamp(ctx, ptr::null_mut(), at(0), at(1), SECONDS, &mut t),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_generate_offset(ctx, ptr::null_mut(), 0, 1, 0, false, &mut o),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_generate_duration(
                ctx,
                ptr::null_mut(),
                duration(0, 0),
                duration(1, 0),
                SECONDS,
                &mut d
            ),
            HEGEL_E_INVALID_HANDLE
        );
        let (lo, hi) = (at(-100_000_000_000), at(100_000_000_000));
        let values = each_case(ctx, 200, |tc| {
            let mut t = at(0);
            assert_eq!(
                hegel_generate_timestamp(ctx, tc, lo, hi, SECONDS, ptr::null_mut()),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("out parameter is null"));
            assert_eq!(
                hegel_generate_timestamp(ctx, tc, lo, hi, 9, &mut t),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("unknown precision 9"));
            assert_eq!(
                hegel_generate_timestamp(ctx, tc, hi, lo, SECONDS, &mut t),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("min_value <= max_value"));
            ok(hegel_generate_timestamp(
                ctx,
                tc,
                lo,
                hi,
                HEGEL_PRECISION_MILLISECONDS as u32,
                &mut t,
            ));
            let mut hourly = at(0);
            ok(hegel_generate_timestamp(
                ctx,
                tc,
                lo,
                hi,
                HEGEL_PRECISION_HOURS as u32,
                &mut hourly,
            ));
            assert_eq!((hourly.second % 3_600, hourly.nanosecond), (0, 0));
            let mut o = hegel_offset_t {
                seconds: 0,
                known: false,
            };
            assert_eq!(
                hegel_generate_offset(ctx, tc, 0, 1, 0, false, ptr::null_mut()),
                HEGEL_E_INVALID_ARG
            );
            assert_eq!(
                hegel_generate_offset(ctx, tc, 0, 1, 4, false, &mut o),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("unknown granularity 4"));
            assert_eq!(
                hegel_generate_offset(ctx, tc, 1, 3_599, 0, false, &mut o),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("no offset at granularity"));
            ok(hegel_generate_offset(
                ctx,
                tc,
                -50_400,
                50_400,
                HEGEL_OFFSET_GRANULARITY_QUARTER_HOURS as u32,
                true,
                &mut o,
            ));
            let mut known = o;
            ok(hegel_generate_offset(
                ctx,
                tc,
                -50_400,
                50_400,
                HEGEL_OFFSET_GRANULARITY_HOURS as u32,
                false,
                &mut known,
            ));
            for granularity in [
                HEGEL_OFFSET_GRANULARITY_MINUTES,
                HEGEL_OFFSET_GRANULARITY_SECONDS,
            ] {
                let mut fine = o;
                ok(hegel_generate_offset(
                    ctx,
                    tc,
                    -90,
                    90,
                    granularity as u32,
                    false,
                    &mut fine,
                ));
                assert!(fine.known && (-90..=90).contains(&fine.seconds));
                assert!(granularity != HEGEL_OFFSET_GRANULARITY_MINUTES || fine.seconds % 60 == 0);
            }
            let mut d = duration(0, 0);
            assert_eq!(
                hegel_generate_duration(
                    ctx,
                    tc,
                    duration(0, 0),
                    duration(1, 0),
                    SECONDS,
                    ptr::null_mut()
                ),
                HEGEL_E_INVALID_ARG
            );
            assert_eq!(
                hegel_generate_duration(ctx, tc, duration(0, 0), duration(1, 0), 8, &mut d),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("unknown precision 8"));
            assert_eq!(
                hegel_generate_duration(ctx, tc, duration(-1, 1), duration(1, 0), SECONDS, &mut d),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("not a valid duration"));
            ok(hegel_generate_duration(
                ctx,
                tc,
                duration(-3_600, -500_000_000),
                duration(3_600, 500_000_000),
                HEGEL_PRECISION_MICROSECONDS as u32,
                &mut d,
            ));
            (t, o, known, d)
        });
        let mut unknowns = 0;
        for (t, o, known, d) in &values {
            assert!((lo.second..=hi.second).contains(&t.second), "{}", t.second);
            assert_eq!(t.nanosecond % 1_000_000, 0);
            if o.known {
                assert_eq!(o.seconds % 900, 0);
                assert!((-50_400..=50_400).contains(&o.seconds));
            } else {
                assert_eq!(o.seconds, 0);
                unknowns += 1;
            }
            assert!(known.known);
            assert_eq!(known.seconds % 3_600, 0);
            let ns = i128::from(d.seconds) * 1_000_000_000 + i128::from(d.nanoseconds);
            assert!(
                (-3_600_500_000_000..=3_600_500_000_000).contains(&ns),
                "{ns}"
            );
            assert_eq!(ns % 1_000, 0);
            assert!(d.seconds == 0 || d.nanoseconds == 0 || (d.seconds < 0) == (d.nanoseconds < 0));
        }
        assert!((5..=60).contains(&unknowns), "{unknowns}");
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn span_specs_validate_their_arguments_and_draw() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_span_spec_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_span_spec_new"));
        ok(hegel_span_spec_free(ctx, ptr::null_mut()));
        let null_handle: [Call<'_>; 2] = [
            (
                "hegel_span_spec_set_unit_bounds",
                Box::new(|| hegel_span_spec_set_unit_bounds(ctx, ptr::null_mut(), 0, 0, 1)),
            ),
            (
                "hegel_span_spec_set_sign",
                Box::new(|| hegel_span_spec_set_sign(ctx, ptr::null_mut(), 0)),
            ),
        ];
        for (name, call) in &null_handle {
            assert_eq!(call(), HEGEL_E_INVALID_HANDLE, "{name}");
            assert!(last_error(ctx).contains(name), "{}", last_error(ctx));
        }
        let spec = span_spec(ctx);
        let mut out = span();
        assert_eq!(
            hegel_generate_span(ctx, ptr::null_mut(), spec, &mut out),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_span_spec_set_unit_bounds(ctx, spec, 10, 0, 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown unit 10"));
        assert_eq!(
            hegel_span_spec_set_unit_bounds(ctx, spec, HEGEL_SPAN_UNIT_YEARS as u32, 1, 0),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("min <= max"));
        assert_eq!(hegel_span_spec_set_sign(ctx, spec, 3), HEGEL_E_INVALID_ARG);
        assert!(last_error(ctx).contains("unknown sign policy 3"));
        ok(hegel_span_spec_set_sign(
            ctx,
            spec,
            HEGEL_SPAN_SIGN_UNIFORM as u32,
        ));
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            spec,
            HEGEL_SPAN_UNIT_YEARS as u32,
            -3,
            3,
        ));
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            spec,
            HEGEL_SPAN_UNIT_MONTHS as u32,
            0,
            11,
        ));
        assert_eq!(
            hegel_span_spec_set_sign(ctx, spec, HEGEL_SPAN_SIGN_NONNEGATIVE as u32),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("requires min >= 0"));
        let conflict = span_spec(ctx);
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            conflict,
            HEGEL_SPAN_UNIT_WEEKS as u32,
            1,
            2,
        ));
        assert_eq!(
            hegel_span_spec_set_unit_bounds(ctx, conflict, HEGEL_SPAN_UNIT_DAYS as u32, -5, -1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("one sign"));
        ok(hegel_span_spec_free(ctx, conflict));
        let empty = span_spec(ctx);
        let values = each_case(ctx, 100, |tc| {
            let mut out = span();
            assert_eq!(
                hegel_generate_span(ctx, tc, ptr::null(), &mut out),
                HEGEL_E_INVALID_HANDLE
            );
            assert!(last_error(ctx).contains("spec handle is null"));
            assert_eq!(
                hegel_generate_span(ctx, tc, spec, ptr::null_mut()),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("out parameter is null"));
            assert_eq!(
                hegel_generate_span(ctx, tc, empty, &mut out),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains("no unit is enabled"));
            ok(hegel_generate_span(ctx, tc, spec, &mut out));
            out
        });
        let mut seen = BTreeSet::new();
        for s in &values {
            assert!((-3..=3).contains(&s.years), "{}", s.years);
            assert!((0..=11).contains(&s.months), "{}", s.months);
            assert!(s.years >= 0 || s.months == 0);
            assert_eq!(
                (s.weeks, s.days, s.hours, s.minutes, s.seconds),
                (0, 0, 0, 0, 0)
            );
            assert_eq!((s.milliseconds, s.microseconds, s.nanoseconds), (0, 0, 0));
            seen.insert((s.years, s.months));
        }
        assert!(seen.iter().any(|&(y, _)| y < 0));
        assert!(seen.iter().any(|&(_, m)| m > 0));
        ok(hegel_span_spec_set_sign(
            ctx,
            spec,
            HEGEL_SPAN_SIGN_MIXED as u32,
        ));
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            spec,
            HEGEL_SPAN_UNIT_DAYS as u32,
            -5,
            -1,
        ));
        let values = each_case(ctx, 50, |tc| {
            let mut out = span();
            ok(hegel_generate_span(ctx, tc, spec, &mut out));
            out
        });
        assert!(values.iter().all(|s| (-5..=-1).contains(&s.days)));
        assert!(values.iter().any(|s| s.years > 0));
        ok(hegel_span_spec_free(ctx, empty));
        ok(hegel_span_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}
