//! In-process exercise of the `hegel_zone_source_*` functions: argument
//! checking, each input shape through the C structs, the loader and
//! oracle callbacks and their errors, the `_in` queries, and a time zone
//! spec drawing from a source. The default source is exercised in its own
//! binary, `c_abi_default_zone_source`, since it is process-wide.

mod common;

use common::{last_error, make_settings_no_db, next_case, ok, start};
use hegel_c::hegel_result_t::*;
use hegel_c::hegel_zone_after_t::*;
use hegel_c::hegel_zone_rule_day_t::*;
use hegel_c::{
    HEGEL_TIMEZONE_TAG_LEGACY, HEGEL_TIMEZONE_TAG_LINK, HegelContext, HegelZoneSource,
    hegel_context_free, hegel_context_new, hegel_context_set_error, hegel_date_t, hegel_datetime_t,
    hegel_generate_timezone, hegel_owned_string_free, hegel_owned_string_t, hegel_result_t,
    hegel_settings_free, hegel_string_t, hegel_test_case_free, hegel_time_t, hegel_timestamp_t,
    hegel_timezone_info_in, hegel_timezone_info_t, hegel_timezone_kind_t,
    hegel_timezone_offset_at_in, hegel_timezone_offset_t, hegel_timezone_resolution_t,
    hegel_timezone_resolve_in, hegel_timezone_spec_free, hegel_timezone_spec_new,
    hegel_timezone_spec_set_source, hegel_zone_data_t, hegel_zone_oracle_t, hegel_zone_rule_date_t,
    hegel_zone_source_add_link, hegel_zone_source_add_oracle, hegel_zone_source_add_tzif,
    hegel_zone_source_add_zone, hegel_zone_source_bundled, hegel_zone_source_declare,
    hegel_zone_source_free, hegel_zone_source_new, hegel_zone_source_set_loader,
    hegel_zone_source_set_version, hegel_zone_source_version, hegel_zone_transition_t,
    hegel_zone_type_t,
};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

fn s(text: &str) -> hegel_string_t {
    hegel_string_t {
        data: text.as_ptr().cast(),
        len: text.len(),
    }
}

fn null_string() -> hegel_string_t {
    hegel_string_t {
        data: ptr::null(),
        len: 1,
    }
}

fn string(p: hegel_string_t) -> &'static str {
    let bytes = unsafe { std::slice::from_raw_parts(p.data.cast::<u8>(), p.len + 1) };
    assert_eq!(bytes[p.len], 0);
    std::str::from_utf8(&bytes[..p.len]).unwrap()
}

fn tzif(name: &str) -> &'static [u8] {
    jiff_tzdb::get(name).unwrap().1
}

fn at(second: i64) -> hegel_timestamp_t {
    hegel_timestamp_t {
        second,
        nanosecond: 0,
    }
}

fn year_start(year: i16) -> i64 {
    jiff::civil::date(year, 1, 1)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
}

unsafe fn new_source(ctx: *mut HegelContext) -> *mut HegelZoneSource {
    let mut source = ptr::null_mut();
    ok(unsafe { hegel_zone_source_new(ctx, &mut source) });
    source
}

unsafe fn add_tzif(ctx: *mut HegelContext, source: *mut HegelZoneSource, name: &str, data: &str) {
    let bytes = tzif(data);
    ok(unsafe { hegel_zone_source_add_tzif(ctx, source, s(name), bytes.as_ptr(), bytes.len()) });
}

unsafe fn offset_in(
    ctx: *mut HegelContext,
    source: *const HegelZoneSource,
    name: &str,
    second: i64,
) -> hegel_timezone_offset_t {
    let mut out = hegel_timezone_offset_t {
        offset_seconds: 0,
        offset_known: false,
        is_dst: false,
        abbreviation: hegel_string_t::default(),
    };
    ok(unsafe { hegel_timezone_offset_at_in(ctx, source, s(name), at(second), &mut out) });
    out
}

unsafe fn info_in(
    ctx: *mut HegelContext,
    source: *const HegelZoneSource,
    name: &str,
) -> hegel_timezone_info_t {
    let mut out = hegel_timezone_info_t {
        known: false,
        primary_name: hegel_string_t::default(),
        is_link: false,
        rank: 0,
        tags: 0,
    };
    ok(unsafe { hegel_timezone_info_in(ctx, source, s(name), &mut out) });
    out
}

const JULY_2024: i64 = 1_719_835_200;

#[test]
fn sources_are_made_and_freed() {
    let ctx = hegel_context_new();
    unsafe {
        assert_eq!(
            hegel_zone_source_new(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_zone_source_new: out parameter is null"));
        assert_eq!(
            hegel_zone_source_bundled(ctx, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        let mut bundled = ptr::null_mut();
        ok(hegel_zone_source_bundled(ctx, &mut bundled));
        let mut version = hegel_string_t::default();
        ok(hegel_zone_source_version(ctx, bundled, &mut version));
        let mut bundled_version = hegel_string_t::default();
        ok(hegel_c::hegel_tzdb_version(ctx, &mut bundled_version));
        assert_eq!(string(version), string(bundled_version));
        assert_eq!(
            hegel_zone_source_add_tzif(ctx, bundled, s("Test/Zone"), ptr::null(), 0),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("bundled zone source cannot be changed"));
        ok(hegel_zone_source_free(ctx, bundled));
        ok(hegel_zone_source_free(ctx, ptr::null_mut()));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn tzif_data_is_read_through_the_c_abi() {
    let ctx = hegel_context_new();
    unsafe {
        let source = new_source(ctx);
        add_tzif(ctx, source, "Europe/Atlantis", "Europe/London");
        let summer = offset_in(ctx, source, "Europe/Atlantis", JULY_2024);
        assert_eq!(summer.offset_seconds, 3600);
        assert!(summer.is_dst);
        assert_eq!(string(summer.abbreviation), "BST");
        let info = info_in(ctx, source, "Europe/Atlantis");
        assert!(info.known);
        assert_eq!(string(info.primary_name), "Europe/Atlantis");
        assert_eq!(info.tags & HEGEL_TIMEZONE_TAG_LEGACY, 0);
        assert!(info.rank > info_in(ctx, ptr::null(), "Europe/London").rank);
        assert!(!info_in(ctx, source, "Europe/London").known);
        let unknown = offset_in(ctx, source, "Etc/Unknown", JULY_2024);
        assert_eq!(string(unknown.abbreviation), "-00");
        let mut resolution = hegel_timezone_resolution_t {
            kind: hegel_timezone_kind_t::HEGEL_TIMEZONE_KIND_UNIQUE,
            offset_before: 0,
            offset_after: 0,
            offset_seconds: 0,
            offset_known: false,
            timestamp: at(0),
        };
        let civil = hegel_datetime_t {
            date: hegel_date_t {
                year: 2024,
                month: 3,
                day: 31,
            },
            time: hegel_time_t {
                hour: 1,
                minute: 30,
                second: 0,
                nanosecond: 0,
            },
        };
        ok(hegel_timezone_resolve_in(
            ctx,
            source,
            s("Europe/Atlantis"),
            civil,
            &mut resolution,
        ));
        assert_eq!(
            resolution.kind,
            hegel_timezone_kind_t::HEGEL_TIMEZONE_KIND_GAP
        );
        assert_eq!(
            hegel_timezone_resolve_in(ctx, source, s("Europe/London"), civil, &mut resolution),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("unknown time zone \"Europe/London\""));
        assert_eq!(
            hegel_zone_source_add_tzif(ctx, ptr::null_mut(), s("A/B"), ptr::null(), 0),
            HEGEL_E_INVALID_HANDLE
        );
        assert!(last_error(ctx).contains("source pointer is null"));
        assert_eq!(
            hegel_zone_source_add_tzif(ctx, source, null_string(), ptr::null(), 0),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_zone_source_add_tzif: name is null"));
        assert_eq!(
            hegel_zone_source_add_tzif(ctx, source, s("A/B"), ptr::null(), 3),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("bytes is null"));
        let junk = b"TZif";
        assert_eq!(
            hegel_zone_source_add_tzif(ctx, source, s("A/B"), junk.as_ptr(), junk.len()),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx).contains("hegel_zone_source_add_tzif: zone A/B: malformed TZif data")
        );
        ok(hegel_zone_source_free(ctx, source));
        ok(hegel_context_free(ctx));
    }
}

fn zone_type(utoff: i32, is_dst: bool, abbreviation: &'static str) -> hegel_zone_type_t {
    hegel_zone_type_t {
        utoff,
        is_dst,
        abbreviation: s(abbreviation),
    }
}

fn month_rule(month: u8, time: i32) -> hegel_zone_rule_date_t {
    hegel_zone_rule_date_t {
        kind: HEGEL_ZONE_RULE_DAY_MONTH_WEEK_DAY as u32,
        day: 0,
        month,
        week: 5,
        weekday: 0,
        time,
    }
}

fn london_data(transitions: &[hegel_zone_transition_t]) -> hegel_zone_data_t {
    hegel_zone_data_t {
        initial: zone_type(0, false, "GMT"),
        transitions: transitions.as_ptr(),
        transition_count: transitions.len(),
        after: HEGEL_ZONE_AFTER_RULE as u32,
        standard: zone_type(0, false, "GMT"),
        has_dst: true,
        dst: zone_type(3600, true, "BST"),
        dst_start: month_rule(3, 3600),
        dst_end: month_rule(10, 7200),
        last_year: 0,
    }
}

#[test]
fn structured_data_is_read_through_the_c_abi() {
    let ctx = hegel_context_new();
    unsafe {
        let source = new_source(ctx);
        let data = london_data(&[]);
        ok(hegel_zone_source_add_zone(
            ctx,
            source,
            s("Test/Ruled"),
            &data,
        ));
        assert_eq!(
            offset_in(ctx, source, "Test/Ruled", JULY_2024).offset_seconds,
            3600
        );
        let julian = hegel_zone_data_t {
            dst_start: hegel_zone_rule_date_t {
                kind: HEGEL_ZONE_RULE_DAY_JULIAN as u32,
                day: 100,
                ..month_rule(0, 0)
            },
            dst_end: hegel_zone_rule_date_t {
                kind: HEGEL_ZONE_RULE_DAY_ZERO_BASED as u32,
                day: 250,
                ..month_rule(0, 0)
            },
            ..data
        };
        ok(hegel_zone_source_add_zone(
            ctx,
            source,
            s("Test/Julian"),
            &julian,
        ));
        assert_eq!(
            offset_in(ctx, source, "Test/Julian", JULY_2024).offset_seconds,
            3600
        );
        let fixed = hegel_zone_data_t {
            has_dst: false,
            ..data
        };
        ok(hegel_zone_source_add_zone(
            ctx,
            source,
            s("Test/Fixed"),
            &fixed,
        ));
        assert_eq!(
            offset_in(ctx, source, "Test/Fixed", JULY_2024).offset_seconds,
            0
        );
        let transitions: Vec<hegel_zone_transition_t> = (2019..=2020)
            .flat_map(|y| {
                let start = year_start(y);
                [
                    hegel_zone_transition_t {
                        instant: start + 90 * 86_400,
                        local_type: zone_type(3600, true, "BST"),
                    },
                    hegel_zone_transition_t {
                        instant: start + 300 * 86_400,
                        local_type: zone_type(0, false, "GMT"),
                    },
                ]
            })
            .collect();
        let infer = hegel_zone_data_t {
            after: HEGEL_ZONE_AFTER_INFER as u32,
            last_year: 2020,
            ..london_data(&transitions)
        };
        ok(hegel_zone_source_add_zone(
            ctx,
            source,
            s("Test/Inferred"),
            &infer,
        ));
        let later = year_start(2090) + 200 * 86_400;
        assert_eq!(
            offset_in(ctx, source, "Test/Inferred", later).offset_seconds,
            3600
        );
        let hold = hegel_zone_data_t {
            after: HEGEL_ZONE_AFTER_HOLD as u32,
            ..infer
        };
        ok(hegel_zone_source_add_zone(
            ctx,
            source,
            s("Test/Held"),
            &hold,
        ));
        assert_eq!(offset_in(ctx, source, "Test/Held", later).offset_seconds, 0);

        let rejects = |data: *const hegel_zone_data_t, message: &str| {
            assert_eq!(
                hegel_zone_source_add_zone(ctx, source, s("Test/Bad"), data),
                HEGEL_E_INVALID_ARG
            );
            let error = last_error(ctx);
            assert!(error.starts_with("hegel_zone_source_add_zone: "), "{error}");
            assert!(error.contains(message), "{error}");
        };
        rejects(ptr::null(), "data is null");
        let null_transitions = hegel_zone_data_t {
            transitions: ptr::null(),
            transition_count: 2,
            ..data
        };
        rejects(&null_transitions, "transitions is null");
        rejects(
            &hegel_zone_data_t { after: 7, ..data },
            "after 7 is not a hegel_zone_after_t",
        );
        let bad_kind = hegel_zone_data_t {
            dst_end: hegel_zone_rule_date_t {
                kind: 9,
                ..month_rule(10, 0)
            },
            ..data
        };
        rejects(&bad_kind, "dst_end.kind 9 is not a hegel_zone_rule_day_t");
        let bad_type = hegel_zone_type_t {
            abbreviation: null_string(),
            ..zone_type(0, false, "")
        };
        rejects(
            &hegel_zone_data_t {
                initial: bad_type,
                ..data
            },
            "initial.abbreviation is null",
        );
        rejects(
            &hegel_zone_data_t {
                standard: bad_type,
                ..data
            },
            "standard.abbreviation is null",
        );
        rejects(
            &hegel_zone_data_t {
                dst: bad_type,
                ..data
            },
            "dst.abbreviation is null",
        );
        let bad_transition = [hegel_zone_transition_t {
            instant: 0,
            local_type: bad_type,
        }];
        rejects(
            &london_data(&bad_transition),
            "transition.abbreviation is null",
        );
        rejects(
            &hegel_zone_data_t {
                dst_start: month_rule(13, 0),
                ..data
            },
            "out of range",
        );
        ok(hegel_zone_source_free(ctx, source));
        ok(hegel_context_free(ctx));
    }
}

const CHANGE: i64 = 1_000_000_000;

unsafe extern "C" fn step_offset(
    user_data: *mut c_void,
    ctx: *mut HegelContext,
    instant: i64,
    out_type: *mut hegel_zone_type_t,
) -> hegel_result_t {
    let mode = unsafe { *user_data.cast::<u32>() };
    match mode {
        1 => {
            ok(unsafe { hegel_context_set_error(ctx, s("the oracle is closed")) });
            HEGEL_E_INVALID_ARG
        }
        2 => HEGEL_E_INVALID_ARG,
        3 => {
            unsafe { *out_type = zone_type(0, false, "") };
            unsafe { (*out_type).abbreviation = null_string() };
            HEGEL_OK
        }
        _ => {
            let kind = if instant < CHANGE {
                zone_type(0, false, "AAA")
            } else {
                zone_type(3600, false, "BBB")
            };
            unsafe { *out_type = kind };
            HEGEL_OK
        }
    }
}

unsafe extern "C" fn step_next(
    user_data: *mut c_void,
    ctx: *mut HegelContext,
    after: i64,
    out_found: *mut bool,
    out_instant: *mut i64,
) -> hegel_result_t {
    if unsafe { *user_data.cast::<u32>() } == 4 {
        ok(unsafe { hegel_context_set_error(ctx, s("no more")) });
        return HEGEL_E_INVALID_ARG;
    }
    unsafe {
        *out_found = after < CHANGE;
        *out_instant = CHANGE;
    }
    HEGEL_OK
}

#[test]
fn oracles_are_probed_through_the_c_abi() {
    let ctx = hegel_context_new();
    unsafe {
        let source = new_source(ctx);
        let mut mode: u32 = 0;
        let oracle = |mode: &mut u32, with_next: bool| hegel_zone_oracle_t {
            offset_at: Some(step_offset),
            next_transition: if with_next { Some(step_next) } else { None },
            user_data: (mode as *mut u32).cast(),
            first_year: 2000,
            last_year: 2002,
            after: HEGEL_ZONE_AFTER_HOLD as u32,
        };
        for with_next in [false, true] {
            let o = oracle(&mut mode, with_next);
            ok(hegel_zone_source_add_oracle(
                ctx,
                source,
                s("Test/Step"),
                &o,
            ));
            let before = offset_in(ctx, source, "Test/Step", CHANGE - 1);
            let after = offset_in(ctx, source, "Test/Step", CHANGE);
            assert_eq!((before.offset_seconds, after.offset_seconds), (0, 3600));
            assert_eq!(string(after.abbreviation), "BBB");
        }
        let o = hegel_zone_oracle_t {
            after: HEGEL_ZONE_AFTER_INFER as u32,
            ..oracle(&mut mode, true)
        };
        ok(hegel_zone_source_add_oracle(
            ctx,
            source,
            s("Test/Inferred"),
            &o,
        ));
        let rejects = |oracle: *const hegel_zone_oracle_t, message: &str| {
            assert_eq!(
                hegel_zone_source_add_oracle(ctx, source, s("Test/Bad"), oracle),
                HEGEL_E_INVALID_ARG
            );
            let error = last_error(ctx);
            assert!(
                error.starts_with("hegel_zone_source_add_oracle: "),
                "{error}"
            );
            assert!(error.contains(message), "{error}");
        };
        rejects(ptr::null(), "oracle is null");
        let mut fresh = 0;
        let template = oracle(&mut fresh, true);
        rejects(
            &hegel_zone_oracle_t {
                offset_at: None,
                ..template
            },
            "oracle.offset_at is null",
        );
        rejects(
            &hegel_zone_oracle_t {
                after: HEGEL_ZONE_AFTER_RULE as u32,
                ..template
            },
            "neither HEGEL_ZONE_AFTER_HOLD nor HEGEL_ZONE_AFTER_INFER",
        );
        rejects(
            &hegel_zone_oracle_t {
                last_year: 1999,
                ..template
            },
            "probe range",
        );
        for (failing, message) in [
            (
                1,
                "the oracle's offset_at(946684800) failed: the oracle is closed",
            ),
            (
                2,
                "the oracle's offset_at(946684800) failed with HEGEL_E_INVALID_ARG",
            ),
            (3, "the oracle's answer.abbreviation is null"),
            (4, "the oracle's next_transition(946684800) failed: no more"),
        ] {
            let mut mode = failing;
            rejects(&oracle(&mut mode, true), message);
        }
        ok(hegel_zone_source_free(ctx, source));
        ok(hegel_context_free(ctx));
    }
}

#[test]
fn links_names_and_versions_go_through_the_c_abi() {
    let ctx = hegel_context_new();
    unsafe {
        let source = new_source(ctx);
        add_tzif(ctx, source, "Asia/Kolkata", "Asia/Kolkata");
        ok(hegel_zone_source_add_link(
            ctx,
            source,
            s("Asia/Calcutta"),
            s("Asia/Kolkata"),
        ));
        let link = info_in(ctx, source, "Asia/Calcutta");
        assert!(link.is_link);
        assert_ne!(link.tags & HEGEL_TIMEZONE_TAG_LINK, 0);
        assert_eq!(string(link.primary_name), "Asia/Kolkata");
        assert_eq!(
            hegel_zone_source_add_link(ctx, source, s("A/B"), null_string()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("target"));
        assert_eq!(
            hegel_zone_source_add_link(ctx, source, s("A/B"), s("A/B")),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_zone_source_add_link: \"A/B\" cannot be a link"));
        let names = [s("Mars/Base"), s("Europe/Atlantis")];
        ok(hegel_zone_source_declare(
            ctx,
            source,
            names.as_ptr(),
            names.len(),
        ));
        let mars = info_in(ctx, source, "Mars/Base");
        assert!(mars.known);
        assert_eq!(mars.tags, 0);
        let mut offset = hegel_timezone_offset_t {
            offset_seconds: 0,
            offset_known: false,
            is_dst: false,
            abbreviation: hegel_string_t::default(),
        };
        assert_eq!(
            hegel_timezone_offset_at_in(ctx, source, s("Mars/Base"), at(0), &mut offset),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("no data for \"Mars/Base\""));
        assert_eq!(
            hegel_zone_source_declare(ctx, source, ptr::null(), 2),
            HEGEL_E_INVALID_ARG
        );
        let bad = [s("")];
        assert_eq!(
            hegel_zone_source_declare(ctx, source, bad.as_ptr(), 1),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("cannot name a zone"));
        let mut version = hegel_string_t::default();
        ok(hegel_zone_source_version(ctx, source, &mut version));
        assert_eq!(string(version), "");
        ok(hegel_zone_source_set_version(ctx, source, s("2099z")));
        ok(hegel_zone_source_version(ctx, source, &mut version));
        assert_eq!(string(version), "2099z");
        assert_eq!(
            hegel_zone_source_set_version(ctx, source, null_string()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_zone_source_set_version: version is null"));
        assert_eq!(
            hegel_zone_source_version(ctx, ptr::null(), &mut version),
            HEGEL_E_INVALID_HANDLE
        );
        assert_eq!(
            hegel_zone_source_version(ctx, source, ptr::null_mut()),
            HEGEL_E_INVALID_ARG
        );
        assert_eq!(
            hegel_context_set_error(ctx, null_string()),
            HEGEL_E_INVALID_ARG
        );
        assert!(last_error(ctx).contains("hegel_context_set_error: message is null"));
        ok(hegel_zone_source_free(ctx, source));
        ok(hegel_context_free(ctx));
    }
}

struct LoaderState {
    calls: AtomicUsize,
}

unsafe extern "C" fn loader(
    user_data: *mut c_void,
    ctx: *mut HegelContext,
    source: *mut HegelZoneSource,
    name: hegel_string_t,
) -> hegel_result_t {
    let state = unsafe { &*user_data.cast::<LoaderState>() };
    state.calls.fetch_add(1, Ordering::SeqCst);
    let name = string(name);
    match name {
        "Fail/Loudly" => {
            ok(unsafe { hegel_context_set_error(ctx, s("cannot read the file")) });
            HEGEL_E_INVALID_ARG
        }
        "Fail/Quietly" => HEGEL_E_INVALID_ARG,
        _ => match jiff_tzdb::get(name) {
            Some((_, bytes)) => unsafe {
                hegel_zone_source_add_tzif(ctx, source, s(name), bytes.as_ptr(), bytes.len())
            },
            None => HEGEL_OK,
        },
    }
}

#[test]
fn a_loader_supplies_data_through_the_c_abi() {
    let ctx = hegel_context_new();
    let state = LoaderState {
        calls: AtomicUsize::new(0),
    };
    unsafe {
        let source = new_source(ctx);
        let names = [s("Europe/London"), s("Asia/Tokyo")];
        ok(hegel_zone_source_declare(
            ctx,
            source,
            names.as_ptr(),
            names.len(),
        ));
        let user_data = (&state as *const LoaderState).cast_mut().cast();
        ok(hegel_zone_source_set_loader(
            ctx,
            source,
            Some(loader),
            user_data,
        ));
        assert_eq!(
            offset_in(ctx, source, "Europe/London", JULY_2024).offset_seconds,
            3600
        );
        assert_eq!(
            offset_in(ctx, source, "Europe/London", JULY_2024).offset_seconds,
            3600
        );
        assert_eq!(state.calls.load(Ordering::SeqCst), 1);
        let mut offset = offset_in(ctx, source, "Europe/London", 0);
        for (name, message) in [
            (
                "Fail/Loudly",
                "the zone source's loader for \"Fail/Loudly\" failed: cannot read the file",
            ),
            (
                "Fail/Quietly",
                "the zone source's loader for \"Fail/Quietly\" failed with HEGEL_E_INVALID_ARG",
            ),
        ] {
            assert_eq!(
                hegel_timezone_offset_at_in(ctx, source, s(name), at(0), &mut offset),
                HEGEL_E_INVALID_ARG
            );
            assert!(last_error(ctx).contains(message), "{}", last_error(ctx));
        }
        let mut spec = ptr::null_mut();
        ok(hegel_timezone_spec_new(ctx, &mut spec));
        ok(hegel_timezone_spec_set_source(ctx, spec, source));
        let settings = make_settings_no_db(ctx);
        let run = start(ctx, settings);
        let tc = next_case(ctx, run);
        let mut result = hegel_owned_string_t {
            data: ptr::null(),
            len: 0,
        };
        ok(hegel_generate_timezone(ctx, tc, spec, &mut result));
        let drawn = string(hegel_string_t {
            data: result.data,
            len: result.len,
        })
        .to_string();
        assert!(
            ["Europe/London", "Asia/Tokyo"].contains(&drawn.as_str()),
            "{drawn}"
        );
        ok(hegel_owned_string_free(ctx, &mut result));
        ok(hegel_test_case_free(ctx, tc));
        ok(hegel_c::hegel_run_free(ctx, run));
        ok(hegel_settings_free(ctx, settings));
        ok(hegel_timezone_spec_set_source(ctx, spec, ptr::null()));
        ok(hegel_timezone_spec_free(ctx, spec));
        ok(hegel_zone_source_set_loader(
            ctx,
            source,
            None,
            ptr::null_mut(),
        ));
        assert_eq!(
            hegel_zone_source_set_loader(ctx, ptr::null_mut(), None, ptr::null_mut()),
            HEGEL_E_INVALID_HANDLE
        );
        ok(hegel_zone_source_free(ctx, source));
        ok(hegel_context_free(ctx));
    }
}
