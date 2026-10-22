package dev.hegel.lowlevel;

/**
 * Constants from the libhegel C ABI ({@code hegel-c/include/hegel.h}): return codes, run and case
 * statuses, phase and health-check bit masks, and the sentinels the structured primitives use.
 * Kept in sync with the engine header.
 */
public final class Abi {
    private Abi() {}

    // Return codes (hegel_result_t).
    public static final int OK = 0;
    public static final int E_STOP_TEST = -1;
    public static final int E_ASSUME = -2;
    public static final int E_BACKEND = -3;
    public static final int E_INVALID_HANDLE = -4;
    public static final int E_INVALID_ARG = -5;
    public static final int E_ALREADY_COMPLETE = -6;
    public static final int E_NOT_COMPLETE = -7;
    public static final int E_INTERNAL = -8;
    public static final int E_CONCURRENT_USE = -9;
    public static final int E_RETRY = -10;

    // Aggregate run outcome (hegel_run_status_t). Value 3 (FAILED_NONDETERMINISTIC, retired in the
    // 0.44 ABI) is never reported and must not be reused: a failing nondeterministic run reports
    // plain FAILED, with a caveat on each failure.
    public static final int RUN_STATUS_PASSED = 0;
    public static final int RUN_STATUS_FAILED = 1;
    public static final int RUN_STATUS_ERROR = 2;

    // hegel_nondeterminism_strictness_t: how a run reacts on detecting a nondeterministic test.
    public static final int NONDETERMINISM_QUIET = 0;
    public static final int NONDETERMINISM_WARN = 1;
    public static final int NONDETERMINISM_ERROR = 2;

    // Phases (bitmask for hegel_settings_set_phases).
    public static final int PHASE_EXPLICIT = 1 << 0;
    public static final int PHASE_REUSE = 1 << 1;
    public static final int PHASE_GENERATE = 1 << 2;
    public static final int PHASE_TARGET = 1 << 3;
    public static final int PHASE_SHRINK = 1 << 4;
    public static final int PHASE_ALL = 31;

    // Health-check suppression bitmask (hegel_settings_set_suppress_health_check).
    public static final int HC_FILTER_TOO_MUCH = 1 << 0;
    public static final int HC_TOO_SLOW = 1 << 1;
    public static final int HC_TEST_CASES_TOO_LARGE = 1 << 2;
    public static final int HC_LARGE_INITIAL_TEST_CASE = 1 << 3;

    // hegel_backend_t. There is no automatic value: leaving the backend unset lets the engine's
    // settings profile choose (the shipped `workload` profile, selected inside Antithesis, uses
    // urandom).
    public static final int BACKEND_DEFAULT = 1;
    public static final int BACKEND_URANDOM = 2;

    // hegel_verbosity_t.
    public static final int VERBOSITY_NORMAL = 0;
    public static final int VERBOSITY_QUIET = 1;
    public static final int VERBOSITY_VERBOSE = 2;
    public static final int VERBOSITY_DEBUG = 3;

    // hegel_status_t (argument to hegel_mark_complete).
    public static final int STATUS_VALID = 0;
    public static final int STATUS_INVALID = 1;
    public static final int STATUS_OVERRUN = 2;
    public static final int STATUS_INTERESTING = 3;

    // hegel_precision_t: the unit a drawn time, datetime, zoned datetime, timestamp or duration is
    // counted in.
    public static final int PRECISION_NANOSECONDS = 0;
    public static final int PRECISION_MICROSECONDS = 1;
    public static final int PRECISION_MILLISECONDS = 2;
    public static final int PRECISION_SECONDS = 3;
    public static final int PRECISION_MINUTES = 4;
    public static final int PRECISION_HOURS = 5;

    // hegel_imaginary_t: what a zoned datetime draw does with a wall time the clocks skipped.
    public static final int IMAGINARY_ALLOW = 0;
    public static final int IMAGINARY_AVOID = 1;

    // hegel_ambiguous_t: which instant a zoned datetime draw means by a repeated wall time.
    public static final int AMBIGUOUS_BOTH = 0;
    public static final int AMBIGUOUS_EARLIER = 1;
    public static final int AMBIGUOUS_LATER = 2;

    // hegel_timezone_kind_t: how a wall time resolves in a zone.
    public static final int TIMEZONE_KIND_UNIQUE = 0;
    public static final int TIMEZONE_KIND_GAP = 1;
    public static final int TIMEZONE_KIND_FOLD = 2;

    // hegel_offset_granularity_t: the values a UTC offset draw restricts itself to.
    public static final int OFFSET_GRANULARITY_HOURS = 0;
    public static final int OFFSET_GRANULARITY_QUARTER_HOURS = 1;
    public static final int OFFSET_GRANULARITY_MINUTES = 2;
    public static final int OFFSET_GRANULARITY_SECONDS = 3;

    // hegel_span_unit_t: the units of a calendar span.
    public static final int SPAN_UNIT_YEARS = 0;
    public static final int SPAN_UNIT_MONTHS = 1;
    public static final int SPAN_UNIT_WEEKS = 2;
    public static final int SPAN_UNIT_DAYS = 3;
    public static final int SPAN_UNIT_HOURS = 4;
    public static final int SPAN_UNIT_MINUTES = 5;
    public static final int SPAN_UNIT_SECONDS = 6;
    public static final int SPAN_UNIT_MILLISECONDS = 7;
    public static final int SPAN_UNIT_MICROSECONDS = 8;
    public static final int SPAN_UNIT_NANOSECONDS = 9;

    // hegel_span_sign_t: how the signs of a span's units are drawn.
    public static final int SPAN_SIGN_UNIFORM = 0;
    public static final int SPAN_SIGN_MIXED = 1;
    public static final int SPAN_SIGN_NONNEGATIVE = 2;

    // HEGEL_TIMEZONE_TAG_*: the feature tags hegel_timezone_info reports as a bit set.
    public static final int TIMEZONE_TAG_FIXED = 1 << 0;
    public static final int TIMEZONE_TAG_WHOLE_HOUR = 1 << 1;
    public static final int TIMEZONE_TAG_FRACTIONAL = 1 << 2;
    public static final int TIMEZONE_TAG_SECONDS = 1 << 3;
    public static final int TIMEZONE_TAG_DST = 1 << 4;
    public static final int TIMEZONE_TAG_DST_OTHER = 1 << 5;
    public static final int TIMEZONE_TAG_MULTI = 1 << 6;
    public static final int TIMEZONE_TAG_BASE_CHANGE = 1 << 7;
    public static final int TIMEZONE_TAG_MIDNIGHT = 1 << 8;
    public static final int TIMEZONE_TAG_SPANS_MIDNIGHT = 1 << 9;
    public static final int TIMEZONE_TAG_WHOLE_DAY = 1 << 10;
    public static final int TIMEZONE_TAG_SAME_OFFSET = 1 << 11;
    public static final int TIMEZONE_TAG_UNDEFINED = 1 << 12;
    public static final int TIMEZONE_TAG_LINK = 1 << 13;
    public static final int TIMEZONE_TAG_LEGACY = 1 << 14;
    public static final int TIMEZONE_TAG_INVERTED_SIGN = 1 << 15;

    // Sentinel written by hegel_state_machine_next_group / hegel_state_machine_next_rule (INT64_MIN).
    public static final long STATE_MACHINE_DONE = Long.MIN_VALUE;
    public static final long UNBOUNDED = -1L; // 0xFFFFFFFFFFFFFFFF as a Java long
    public static final long NO_MAX_CODEPOINT = 0xFFFFFFFFL;
}
