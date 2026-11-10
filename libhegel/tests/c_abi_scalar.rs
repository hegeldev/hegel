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
    hegel_duration_t, hegel_format_date, hegel_format_datetime, hegel_format_duration,
    hegel_format_offset, hegel_format_span, hegel_format_time, hegel_format_timestamp,
    hegel_generate_date, hegel_generate_datetime, hegel_generate_duration, hegel_generate_offset,
    hegel_generate_span, hegel_generate_time, hegel_generate_timestamp, hegel_mark_complete,
    hegel_offset_t, hegel_owned_string_free, hegel_owned_string_t, hegel_settings_set_test_cases,
    hegel_span_spec_free, hegel_span_spec_new, hegel_span_spec_set_sign,
    hegel_span_spec_set_unit_bounds, hegel_span_spec_validate, hegel_span_t, hegel_status_t,
    hegel_time_t, hegel_timestamp_t, hegel_validate_date, hegel_validate_datetime,
    hegel_validate_duration, hegel_validate_offset, hegel_validate_time, hegel_validate_timestamp,
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
        assert_eq!(
            last_error(ctx),
            "hegel_span_spec_set_unit_bounds: min_value <= max_value is required for Years, \
             got [1, 0]"
        );
        assert_eq!(hegel_span_spec_set_sign(ctx, spec, 3), HEGEL_E_INVALID_ARG);
        assert_eq!(
            last_error(ctx),
            "hegel_span_spec_set_sign: unknown sign policy 3"
        );
        assert_eq!(hegel_span_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        assert_eq!(
            last_error(ctx),
            "hegel_span_spec_validate: a span draw needs a unit enabled; set bounds on at least \
             one"
        );
        assert_eq!(
            hegel_span_spec_validate(ctx, ptr::null()),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("hegel_span_spec_validate: spec pointer is null"));
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
        ok(hegel_span_spec_validate(ctx, spec));
        ok(hegel_span_spec_set_sign(
            ctx,
            spec,
            HEGEL_SPAN_SIGN_NONNEGATIVE as u32,
        ));
        assert_eq!(hegel_span_spec_validate(ctx, spec), HEGEL_E_INVALID_ARG);
        assert!(
            last_error(ctx).contains(
                "hegel_span_spec_validate: a span draw under the nonnegative sign policy \
                 requires min >= 0 for Years"
            ),
            "{}",
            last_error(ctx)
        );
        ok(hegel_span_spec_set_sign(
            ctx,
            spec,
            HEGEL_SPAN_SIGN_UNIFORM as u32,
        ));
        ok(hegel_span_spec_validate(ctx, spec));
        let conflict = span_spec(ctx);
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            conflict,
            HEGEL_SPAN_UNIT_WEEKS as u32,
            1,
            2,
        ));
        ok(hegel_span_spec_set_unit_bounds(
            ctx,
            conflict,
            HEGEL_SPAN_UNIT_DAYS as u32,
            -5,
            -1,
        ));
        assert_eq!(hegel_span_spec_validate(ctx, conflict), HEGEL_E_INVALID_ARG);
        assert!(
            last_error(ctx).contains("needs every unit to admit one sign"),
            "{}",
            last_error(ctx)
        );
        ok(hegel_span_spec_set_sign(
            ctx,
            conflict,
            HEGEL_SPAN_SIGN_MIXED as u32,
        ));
        ok(hegel_span_spec_validate(ctx, conflict));
        ok(hegel_span_spec_set_sign(
            ctx,
            conflict,
            HEGEL_SPAN_SIGN_UNIFORM as u32,
        ));
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
            assert!(
                last_error(ctx).contains("hegel_generate_span: a span draw needs a unit enabled"),
                "{}",
                last_error(ctx)
            );
            assert_eq!(
                hegel_generate_span(ctx, tc, conflict, &mut out),
                HEGEL_E_INVALID_ARG
            );
            assert!(
                last_error(ctx).contains("hegel_generate_span: a span draw under the uniform"),
                "{}",
                last_error(ctx)
            );
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
        ok(hegel_span_spec_free(ctx, conflict));
        ok(hegel_span_spec_free(ctx, empty));
        ok(hegel_span_spec_free(ctx, spec));
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

/// The text a `hegel_format_*` call writes, freed.
unsafe fn text(
    ctx: *mut HegelContext,
    format: impl FnOnce(*mut hegel_owned_string_t) -> hegel_c::hegel_result_t,
) -> String {
    let mut out = hegel_owned_string_t {
        data: ptr::null(),
        len: 0,
    };
    ok(format(&mut out));
    assert_eq!(last_error(ctx), "");
    assert_eq!(unsafe { *out.data.add(out.len) }, 0);
    let text = String::from_utf8(
        unsafe { std::slice::from_raw_parts(out.data.cast::<u8>(), out.len) }.to_vec(),
    )
    .unwrap();
    ok(unsafe { hegel_owned_string_free(ctx, &mut out) });
    text
}

#[test]
fn values_format_as_their_canonical_text() {
    let ctx = hegel_context_new();
    unsafe {
        for (value, expected) in [
            (date(2024, 3, 10), "2024-03-10"),
            (date(10_000, 1, 1), "+010000-01-01"),
            (date(-1, 12, 31), "-000001-12-31"),
            (date(2024, 13, 99), "2024-13-99"),
        ] {
            assert_eq!(
                text(ctx, |out| hegel_format_date(ctx, value, out)),
                expected
            );
        }
        for (value, expected) in [
            (time(2, 30, 0, 0), "02:30:00"),
            (time(2, 30, 0, 500_000_000), "02:30:00.5"),
            (time(23, 59, 59, 999_999_999), "23:59:59.999999999"),
        ] {
            assert_eq!(
                text(ctx, |out| hegel_format_time(ctx, value, out)),
                expected
            );
        }
        let datetime = hegel_datetime_t {
            date: date(2024, 3, 10),
            time: time(2, 30, 0, 120_000_000),
        };
        assert_eq!(
            text(ctx, |out| hegel_format_datetime(ctx, datetime, out)),
            "2024-03-10T02:30:00.12"
        );
        for (value, expected) in [
            (at(946_684_800), "2000-01-01T00:00:00Z"),
            (
                hegel_timestamp_t {
                    second: -2,
                    nanosecond: 500_000_000,
                },
                "1969-12-31T23:59:58.5Z",
            ),
            (at(i64::MIN), "-292277022657-01-27T08:29:52Z"),
        ] {
            assert_eq!(
                text(ctx, |out| hegel_format_timestamp(ctx, value, out)),
                expected
            );
        }
        for (seconds, known, expected) in [
            (19_800, true, "+05:30"),
            (-2_670, true, "-00:44:30"),
            (0, true, "+00:00"),
            (0, false, "-00:00"),
        ] {
            let value = hegel_offset_t { seconds, known };
            assert_eq!(
                text(ctx, |out| hegel_format_offset(ctx, value, out)),
                expected
            );
        }
        for (value, expected) in [
            (duration(5_405, 500_000_000), "PT1H30M5.5S"),
            (duration(0, -500_000_000), "-PT0.5S"),
            (duration(0, 0), "PT0S"),
            (duration(86_400, 0), "PT24H"),
        ] {
            assert_eq!(
                text(ctx, |out| hegel_format_duration(ctx, value, out)),
                expected
            );
        }
        let spans = [
            (span(), "0s"),
            (
                hegel_span_t {
                    years: 1,
                    months: 2,
                    days: 3,
                    ..span()
                },
                "1y 2mo 3d",
            ),
            (
                hegel_span_t {
                    days: -3,
                    hours: -4,
                    ..span()
                },
                "-3d 4h",
            ),
            (
                hegel_span_t {
                    days: -3,
                    hours: 4,
                    ..span()
                },
                "-3d +4h",
            ),
            (
                hegel_span_t {
                    weeks: 1,
                    minutes: 2,
                    seconds: 3,
                    milliseconds: 4,
                    microseconds: 5,
                    nanoseconds: 6,
                    ..span()
                },
                "1w 2m 3s 4ms 5µs 6ns",
            ),
        ];
        for (value, expected) in &spans {
            assert_eq!(
                text(ctx, |out| hegel_format_span(ctx, value, out)),
                *expected
            );
        }

        let null_out: [Call<'_>; 7] = [
            (
                "hegel_format_date",
                Box::new(|| hegel_format_date(ctx, date(2000, 1, 1), ptr::null_mut())),
            ),
            (
                "hegel_format_time",
                Box::new(|| hegel_format_time(ctx, time(0, 0, 0, 0), ptr::null_mut())),
            ),
            (
                "hegel_format_datetime",
                Box::new(|| hegel_format_datetime(ctx, datetime, ptr::null_mut())),
            ),
            (
                "hegel_format_timestamp",
                Box::new(|| hegel_format_timestamp(ctx, at(0), ptr::null_mut())),
            ),
            (
                "hegel_format_offset",
                Box::new(|| {
                    let value = hegel_offset_t {
                        seconds: 0,
                        known: true,
                    };
                    hegel_format_offset(ctx, value, ptr::null_mut())
                }),
            ),
            (
                "hegel_format_duration",
                Box::new(|| hegel_format_duration(ctx, duration(0, 0), ptr::null_mut())),
            ),
            (
                "hegel_format_span",
                Box::new(|| hegel_format_span(ctx, &span(), ptr::null_mut())),
            ),
        ];
        for (name, call) in &null_out {
            assert_eq!(call(), HEGEL_E_INVALID_ARG, "{name}");
            assert_eq!(last_error(ctx), format!("{name}: out parameter is null"));
        }
        let mut out = hegel_owned_string_t {
            data: ptr::null(),
            len: 0,
        };
        assert_eq!(
            hegel_format_span(ctx, ptr::null(), &mut out),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(last_error(ctx), "hegel_format_span: value is null");
        assert!(out.data.is_null());
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

#[test]
fn drawn_values_format_as_jiff_parses_them() {
    let ctx = hegel_context_new();
    unsafe {
        let min = hegel_timestamp_t {
            second: -3,
            nanosecond: 250_000_000,
        };
        let max = hegel_timestamp_t {
            second: -1,
            nanosecond: 750_000_000,
        };
        let drawn = each_case(ctx, 100, |tc| {
            let mut ts = at(0);
            ok(hegel_generate_timestamp(
                ctx,
                tc,
                min,
                max,
                NANOSECONDS,
                &mut ts,
            ));
            let mut d = duration(0, 0);
            ok(hegel_generate_duration(
                ctx,
                tc,
                duration(-3, -500_000_000),
                duration(3, 500_000_000),
                NANOSECONDS,
                &mut d,
            ));
            (ts, d)
        });
        let mut fractional = false;
        for (ts, d) in drawn {
            assert!(ts.nanosecond < 1_000_000_000);
            assert!((-3..=-1).contains(&ts.second), "{}", ts.second);
            let nanos = i128::from(ts.second) * 1_000_000_000 + i128::from(ts.nanosecond);
            assert!((-2_750_000_000..=-250_000_000).contains(&nanos), "{nanos}");
            fractional |= ts.nanosecond != 0;
            let shown = text(ctx, |out| hegel_format_timestamp(ctx, ts, out));
            let theirs: jiff::Timestamp = shown.parse().unwrap();
            assert_eq!(theirs.as_nanosecond(), nanos, "{shown}");
            assert_eq!(theirs.to_string(), shown);
            let shown = text(ctx, |out| hegel_format_duration(ctx, d, out));
            // jiff parses a negative duration under a second as positive,
            // so the sign is checked apart from the magnitude.
            let theirs: jiff::SignedDuration = shown.parse().unwrap();
            let expected = jiff::SignedDuration::new(d.seconds, d.nanoseconds);
            assert_eq!(theirs.abs(), expected.abs(), "{shown}");
            assert_eq!(shown.starts_with('-'), expected.is_negative(), "{shown}");
        }
        assert!(fractional);
    }
    unsafe { ok(hegel_context_free(ctx)) };
}

/// Check that a `hegel_validate_*` call and the draw it stands for agree
/// on one set of arguments. With `rejected` unset both accept them; with
/// it set both report `HEGEL_E_INVALID_ARG` with a message holding that
/// text, the same message but for the C function's name where the draw's
/// carries it.
fn check_agreement(
    ctx: *mut HegelContext,
    what: &str,
    rejected: Option<&str>,
    validate: impl FnOnce() -> hegel_c::hegel_result_t,
    draw: impl FnOnce() -> hegel_c::hegel_result_t,
) {
    let validated = (validate(), last_error(ctx));
    let drawn = (draw(), last_error(ctx));
    match rejected {
        None => {
            assert_eq!(validated, (HEGEL_OK, String::new()), "{what}");
            assert_eq!(drawn, (HEGEL_OK, String::new()), "{what}");
        }
        Some(text) => {
            assert_eq!(validated.0, HEGEL_E_INVALID_ARG, "{what}");
            assert_eq!(drawn.0, HEGEL_E_INVALID_ARG, "{what}");
            assert!(validated.1.contains(text), "{what}: {}", validated.1);
            assert_eq!(
                validated.1,
                drawn.1.replace("hegel_generate_", "hegel_validate_"),
                "{what}"
            );
        }
    }
}

#[test]
fn validate_functions_accept_and_reject_what_their_draws_do() {
    let ctx = hegel_context_new();
    unsafe {
        let mondays = date_options(ctx);
        ok(hegel_date_options_set_weekdays(ctx, mondays, 0b000_0001));
        let sundays = date_options(ctx);
        ok(hegel_date_options_set_weekdays(ctx, sundays, 0b100_0000));
        let none: *const HegelDateOptions = ptr::null();
        let monday = date(2024, 1, 1);
        let dates = [
            ("a year", date(2000, 1, 1), date(2000, 12, 31), none, None),
            (
                "a masked year",
                date(2000, 1, 1),
                date(2000, 12, 31),
                mondays.cast_const(),
                None,
            ),
            (
                "one allowed day",
                monday,
                monday,
                mondays.cast_const(),
                None,
            ),
            (
                "a bad min date",
                date(2023, 2, 30),
                date(2024, 1, 1),
                none,
                Some("min_value is not a valid date"),
            ),
            (
                "a bad max date",
                date(2023, 1, 1),
                date(1_000_000, 1, 1),
                none,
                Some("max_value is not a valid date"),
            ),
            (
                "inverted dates",
                date(2024, 1, 2),
                date(2024, 1, 1),
                none,
                Some("generate_date requires min_value <= max_value"),
            ),
            (
                "no allowed day",
                monday,
                monday,
                sundays.cast_const(),
                Some("generate_date: no date in"),
            ),
        ];
        let midnight = time(0, 0, 0, 0);
        let last = time(23, 59, 59, 999_999_999);
        let times = [
            ("a day", midnight, last, NANOSECONDS, None),
            (
                "one time",
                midnight,
                midnight,
                HEGEL_PRECISION_HOURS as u32,
                None,
            ),
            (
                "a bad min time",
                time(24, 0, 0, 0),
                last,
                SECONDS,
                Some("min_value is not a valid time"),
            ),
            (
                "a bad max time",
                midnight,
                time(0, 0, 0, 1_000_000_000),
                SECONDS,
                Some("max_value is not a valid time"),
            ),
            (
                "inverted times",
                time(1, 0, 0, 0),
                midnight,
                SECONDS,
                Some("generate_time requires min_value <= max_value"),
            ),
            (
                "a time precision",
                midnight,
                last,
                9,
                Some("hegel_validate_time: unknown precision 9"),
            ),
            (
                "no time",
                time(0, 0, 0, 1),
                time(0, 0, 0, 999),
                SECONDS,
                Some("generate_time: no value at precision"),
            ),
        ];
        let datetime = |date: hegel_date_t, time: hegel_time_t| hegel_datetime_t { date, time };
        let start = datetime(date(2000, 1, 1), midnight);
        let end = datetime(date(2000, 12, 31), last);
        let datetimes = [
            ("a year", start, end, NANOSECONDS, None),
            (
                "one datetime",
                start,
                start,
                HEGEL_PRECISION_HOURS as u32,
                None,
            ),
            (
                "a bad min date",
                datetime(date(2000, 13, 1), midnight),
                end,
                SECONDS,
                Some("min_value.date is not a valid date"),
            ),
            (
                "a bad max date",
                start,
                datetime(date(2000, 4, 31), midnight),
                SECONDS,
                Some("max_value.date is not a valid date"),
            ),
            (
                "a bad min time",
                datetime(date(2000, 1, 1), time(0, 60, 0, 0)),
                end,
                SECONDS,
                Some("min_value.time is not a valid time"),
            ),
            (
                "a bad max time",
                start,
                datetime(date(2000, 12, 31), time(0, 0, 60, 0)),
                SECONDS,
                Some("max_value.time is not a valid time"),
            ),
            (
                "inverted datetimes",
                end,
                start,
                SECONDS,
                Some("generate_datetime requires min_value <= max_value"),
            ),
            (
                "a datetime precision",
                start,
                end,
                6,
                Some("hegel_validate_datetime: unknown precision 6"),
            ),
            (
                "no datetime",
                datetime(date(2000, 1, 1), time(0, 0, 0, 1)),
                datetime(date(2000, 1, 1), time(0, 0, 0, 999)),
                SECONDS,
                Some("generate_datetime: no value at precision"),
            ),
        ];
        let instant = |second: i64, nanosecond: u32| hegel_timestamp_t { second, nanosecond };
        let timestamps = [
            (
                "a century",
                at(-1_000_000_000),
                at(2_000_000_000),
                NANOSECONDS,
                None,
            ),
            (
                "one instant",
                at(3_600),
                at(3_600),
                HEGEL_PRECISION_HOURS as u32,
                None,
            ),
            (
                "a min second out of range",
                at(i64::MIN),
                at(0),
                SECONDS,
                Some("min_value.second must be in"),
            ),
            (
                "a max second out of range",
                at(0),
                at(i64::MAX),
                SECONDS,
                Some("max_value.second must be in"),
            ),
            (
                "a bad min nanosecond",
                instant(0, 1_000_000_000),
                at(1),
                SECONDS,
                Some("min_value.nanosecond must be at most 999999999"),
            ),
            (
                "a bad max nanosecond",
                at(0),
                instant(1, 1_000_000_000),
                SECONDS,
                Some("max_value.nanosecond must be at most 999999999"),
            ),
            (
                "inverted instants",
                at(1),
                at(0),
                SECONDS,
                Some("generate_timestamp requires min_value <= max_value"),
            ),
            (
                "a timestamp precision",
                at(0),
                at(1),
                7,
                Some("hegel_validate_timestamp: unknown precision 7"),
            ),
            (
                "no instant",
                instant(0, 1),
                instant(0, 999),
                SECONDS,
                Some("generate_timestamp: no value at precision"),
            ),
        ];
        let hours = HEGEL_OFFSET_GRANULARITY_HOURS as u32;
        let seconds = HEGEL_OFFSET_GRANULARITY_SECONDS as u32;
        let offsets = [
            ("every offset", -93_599, 93_599, seconds, true, None),
            ("whole hours", -3_600, 3_600, hours, false, None),
            (
                "a min offset out of range",
                -93_600,
                0,
                seconds,
                false,
                Some("min_seconds must be in"),
            ),
            (
                "a max offset out of range",
                0,
                93_600,
                seconds,
                true,
                Some("max_seconds must be in"),
            ),
            (
                "inverted offsets",
                1,
                0,
                seconds,
                false,
                Some("generate_offset requires min_seconds <= max_seconds"),
            ),
            (
                "a granularity",
                0,
                1,
                4,
                false,
                Some("hegel_validate_offset: unknown granularity 4"),
            ),
            (
                "no offset",
                1,
                3_599,
                hours,
                true,
                Some("generate_offset: no offset at granularity"),
            ),
        ];
        let durations = [
            (
                "a day either way",
                duration(-86_400, 0),
                duration(86_400, 0),
                NANOSECONDS,
                None,
            ),
            (
                "one duration",
                duration(0, 0),
                duration(0, 0),
                HEGEL_PRECISION_HOURS as u32,
                None,
            ),
            (
                "a bad min duration",
                duration(1, -1),
                duration(2, 0),
                SECONDS,
                Some("min_value is not a valid duration"),
            ),
            (
                "a bad max duration",
                duration(0, 0),
                duration(0, 1_000_000_000),
                SECONDS,
                Some("max_value is not a valid duration"),
            ),
            (
                "inverted durations",
                duration(1, 0),
                duration(0, 0),
                SECONDS,
                Some("generate_duration requires min_value <= max_value"),
            ),
            (
                "a duration precision",
                duration(0, 0),
                duration(1, 0),
                8,
                Some("hegel_validate_duration: unknown precision 8"),
            ),
            (
                "no duration",
                duration(0, 1),
                duration(0, 999),
                SECONDS,
                Some("generate_duration: no value at precision"),
            ),
        ];

        let cases = each_case(ctx, 20, |tc| {
            for (what, min, max, options, rejected) in dates {
                let mut out = date(0, 0, 0);
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_date(ctx, min, max, options),
                    || hegel_generate_date(ctx, tc, min, max, options, &mut out),
                );
            }
            for (what, min, max, precision, rejected) in times {
                let mut out = midnight;
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_time(ctx, min, max, precision),
                    || hegel_generate_time(ctx, tc, min, max, precision, &mut out),
                );
            }
            for (what, min, max, precision, rejected) in datetimes {
                let mut out = start;
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_datetime(ctx, min, max, precision),
                    || hegel_generate_datetime(ctx, tc, min, max, precision, &mut out),
                );
            }
            for (what, min, max, precision, rejected) in timestamps {
                let mut out = at(0);
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_timestamp(ctx, min, max, precision),
                    || hegel_generate_timestamp(ctx, tc, min, max, precision, &mut out),
                );
            }
            for (what, min, max, granularity, unknown, rejected) in offsets {
                let mut out = hegel_offset_t {
                    seconds: 0,
                    known: false,
                };
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_offset(ctx, min, max, granularity, unknown),
                    || hegel_generate_offset(ctx, tc, min, max, granularity, unknown, &mut out),
                );
            }
            for (what, min, max, precision, rejected) in durations {
                let mut out = duration(0, 0);
                check_agreement(
                    ctx,
                    what,
                    rejected,
                    || hegel_validate_duration(ctx, min, max, precision),
                    || hegel_generate_duration(ctx, tc, min, max, precision, &mut out),
                );
            }
        });
        assert_eq!(cases.len(), 20);

        ok(hegel_date_options_free(ctx, mondays));
        ok(hegel_date_options_free(ctx, sundays));
        ok(hegel_context_free(ctx));
    }
}
