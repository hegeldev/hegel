//! Mirrors of the `repr(C)` types the `hegel_*` functions traffic in.
//!
//! Every definition must match `hegel-c/src/lib.rs` exactly — same
//! discriminants, fields, and layout; the meanings are documented there.
//! `sys_tests.rs` compile-asserts the match whenever the `static-engine`
//! feature makes both sets of definitions visible. The handle structs stand
//! in for types the engine never exposes by value, so they are deliberately
//! opaque. `dead_code` is allowed because this is an ABI surface: variants
//! exist because the engine can produce them, not because the frontend
//! constructs them.

#![allow(dead_code)]
#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_void};

macro_rules! opaque_handles {
    ($($name:ident),* $(,)?) => {
        $(
            #[repr(C)]
            pub(crate) struct $name {
                _opaque: [u8; 0],
            }
        )*
    };
}

opaque_handles!(
    HegelCollection,
    HegelContext,
    HegelFailure,
    HegelGrammar,
    HegelPool,
    HegelPrinter,
    HegelPrinterOptions,
    HegelRecursion,
    HegelReflowOptions,
    HegelRun,
    HegelRunResult,
    HegelSettings,
    HegelStateMachine,
    HegelStringGenerator,
    HegelTestCase,
    HegelDateOptions,
    HegelSpanSpec,
    HegelTimezoneSpec,
    HegelZonedSpec,
);

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[must_use]
pub(crate) enum hegel_result_t {
    HEGEL_OK = 0,
    HEGEL_E_STOP_TEST = -1,
    HEGEL_E_ASSUME = -2,
    HEGEL_E_BACKEND = -3,
    HEGEL_E_INVALID_HANDLE = -4,
    HEGEL_E_INVALID_ARG = -5,
    HEGEL_E_ALREADY_COMPLETE = -6,
    HEGEL_E_NOT_COMPLETE = -7,
    HEGEL_E_INTERNAL = -8,
    HEGEL_E_CONCURRENT_USE = -9,
    HEGEL_E_RETRY = -10,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_status_t {
    HEGEL_STATUS_VALID = 0,
    HEGEL_STATUS_INVALID = 1,
    HEGEL_STATUS_OVERRUN = 2,
    HEGEL_STATUS_INTERESTING = 3,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_backend_t {
    HEGEL_BACKEND_DEFAULT = 1,
    HEGEL_BACKEND_URANDOM = 2,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum hegel_run_status_t {
    HEGEL_RUN_STATUS_PASSED = 0,
    HEGEL_RUN_STATUS_FAILED = 1,
    HEGEL_RUN_STATUS_ERROR = 2,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum hegel_run_error_kind_t {
    HEGEL_RUN_ERROR_NONE = 0,
    HEGEL_RUN_ERROR_HEALTH_CHECK = 1,
    HEGEL_RUN_ERROR_FLAKY = 2,
    HEGEL_RUN_ERROR_NONDETERMINISTIC = 3,
    HEGEL_RUN_ERROR_UNSATISFIABLE = 4,
    HEGEL_RUN_ERROR_USAGE = 5,
    HEGEL_RUN_ERROR_INTERNAL = 6,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_verbosity_t {
    HEGEL_VERBOSITY_NORMAL = 0,
    HEGEL_VERBOSITY_QUIET = 1,
    HEGEL_VERBOSITY_VERBOSE = 2,
    HEGEL_VERBOSITY_DEBUG = 3,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_nondeterminism_strictness_t {
    HEGEL_NONDETERMINISM_QUIET = 0,
    HEGEL_NONDETERMINISM_WARN = 1,
    HEGEL_NONDETERMINISM_ERROR = 2,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_phase_t {
    HEGEL_PHASE_EXPLICIT = 1 << 0,
    HEGEL_PHASE_REUSE = 1 << 1,
    HEGEL_PHASE_GENERATE = 1 << 2,
    HEGEL_PHASE_TARGET = 1 << 3,
    HEGEL_PHASE_SHRINK = 1 << 4,
    HEGEL_PHASE_ALL = 0x1F,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_health_check_t {
    HEGEL_HC_FILTER_TOO_MUCH = 1 << 0,
    HEGEL_HC_TOO_SLOW = 1 << 1,
    HEGEL_HC_TEST_CASES_TOO_LARGE = 1 << 2,
    HEGEL_HC_LARGE_INITIAL_TEST_CASE = 1 << 3,
}

pub(crate) type hegel_output_callback_t =
    Option<unsafe extern "C" fn(user_data: *mut c_void, line: *const c_char, len: usize)>;

pub(crate) type hegel_grammar_import_t = Option<
    unsafe extern "C" fn(
        user_data: *mut c_void,
        base_path: *const c_char,
        grammar_path: *const c_char,
        out_path: *mut *const c_char,
        out_text: *mut *const c_char,
        out_text_len: *mut usize,
    ) -> bool,
>;

pub(crate) const HEGEL_STATE_MACHINE_DONE: i64 = i64::MIN;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_date_t {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_time_t {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_datetime_t {
    pub date: hegel_date_t,
    pub time: hegel_time_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_timestamp_t {
    pub second: i64,
    pub nanosecond: u32,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum hegel_timezone_kind_t {
    HEGEL_TIMEZONE_KIND_UNIQUE = 0,
    HEGEL_TIMEZONE_KIND_GAP = 1,
    HEGEL_TIMEZONE_KIND_FOLD = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_timezone_resolution_t {
    pub kind: hegel_timezone_kind_t,
    pub offset_before: i32,
    pub offset_after: i32,
    pub offset_seconds: i32,
    pub offset_known: bool,
    pub timestamp: hegel_timestamp_t,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_timezone_offset_t {
    pub offset_seconds: i32,
    pub offset_known: bool,
    pub is_dst: bool,
    pub abbreviation: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_timezone_info_t {
    pub known: bool,
    pub primary_name: *const c_char,
    pub is_link: bool,
    pub rank: u16,
    pub tags: u32,
}

pub(crate) const HEGEL_TIMEZONE_TAG_FIXED: u32 = 1 << 0;
pub(crate) const HEGEL_TIMEZONE_TAG_WHOLE_HOUR: u32 = 1 << 1;
pub(crate) const HEGEL_TIMEZONE_TAG_FRACTIONAL: u32 = 1 << 2;
pub(crate) const HEGEL_TIMEZONE_TAG_SECONDS: u32 = 1 << 3;
pub(crate) const HEGEL_TIMEZONE_TAG_DST: u32 = 1 << 4;
pub(crate) const HEGEL_TIMEZONE_TAG_DST_OTHER: u32 = 1 << 5;
pub(crate) const HEGEL_TIMEZONE_TAG_MULTI: u32 = 1 << 6;
pub(crate) const HEGEL_TIMEZONE_TAG_BASE_CHANGE: u32 = 1 << 7;
pub(crate) const HEGEL_TIMEZONE_TAG_MIDNIGHT: u32 = 1 << 8;
pub(crate) const HEGEL_TIMEZONE_TAG_SPANS_MIDNIGHT: u32 = 1 << 9;
pub(crate) const HEGEL_TIMEZONE_TAG_WHOLE_DAY: u32 = 1 << 10;
pub(crate) const HEGEL_TIMEZONE_TAG_SAME_OFFSET: u32 = 1 << 11;
pub(crate) const HEGEL_TIMEZONE_TAG_UNDEFINED: u32 = 1 << 12;
pub(crate) const HEGEL_TIMEZONE_TAG_LINK: u32 = 1 << 13;
pub(crate) const HEGEL_TIMEZONE_TAG_LEGACY: u32 = 1 << 14;
pub(crate) const HEGEL_TIMEZONE_TAG_INVERTED_SIGN: u32 = 1 << 15;

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_precision_t {
    HEGEL_PRECISION_NANOSECONDS = 0,
    HEGEL_PRECISION_MICROSECONDS = 1,
    HEGEL_PRECISION_MILLISECONDS = 2,
    HEGEL_PRECISION_SECONDS = 3,
    HEGEL_PRECISION_MINUTES = 4,
    HEGEL_PRECISION_HOURS = 5,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_imaginary_t {
    HEGEL_IMAGINARY_ALLOW = 0,
    HEGEL_IMAGINARY_AVOID = 1,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_ambiguous_t {
    HEGEL_AMBIGUOUS_BOTH = 0,
    HEGEL_AMBIGUOUS_EARLIER = 1,
    HEGEL_AMBIGUOUS_LATER = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_zoned_datetime_t {
    pub civil: hegel_datetime_t,
    pub offset_seconds: i32,
    pub offset_known: bool,
    pub fold: bool,
    pub kind: hegel_timezone_kind_t,
    pub offset_before: i32,
    pub offset_after: i32,
    pub timestamp: hegel_timestamp_t,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_offset_granularity_t {
    HEGEL_OFFSET_GRANULARITY_HOURS = 0,
    HEGEL_OFFSET_GRANULARITY_QUARTER_HOURS = 1,
    HEGEL_OFFSET_GRANULARITY_MINUTES = 2,
    HEGEL_OFFSET_GRANULARITY_SECONDS = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_offset_t {
    pub seconds: i32,
    pub known: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_duration_t {
    pub seconds: i64,
    pub nanoseconds: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_span_unit_t {
    HEGEL_SPAN_UNIT_YEARS = 0,
    HEGEL_SPAN_UNIT_MONTHS = 1,
    HEGEL_SPAN_UNIT_WEEKS = 2,
    HEGEL_SPAN_UNIT_DAYS = 3,
    HEGEL_SPAN_UNIT_HOURS = 4,
    HEGEL_SPAN_UNIT_MINUTES = 5,
    HEGEL_SPAN_UNIT_SECONDS = 6,
    HEGEL_SPAN_UNIT_MILLISECONDS = 7,
    HEGEL_SPAN_UNIT_MICROSECONDS = 8,
    HEGEL_SPAN_UNIT_NANOSECONDS = 9,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) enum hegel_span_sign_t {
    HEGEL_SPAN_SIGN_UNIFORM = 0,
    HEGEL_SPAN_SIGN_MIXED = 1,
    HEGEL_SPAN_SIGN_NONNEGATIVE = 2,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct hegel_span_t {
    pub years: i64,
    pub months: i64,
    pub weeks: i64,
    pub days: i64,
    pub hours: i64,
    pub minutes: i64,
    pub seconds: i64,
    pub milliseconds: i64,
    pub microseconds: i64,
    pub nanoseconds: i64,
}

#[repr(C)]
pub(crate) struct hegel_generate_bytes_result_t {
    pub data: *mut u8,
    pub len: usize,
}

#[repr(C)]
pub(crate) struct hegel_generate_string_result_t {
    pub data: *mut c_char,
    pub len: usize,
}

#[repr(C)]
pub(crate) struct hegel_printer_value_result_t {
    pub data: *mut c_char,
    pub len: usize,
}
