/**
 * Thin, typed binding to the native `libhegel` C ABI (declared in
 * `libhegel/include/hegel.h` of this repository) via {@link koffi}.
 *
 * The {@link Libhegel} class owns the loaded library's function pointers and
 * exposes ergonomic wrappers. Every fallible call takes a `hegel_context_t*`
 * first argument and returns a `hegel_result_t` code (`HEGEL_OK` is zero;
 * negatives are errors), writing any value it produces — a handle, a string, a
 * count — through a trailing out-parameter. The wrappers map those codes to
 * thrown errors ({@link StopTestError} / {@link AssumeError} for the
 * choice-budget / rejected-draw cases, otherwise {@link LibhegelError} carrying
 * the diagnostic from `hegel_context_last_error`) and read the out-parameters
 * back into JS values.
 *
 * libhegel frees nothing for you: every handle written back by the ABI is
 * caller-owned and must be released with its matching free. That covers the
 * constructors (`hegel_context_new`, `hegel_settings_new`, `hegel_run_start`,
 * `hegel_test_case_from_blob`) and — new in this ABI — every test case from
 * `hegel_next_test_case`, the run result from `hegel_run_result`, each failure
 * from `hegel_run_result_failure`, and each collection, pool and state machine
 * from `hegel_new_collection` / `hegel_new_pool` / `hegel_new_state_machine`
 * (the runner releases them all in `finally` blocks).
 * String generators are cached within a run and freed when that run ends.
 *
 * @packageDocumentation
 */

import {
  EngineError,
  doneOrIndex,
  type Engine,
  type ContextHandle,
  type SettingsHandle,
  type RunHandle,
  type RunResultHandle,
  type TestCaseHandle,
  type FailureHandle,
  type CollectionHandle,
  type StringGeneratorHandle,
  type PoolHandle,
  type StateMachineHandle,
  type StateMachineOptions,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeTimestamp,
  type NativeOffset,
  type NativeSpan,
  type NativeZonedDatetime,
  type NativeTimeZoneInfo,
  type DateOptionsHandle,
  type TimeZoneSpecHandle,
  type ZonedSpecHandle,
  type SpanSpecHandle,
  type TextGeneratorOptions,
  type NativeFloatOptions,
  type UuidVersion,
} from "./engine.js";
export {
  Status,
  RunStatus,
  NativeVerbosity,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeTimestamp,
  type NativeOffset,
  type NativeSpan,
  type NativeZonedDatetime,
  type NativeTimeZoneInfo,
  type TextGeneratorOptions,
  type NativeFloatOptions,
  type StateMachineOptions,
  type UuidVersion,
} from "./engine.js";
import { bigIntToTwosComplementLE, twosComplementLEToBigInt } from "./bytes.js";
export { fitsInt64, bigIntToTwosComplementLE, twosComplementLEToBigInt } from "./bytes.js";

import { Buffer } from "node:buffer";
import koffi, { type TypeObject, type LibraryHandle } from "koffi";
import { StopTestError, AssumeError } from "./testCase.js";
import { wtf8ToString } from "./wtf8.js";

/** Opaque libhegel handle (koffi pointer). `null` signals a failed call. */
export type Ptr = unknown;

/** Relevant `hegel_result_t` codes. */
const RESULT_OK = 0;
const RESULT_STOP_TEST = -1;
const RESULT_ASSUME = -2;

/** An error returned by a fallible libhegel call. */
export class LibhegelError extends EngineError {
  readonly code: number;
  constructor(message: string, code: number) {
    super(message);
    this.name = "LibhegelError";
    this.code = code;
  }
}

/**
 * A `hegel_string_t` (so also a `hegel_owned_string_t`) or a
 * `hegel_generate_bytes_result_t` as koffi marshals it: a buffer pointer plus
 * its byte length. Read back from the engine it is engine-owned; passed in it
 * views a JS `Buffer`.
 */
export interface NativeBuffer {
  data: Ptr;
  len: number | bigint;
}

// koffi type objects for the ABI's by-value structs. Deliberately anonymous:
// koffi's named-type registry is global and persists across module reloads
// (e.g. between test files in one worker), so registering a name twice throws.
const dateType: TypeObject = koffi.struct({
  year: "int32_t",
  month: "uint8_t",
  day: "uint8_t",
});
const timeType: TypeObject = koffi.struct({
  hour: "uint8_t",
  minute: "uint8_t",
  second: "uint8_t",
  nanosecond: "uint32_t",
});
const datetimeType: TypeObject = koffi.struct({ date: dateType, time: timeType });
const timestampType: TypeObject = koffi.struct({ second: "int64_t", nanosecond: "uint32_t" });
const offsetType: TypeObject = koffi.struct({ seconds: "int32_t", known: "bool" });
const spanType: TypeObject = koffi.struct({
  years: "int64_t",
  months: "int64_t",
  weeks: "int64_t",
  days: "int64_t",
  hours: "int64_t",
  minutes: "int64_t",
  seconds: "int64_t",
  milliseconds: "int64_t",
  microseconds: "int64_t",
  nanoseconds: "int64_t",
});
const zonedDatetimeType: TypeObject = koffi.struct({
  civil: datetimeType,
  offset_seconds: "int32_t",
  offset_known: "bool",
  fold: "bool",
  kind: "uint32_t",
  offset_before: "int32_t",
  offset_after: "int32_t",
  timestamp: timestampType,
});
// hegel_string_t, hegel_owned_string_t and hegel_generate_bytes_result_t are
// all {pointer, len}. `data` is bound as uint8_t* rather than char* so koffi
// hands back the raw pointer: the strings may contain interior NULs, so they
// are read by `len`.
const stringType: TypeObject = koffi.struct({ data: "uint8_t*", len: "size_t" });
const timezoneInfoType: TypeObject = koffi.struct({
  known: "bool",
  primary_name: stringType,
  is_link: "bool",
  rank: "uint16_t",
  tags: "uint32_t",
});

/** A `hegel_timestamp_t` as koffi marshals it: the `int64_t` may come back as a `number`. */
interface RawTimestamp {
  second: number | bigint;
  nanosecond: number;
}

/** A `hegel_zoned_datetime_t` as koffi marshals it, with the C field names. */
interface RawZonedDatetime {
  civil: NativeDatetime;
  offset_seconds: number;
  offset_known: boolean;
  fold: boolean;
  kind: number;
  offset_before: number;
  offset_after: number;
  timestamp: RawTimestamp;
}

/** A `hegel_timezone_info_t` as koffi marshals it, with the C field names. */
interface RawTimeZoneInfo {
  known: boolean;
  primary_name: NativeBuffer;
  is_link: boolean;
  rank: number;
  tags: number;
}

/** A `hegel_span_t` as koffi marshals it: each `int64_t` may come back as a `number`. */
type RawSpan = Record<keyof NativeSpan, number | bigint>;

/**
 * The set of C functions bound from the shared library.
 *
 * Fallible calls return the `hegel_result_t` code and write their handle / value
 * through a trailing JS out-array (`[null]`, `[0]`); the infallible-for-our-use
 * accessors (constructors other than `hegel_settings_new`, frees, setters,
 * result getters) are presented here as value-returning wrappers, with
 * out-parameter marshalling and checked result codes handled by
 * {@link bindLibrary}. `hegel_settings_new` stays fallible: it resolves the
 * default settings profile, which fails when `HEGEL_DEFAULT_PROFILE` names an
 * unknown profile or a `hegel.toml` is malformed. The output callback taken by
 * `hegel_run_start` / `hegel_test_case_from_blob` is likewise absorbed as NULL
 * (engine output stays on stderr).
 *
 * String arguments are marshalled to `hegel_string_t` views of their UTF-8
 * bytes (length-delimited, so they may contain NUL), `null` to a NULL
 * pointer where the ABI takes an optional string; borrowed string results
 * are decoded to JS strings here, so only the owned `hegel_generate_string`
 * result reaches {@link Libhegel} as a raw buffer.
 */
export interface Bindings {
  contextNew: () => Ptr;
  contextFree: (ctx: Ptr) => void;
  contextLastError: (ctx: Ptr) => string;

  settingsNew: (ctx: Ptr, out: Ptr[]) => number;
  settingsFree: (s: Ptr) => void;
  settingsTestCases: (s: Ptr, n: number) => void;
  settingsVerbosity: (s: Ptr, v: number) => void;
  settingsSeed: (s: Ptr, seed: bigint, hasSeed: boolean) => void;
  settingsDerandomize: (s: Ptr, on: boolean) => void;
  settingsDatabase: (ctx: Ptr, s: Ptr, db: string | null) => void;
  settingsDatabaseKey: (ctx: Ptr, s: Ptr, key: string | null) => void;
  settingsSuppressHealthCheck: (s: Ptr, checks: number) => void;
  settingsReportMultipleFailures: (s: Ptr, yes: boolean) => void;

  runStart: (ctx: Ptr, settings: Ptr, out: Ptr[]) => number;
  nextTestCase: (ctx: Ptr, run: Ptr, out: Ptr[]) => number;
  runResult: (ctx: Ptr, run: Ptr, out: Ptr[]) => number;
  runResultFree: (r: Ptr) => void;
  runFree: (run: Ptr) => void;

  testCaseFromBlob: (ctx: Ptr, s: Ptr, blob: string, out: Ptr[]) => number;
  testCaseFree: (tc: Ptr) => void;

  /**
   * The `forced` / `has_forced` pair of `hegel_generate_boolean` is absorbed
   * as `false, false` (no forced value: the draw is always made from `p`).
   */
  generateBoolean: (ctx: Ptr, tc: Ptr, p: number, out: boolean[]) => number;
  generateInteger: (
    ctx: Ptr,
    tc: Ptr,
    min: bigint,
    max: bigint,
    out: (number | bigint)[],
  ) => number;
  generateIntegerBig: (
    ctx: Ptr,
    tc: Ptr,
    min: Uint8Array,
    max: Uint8Array,
    outValue: Buffer,
    outLen: (number | bigint)[],
  ) => number;
  generateFloat: (ctx: Ptr, tc: Ptr, opts: NativeFloatOptions, out: number[]) => number;
  generateBytes: (ctx: Ptr, tc: Ptr, min: bigint, max: bigint, out: NativeBuffer[]) => number;
  generateBytesResultFree: (result: NativeBuffer) => void;

  stringGeneratorFree: (generator: Ptr) => void;
  stringGeneratorText: (ctx: Ptr, opts: TextGeneratorOptions, out: Ptr[]) => number;
  /**
   * The optional `alphabet` generator is absorbed as NULL (this client never
   * constrains the regex alphabet).
   */
  stringGeneratorRegex: (ctx: Ptr, pattern: string, fullmatch: boolean, out: Ptr[]) => number;
  stringGeneratorEmail: (ctx: Ptr, out: Ptr[]) => number;
  stringGeneratorUrl: (ctx: Ptr, out: Ptr[]) => number;
  stringGeneratorDomain: (ctx: Ptr, maxLength: number, out: Ptr[]) => number;
  generateString: (ctx: Ptr, tc: Ptr, generator: Ptr, out: NativeBuffer[]) => number;
  ownedStringFree: (result: NativeBuffer) => void;

  dateOptionsNew: (ctx: Ptr, out: Ptr[]) => number;
  dateOptionsFree: (options: Ptr) => void;
  dateOptionsSetWeekdays: (ctx: Ptr, options: Ptr, mask: number) => number;
  dateOptionsSetMonths: (ctx: Ptr, options: Ptr, mask: number) => number;
  generateDate: (
    ctx: Ptr,
    tc: Ptr,
    min: NativeDate,
    max: NativeDate,
    options: Ptr,
    out: NativeDate[],
  ) => number;
  generateTime: (
    ctx: Ptr,
    tc: Ptr,
    min: NativeTime,
    max: NativeTime,
    precision: number,
    out: NativeTime[],
  ) => number;
  generateDatetime: (
    ctx: Ptr,
    tc: Ptr,
    min: NativeDatetime,
    max: NativeDatetime,
    precision: number,
    out: NativeDatetime[],
  ) => number;
  generateTimestamp: (
    ctx: Ptr,
    tc: Ptr,
    min: NativeTimestamp,
    max: NativeTimestamp,
    precision: number,
    out: RawTimestamp[],
  ) => number;
  generateOffset: (
    ctx: Ptr,
    tc: Ptr,
    min: number,
    max: number,
    granularity: number,
    allowUnknown: boolean,
    out: NativeOffset[],
  ) => number;
  timezoneInfo: (ctx: Ptr, name: string, out: RawTimeZoneInfo[]) => number;
  timezoneSpecNew: (ctx: Ptr, out: Ptr[]) => number;
  timezoneSpecFree: (spec: Ptr) => void;
  timezoneSpecSetNames: (ctx: Ptr, spec: Ptr, names: readonly string[]) => number;
  generateTimezone: (ctx: Ptr, tc: Ptr, spec: Ptr, out: NativeBuffer[]) => number;
  zonedSpecNew: (ctx: Ptr, out: Ptr[]) => number;
  zonedSpecFree: (spec: Ptr) => void;
  zonedSpecSetCivilBounds: (
    ctx: Ptr,
    spec: Ptr,
    min: NativeDatetime,
    max: NativeDatetime,
  ) => number;
  zonedSpecSetInstantBounds: (
    ctx: Ptr,
    spec: Ptr,
    min: NativeTimestamp,
    max: NativeTimestamp,
  ) => number;
  zonedSpecSetPrecision: (ctx: Ptr, spec: Ptr, precision: number) => number;
  zonedSpecSetImaginary: (ctx: Ptr, spec: Ptr, imaginary: number) => number;
  zonedSpecSetAmbiguous: (ctx: Ptr, spec: Ptr, ambiguous: number) => number;
  generateZonedDatetime: (
    ctx: Ptr,
    tc: Ptr,
    tzSpec: Ptr,
    zonedSpec: Ptr,
    outValue: RawZonedDatetime[],
    outZone: NativeBuffer[],
  ) => number;
  spanSpecNew: (ctx: Ptr, out: Ptr[]) => number;
  spanSpecFree: (spec: Ptr) => void;
  spanSpecSetUnitBounds: (ctx: Ptr, spec: Ptr, unit: number, min: bigint, max: bigint) => number;
  spanSpecSetSign: (ctx: Ptr, spec: Ptr, sign: number) => number;
  generateSpan: (ctx: Ptr, tc: Ptr, spec: Ptr, out: RawSpan[]) => number;
  generateUuid: (
    ctx: Ptr,
    tc: Ptr,
    version: number,
    hasVersion: boolean,
    outBytes: Buffer,
  ) => number;
  generateIpv4: (ctx: Ptr, tc: Ptr, outBytes: Buffer) => number;
  generateIpv6: (ctx: Ptr, tc: Ptr, outBytes: Buffer) => number;

  startSpan: (ctx: Ptr, tc: Ptr, label: number) => number;
  stopSpan: (ctx: Ptr, tc: Ptr, discard: boolean) => number;
  newCollection: (ctx: Ptr, tc: Ptr, min: number, max: bigint, out: Ptr[]) => number;
  collectionMore: (ctx: Ptr, tc: Ptr, collection: Ptr, out: boolean[]) => number;
  collectionReject: (ctx: Ptr, tc: Ptr, collection: Ptr, why: string | null) => number;
  collectionFree: (collection: Ptr) => void;
  markComplete: (ctx: Ptr, tc: Ptr, status: number, origin: string | null) => number;

  newPool: (ctx: Ptr, tc: Ptr, out: Ptr[]) => number;
  poolAdd: (ctx: Ptr, tc: Ptr, pool: Ptr, out: (number | bigint)[]) => number;
  poolGenerate: (
    ctx: Ptr,
    tc: Ptr,
    pool: Ptr,
    consume: boolean,
    out: (number | bigint)[],
  ) => number;
  poolFree: (pool: Ptr) => void;

  /**
   * The `rule_weights` array of `hegel_new_state_machine` is absorbed as NULL
   * (all rules equally likely): this client does not expose rule weights.
   */
  newStateMachine: (
    ctx: Ptr,
    tc: Ptr,
    opts: StateMachineOptions,
    outMachine: Ptr[],
    outConcurrency: (number | bigint)[],
  ) => number;
  stateMachineNextGroup: (ctx: Ptr, tc: Ptr, machine: Ptr, out: (number | bigint)[]) => number;
  stateMachineNextRule: (
    ctx: Ptr,
    tc: Ptr,
    machine: Ptr,
    workerIndex: number,
    out: (number | bigint)[],
  ) => number;
  stateMachineRuleRejected: (ctx: Ptr, tc: Ptr, machine: Ptr, workerIndex: number) => number;
  stateMachineShouldCheckInvariant: (
    ctx: Ptr,
    tc: Ptr,
    machine: Ptr,
    invariantIndex: number,
    out: boolean[],
  ) => number;
  stateMachineFree: (machine: Ptr) => void;

  runResultStatus: (r: Ptr) => number;
  runResultError: (r: Ptr) => string | null;
  runResultFailureCount: (r: Ptr) => number;
  runResultFailure: (r: Ptr, index: number) => Ptr;
  failureFree: (f: Ptr) => void;
  failureOrigin: (f: Ptr) => string;
  failureReproductionBlob: (f: Ptr) => string | null;

  version: () => string;
}

/**
 * Bind every libhegel function used by the client against a loaded koffi
 * library handle.
 *
 * Constructors, frees, setters and result getters check result codes here.
 * APIs without a context in the Engine interface pass NULL, which opts out of
 * detailed diagnostics, not error checking. Draw codes are mapped by Libhegel.
 */
export function bindLibrary(lib: LibraryHandle): Bindings {
  // The koffi FFI boundary is inherently dynamically typed; `Bindings` re-imposes
  // static types on the wrappers below.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const f = (proto: string): ((...args: any[]) => any) => lib.func(proto);
  // Same, for the ABI's struct-by-value functions, which koffi cannot express
  // in prototype-string form with anonymous struct types.
  type KoffiTypeSpec = string | TypeObject | ReturnType<typeof koffi.out>;
  const fs = (
    name: string,
    args: KoffiTypeSpec[],
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  ): ((...args: any[]) => any) => lib.func(name, "int", args);

  const contextNew = f("void* hegel_context_new()");
  const contextFree = f("int hegel_context_free(void* ctx)");
  const contextLastError = lib.func("hegel_context_last_error", stringType, ["void*"]);

  const settingsNew = f("int hegel_settings_new(void* ctx, _Out_ void** out)");
  const settingsFree = f("int hegel_settings_free(void* ctx, void* s)");
  const settingsTestCases = f("int hegel_settings_set_test_cases(void* ctx, void* s, uint64_t n)");
  const settingsVerbosity = f("int hegel_settings_set_verbosity(void* ctx, void* s, uint32_t v)");
  const settingsSeed = f(
    "int hegel_settings_set_seed(void* ctx, void* s, uint64_t seed, bool has_seed)",
  );
  const settingsDerandomize = f("int hegel_settings_set_derandomize(void* ctx, void* s, bool d)");
  const settingsDatabase = fs("hegel_settings_set_database", [
    "void*",
    "void*",
    koffi.pointer(stringType),
  ]);
  const settingsDatabaseKey = fs("hegel_settings_set_database_key", [
    "void*",
    "void*",
    koffi.pointer(stringType),
  ]);
  const settingsSuppressHealthCheck = f(
    "int hegel_settings_set_suppress_health_check(void* ctx, void* s, uint32_t checks)",
  );
  const settingsReportMultipleFailures = f(
    "int hegel_settings_set_report_multiple_failures(void* ctx, void* s, bool yes)",
  );

  const runStart = f(
    "int hegel_run_start(void* ctx, void* settings, void* callback, void* user_data, _Out_ void** out_run)",
  );
  const nextTestCase = f("int hegel_next_test_case(void* ctx, void* run, _Out_ void** out_tc)");
  const runResult = f("int hegel_run_result(void* ctx, void* run, _Out_ void** out_result)");
  const runResultFree = f("int hegel_run_result_free(void* ctx, void* r)");
  const runFree = f("int hegel_run_free(void* ctx, void* run)");

  const testCaseFromBlob = fs("hegel_test_case_from_blob", [
    "void*",
    "void*",
    stringType,
    "void*",
    "void*",
    koffi.out(koffi.pointer("void*")),
  ]);
  const testCaseFree = f("int hegel_test_case_free(void* ctx, void* tc)");

  const generateBoolean = f(
    "int hegel_generate_boolean(void* ctx, void* tc, double p, bool forced, bool has_forced, _Out_ bool* out)",
  );
  const generateInteger = f(
    "int hegel_generate_integer(void* ctx, void* tc, int64_t min, int64_t max, _Out_ int64_t* out)",
  );
  const generateIntegerBig = f(
    "int hegel_generate_integer_big(void* ctx, void* tc, const uint8_t* min, size_t min_len, const uint8_t* max, size_t max_len, _Out_ uint8_t* out_value, size_t out_value_cap, _Out_ size_t* out_len)",
  );
  const generateFloat = f(
    "int hegel_generate_float(void* ctx, void* tc, uint32_t width, double min, double max, bool allow_nan, bool allow_infinity, bool exclude_min, bool exclude_max, double smallest_nonzero_magnitude, _Out_ double* out)",
  );
  const generateBytes = fs("hegel_generate_bytes", [
    "void*",
    "void*",
    "uint64_t",
    "uint64_t",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const generateBytesResultFree = fs("hegel_generate_bytes_result_free", [
    "void*",
    koffi.pointer(stringType),
  ]);

  const stringGeneratorFree = f("int hegel_string_generator_free(void* ctx, void* generator)");
  const stringGeneratorText = fs("hegel_string_generator_text", [
    "void*",
    "uint64_t",
    "uint64_t",
    koffi.pointer(stringType),
    "uint32_t",
    "uint32_t",
    koffi.pointer(stringType),
    "size_t",
    koffi.pointer(stringType),
    "size_t",
    stringType,
    stringType,
    koffi.out(koffi.pointer("void*")),
  ]);
  const stringGeneratorRegex = fs("hegel_string_generator_regex", [
    "void*",
    stringType,
    "bool",
    "void*",
    koffi.out(koffi.pointer("void*")),
  ]);
  const stringGeneratorEmail = f("int hegel_string_generator_email(void* ctx, _Out_ void** out)");
  const stringGeneratorUrl = f("int hegel_string_generator_url(void* ctx, _Out_ void** out)");
  const stringGeneratorDomain = f(
    "int hegel_string_generator_domain(void* ctx, uint64_t max_length, _Out_ void** out)",
  );
  const generateString = fs("hegel_generate_string", [
    "void*",
    "void*",
    "void*",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const ownedStringFree = fs("hegel_owned_string_free", ["void*", koffi.pointer(stringType)]);

  const dateOptionsNew = f("int hegel_date_options_new(void* ctx, _Out_ void** out)");
  const dateOptionsFree = f("int hegel_date_options_free(void* ctx, void* options)");
  const dateOptionsSetWeekdays = f(
    "int hegel_date_options_set_weekdays(void* ctx, void* options, uint32_t mask)",
  );
  const dateOptionsSetMonths = f(
    "int hegel_date_options_set_months(void* ctx, void* options, uint32_t mask)",
  );
  const generateDate = fs("hegel_generate_date", [
    "void*",
    "void*",
    dateType,
    dateType,
    "void*",
    koffi.out(koffi.pointer(dateType)),
  ]);
  const generateTime = fs("hegel_generate_time", [
    "void*",
    "void*",
    timeType,
    timeType,
    "uint32_t",
    koffi.out(koffi.pointer(timeType)),
  ]);
  const generateDatetime = fs("hegel_generate_datetime", [
    "void*",
    "void*",
    datetimeType,
    datetimeType,
    "uint32_t",
    koffi.out(koffi.pointer(datetimeType)),
  ]);
  const generateTimestamp = fs("hegel_generate_timestamp", [
    "void*",
    "void*",
    timestampType,
    timestampType,
    "uint32_t",
    koffi.out(koffi.pointer(timestampType)),
  ]);
  const generateOffset = fs("hegel_generate_offset", [
    "void*",
    "void*",
    "int32_t",
    "int32_t",
    "uint32_t",
    "bool",
    koffi.out(koffi.pointer(offsetType)),
  ]);
  const timezoneInfo = fs("hegel_timezone_info", [
    "void*",
    stringType,
    koffi.out(koffi.pointer(timezoneInfoType)),
  ]);
  const timezoneSpecNew = f("int hegel_timezone_spec_new(void* ctx, _Out_ void** out)");
  const timezoneSpecFree = f("int hegel_timezone_spec_free(void* ctx, void* spec)");
  const timezoneSpecSetNames = fs("hegel_timezone_spec_set_names", [
    "void*",
    "void*",
    koffi.pointer(stringType),
    "size_t",
  ]);
  const generateTimezone = fs("hegel_generate_timezone", [
    "void*",
    "void*",
    "void*",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const zonedSpecNew = f("int hegel_zoned_spec_new(void* ctx, _Out_ void** out)");
  const zonedSpecFree = f("int hegel_zoned_spec_free(void* ctx, void* spec)");
  const zonedSpecSetCivilBounds = fs("hegel_zoned_spec_set_civil_bounds", [
    "void*",
    "void*",
    datetimeType,
    datetimeType,
  ]);
  const zonedSpecSetInstantBounds = fs("hegel_zoned_spec_set_instant_bounds", [
    "void*",
    "void*",
    timestampType,
    timestampType,
  ]);
  const zonedSpecSetPrecision = f(
    "int hegel_zoned_spec_set_precision(void* ctx, void* spec, uint32_t precision)",
  );
  const zonedSpecSetImaginary = f(
    "int hegel_zoned_spec_set_imaginary(void* ctx, void* spec, uint32_t imaginary)",
  );
  const zonedSpecSetAmbiguous = f(
    "int hegel_zoned_spec_set_ambiguous(void* ctx, void* spec, uint32_t ambiguous)",
  );
  const generateZonedDatetime = fs("hegel_generate_zoned_datetime", [
    "void*",
    "void*",
    "void*",
    "void*",
    koffi.out(koffi.pointer(zonedDatetimeType)),
    koffi.out(koffi.pointer(stringType)),
  ]);
  const spanSpecNew = f("int hegel_span_spec_new(void* ctx, _Out_ void** out)");
  const spanSpecFree = f("int hegel_span_spec_free(void* ctx, void* spec)");
  const spanSpecSetUnitBounds = f(
    "int hegel_span_spec_set_unit_bounds(void* ctx, void* spec, uint32_t unit, int64_t min, int64_t max)",
  );
  const spanSpecSetSign = f("int hegel_span_spec_set_sign(void* ctx, void* spec, uint32_t sign)");
  const generateSpan = fs("hegel_generate_span", [
    "void*",
    "void*",
    "void*",
    koffi.out(koffi.pointer(spanType)),
  ]);
  const generateUuid = f(
    "int hegel_generate_uuid(void* ctx, void* tc, uint8_t version, bool has_version, _Out_ uint8_t* out_bytes)",
  );
  const generateIpv4 = f("int hegel_generate_ipv4(void* ctx, void* tc, _Out_ uint8_t* out_bytes)");
  const generateIpv6 = f("int hegel_generate_ipv6(void* ctx, void* tc, _Out_ uint8_t* out_bytes)");

  const startSpan = f("int hegel_start_span(void* ctx, void* tc, uint64_t label)");
  const stopSpan = f("int hegel_stop_span(void* ctx, void* tc, bool discard)");
  const newCollection = f(
    "int hegel_new_collection(void* ctx, void* tc, uint64_t min_size, uint64_t max_size, _Out_ void** out_collection)",
  );
  const collectionMore = f(
    "int hegel_collection_more(void* ctx, void* tc, void* collection, _Out_ bool* out)",
  );
  const collectionReject = fs("hegel_collection_reject", [
    "void*",
    "void*",
    "void*",
    koffi.pointer(stringType),
  ]);
  const collectionFree = f("int hegel_collection_free(void* ctx, void* collection)");
  const markComplete = fs("hegel_mark_complete", [
    "void*",
    "void*",
    "uint32_t",
    koffi.pointer(stringType),
  ]);

  const newPool = f("int hegel_new_pool(void* ctx, void* tc, _Out_ void** out_pool)");
  const poolAdd = f(
    "int hegel_pool_add(void* ctx, void* tc, void* pool, _Out_ int64_t* out_variable_id)",
  );
  const poolGenerate = f(
    "int hegel_pool_generate(void* ctx, void* tc, void* pool, bool consume, _Out_ int64_t* out_variable_id)",
  );
  const poolFree = f("int hegel_pool_free(void* ctx, void* pool)");

  const newStateMachine = fs("hegel_new_state_machine", [
    "void*",
    "void*",
    koffi.pointer(stringType),
    koffi.pointer("int64_t"),
    koffi.pointer("double"),
    "size_t",
    koffi.pointer(stringType),
    koffi.pointer("bool"),
    "size_t",
    "int64_t",
    "int64_t",
    "int64_t",
    koffi.out(koffi.pointer("void*")),
    koffi.out(koffi.pointer("int64_t")),
  ]);
  const stateMachineNextGroup = f(
    "int hegel_state_machine_next_group(void* ctx, void* tc, void* state_machine, _Out_ int64_t* out_group_id)",
  );
  const stateMachineNextRule = f(
    "int hegel_state_machine_next_rule(void* ctx, void* tc, void* state_machine, int64_t worker_index, _Out_ int64_t* out_rule_index)",
  );
  const stateMachineRuleRejected = f(
    "int hegel_state_machine_rule_rejected(void* ctx, void* tc, void* state_machine, int64_t worker_index)",
  );
  const stateMachineShouldCheckInvariant = f(
    "int hegel_state_machine_should_check_invariant(void* ctx, void* tc, void* state_machine, int64_t invariant_index, _Out_ bool* out_should_check)",
  );
  const stateMachineFree = f("int hegel_state_machine_free(void* ctx, void* state_machine)");

  const runResultStatus = f("int hegel_run_result_status(void* ctx, void* r, _Out_ int* out)");
  const runResultError = f("int hegel_run_result_error(void* ctx, void* r, _Out_ void** out)");
  const runResultFailureCount = f(
    "int hegel_run_result_failure_count(void* ctx, void* r, _Out_ size_t* out)",
  );
  const runResultFailure = f(
    "int hegel_run_result_failure(void* ctx, void* r, size_t index, _Out_ void** out)",
  );
  const failureFree = f("int hegel_failure_free(void* ctx, void* f)");
  const failureOrigin = fs("hegel_failure_origin", [
    "void*",
    "void*",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const failureReproductionBlob = f(
    "int hegel_failure_reproduction_blob(void* ctx, void* f, _Out_ void** out)",
  );
  const version = fs("hegel_version", ["void*", koffi.out(koffi.pointer(stringType))]);

  const readString = (result: NativeBuffer): string => wtf8ToString(copyNativeBuffer(result));
  const readOptionalString = (pointer: Ptr): string | null =>
    pointer === null ? null : readString(koffi.decode(pointer, stringType) as NativeBuffer);
  const nativeString = (value: string): NativeBuffer => {
    const data = Buffer.from(value, "utf8");
    return { data, len: data.length };
  };
  const optionalString = (value: string | null): NativeBuffer | null =>
    value === null ? null : nativeString(value);
  const nativeBytes = (value: Uint8Array | null): NativeBuffer =>
    value === null ? { data: null, len: 0 } : { data: value, len: value.length };

  const checked = (code: number, op: string, ctx: Ptr = null): void => {
    if (code !== RESULT_OK) {
      const message = ctx === null ? "" : readString(contextLastError(ctx));
      throw new LibhegelError(`${op} failed${message ? `: ${message}` : ""}`, code);
    }
  };

  return {
    contextNew: () => contextNew(),
    contextFree: (ctx) => checked(contextFree(ctx), "hegel_context_free"),
    contextLastError: (ctx) => readString(contextLastError(ctx)),
    settingsNew: (ctx, out) => settingsNew(ctx, out),
    settingsFree: (s) => checked(settingsFree(null, s), "hegel_settings_free"),
    settingsTestCases: (s, n) => checked(settingsTestCases(null, s, n), "settingsTestCases"),
    settingsVerbosity: (s, v) => checked(settingsVerbosity(null, s, v), "settingsVerbosity"),
    settingsSeed: (s, seed, hasSeed) =>
      checked(settingsSeed(null, s, seed, hasSeed), "settingsSeed"),
    settingsDerandomize: (s, on) =>
      checked(settingsDerandomize(null, s, on), "settingsDerandomize"),
    settingsDatabase: (ctx, s, db) =>
      checked(settingsDatabase(ctx, s, optionalString(db)), "settingsDatabase", ctx),
    settingsDatabaseKey: (ctx, s, key) =>
      checked(settingsDatabaseKey(ctx, s, optionalString(key)), "settingsDatabaseKey", ctx),
    settingsSuppressHealthCheck: (s, checks) =>
      checked(settingsSuppressHealthCheck(null, s, checks), "settingsSuppressHealthCheck"),
    settingsReportMultipleFailures: (s, yes) =>
      checked(settingsReportMultipleFailures(null, s, yes), "settingsReportMultipleFailures"),
    runStart: (ctx, s, out) => runStart(ctx, s, null, null, out),
    nextTestCase: (ctx, run, out) => nextTestCase(ctx, run, out),
    runResult: (ctx, run, out) => runResult(ctx, run, out),
    runResultFree: (r) => checked(runResultFree(null, r), "runResultFree"),
    runFree: (run) => checked(runFree(null, run), "runFree"),
    testCaseFromBlob: (ctx, s, blob, out) =>
      testCaseFromBlob(ctx, s, nativeString(blob), null, null, out),
    testCaseFree: (tc) => checked(testCaseFree(null, tc), "testCaseFree"),
    generateBoolean: (ctx, tc, p, out) => generateBoolean(ctx, tc, p, false, false, out),
    generateInteger: (ctx, tc, min, max, out) => generateInteger(ctx, tc, min, max, out),
    generateIntegerBig: (ctx, tc, min, max, outValue, outLen) =>
      generateIntegerBig(
        ctx,
        tc,
        min,
        min.length,
        max,
        max.length,
        outValue,
        outValue.length,
        outLen,
      ),
    generateFloat: (ctx, tc, opts, out) =>
      generateFloat(
        ctx,
        tc,
        opts.width,
        opts.minValue,
        opts.maxValue,
        opts.allowNan,
        opts.allowInfinity,
        opts.excludeMin,
        opts.excludeMax,
        opts.smallestNonzeroMagnitude,
        out,
      ),
    generateBytes: (ctx, tc, min, max, out) => generateBytes(ctx, tc, min, max, out),
    generateBytesResultFree: (result) =>
      checked(generateBytesResultFree(null, result), "generateBytesResultFree"),
    stringGeneratorFree: (generator) =>
      checked(stringGeneratorFree(null, generator), "hegel_string_generator_free"),
    stringGeneratorText: (ctx, opts, out) =>
      stringGeneratorText(
        ctx,
        opts.minSize,
        opts.maxSize,
        optionalString(opts.codec),
        opts.minCodepoint,
        opts.maxCodepoint,
        opts.categories?.map(nativeString) ?? null,
        opts.categories === null ? 0 : opts.categories.length,
        opts.excludeCategories?.map(nativeString) ?? null,
        opts.excludeCategories === null ? 0 : opts.excludeCategories.length,
        nativeBytes(opts.includeCharacters),
        nativeBytes(opts.excludeCharacters),
        out,
      ),
    stringGeneratorRegex: (ctx, pattern, fullmatch, out) =>
      stringGeneratorRegex(ctx, nativeString(pattern), fullmatch, null, out),
    stringGeneratorEmail: (ctx, out) => stringGeneratorEmail(ctx, out),
    stringGeneratorUrl: (ctx, out) => stringGeneratorUrl(ctx, out),
    stringGeneratorDomain: (ctx, maxLength, out) => stringGeneratorDomain(ctx, maxLength, out),
    generateString: (ctx, tc, generator, out) => generateString(ctx, tc, generator, out),
    ownedStringFree: (result) => checked(ownedStringFree(null, result), "ownedStringFree"),
    dateOptionsNew: (ctx, out) => dateOptionsNew(ctx, out),
    dateOptionsFree: (options) => checked(dateOptionsFree(null, options), "dateOptionsFree"),
    dateOptionsSetWeekdays: (ctx, options, mask) => dateOptionsSetWeekdays(ctx, options, mask),
    dateOptionsSetMonths: (ctx, options, mask) => dateOptionsSetMonths(ctx, options, mask),
    generateDate: (ctx, tc, min, max, options, out) =>
      generateDate(ctx, tc, min, max, options, out),
    generateTime: (ctx, tc, min, max, precision, out) =>
      generateTime(ctx, tc, min, max, precision, out),
    generateDatetime: (ctx, tc, min, max, precision, out) =>
      generateDatetime(ctx, tc, min, max, precision, out),
    generateTimestamp: (ctx, tc, min, max, precision, out) =>
      generateTimestamp(ctx, tc, min, max, precision, out),
    generateOffset: (ctx, tc, min, max, granularity, allowUnknown, out) =>
      generateOffset(ctx, tc, min, max, granularity, allowUnknown, out),
    timezoneInfo: (ctx, name, out) => timezoneInfo(ctx, nativeString(name), out),
    timezoneSpecNew: (ctx, out) => timezoneSpecNew(ctx, out),
    timezoneSpecFree: (spec) => checked(timezoneSpecFree(null, spec), "timezoneSpecFree"),
    timezoneSpecSetNames: (ctx, spec, names) =>
      timezoneSpecSetNames(ctx, spec, names.map(nativeString), names.length),
    generateTimezone: (ctx, tc, spec, out) => generateTimezone(ctx, tc, spec, out),
    zonedSpecNew: (ctx, out) => zonedSpecNew(ctx, out),
    zonedSpecFree: (spec) => checked(zonedSpecFree(null, spec), "zonedSpecFree"),
    zonedSpecSetCivilBounds: (ctx, spec, min, max) => zonedSpecSetCivilBounds(ctx, spec, min, max),
    zonedSpecSetInstantBounds: (ctx, spec, min, max) =>
      zonedSpecSetInstantBounds(ctx, spec, min, max),
    zonedSpecSetPrecision: (ctx, spec, precision) => zonedSpecSetPrecision(ctx, spec, precision),
    zonedSpecSetImaginary: (ctx, spec, imaginary) => zonedSpecSetImaginary(ctx, spec, imaginary),
    zonedSpecSetAmbiguous: (ctx, spec, ambiguous) => zonedSpecSetAmbiguous(ctx, spec, ambiguous),
    generateZonedDatetime: (ctx, tc, tzSpec, zonedSpec, outValue, outZone) =>
      generateZonedDatetime(ctx, tc, tzSpec, zonedSpec, outValue, outZone),
    spanSpecNew: (ctx, out) => spanSpecNew(ctx, out),
    spanSpecFree: (spec) => checked(spanSpecFree(null, spec), "spanSpecFree"),
    spanSpecSetUnitBounds: (ctx, spec, unit, min, max) =>
      spanSpecSetUnitBounds(ctx, spec, unit, min, max),
    spanSpecSetSign: (ctx, spec, sign) => spanSpecSetSign(ctx, spec, sign),
    generateSpan: (ctx, tc, spec, out) => generateSpan(ctx, tc, spec, out),
    generateUuid: (ctx, tc, version, hasVersion, outBytes) =>
      generateUuid(ctx, tc, version, hasVersion, outBytes),
    generateIpv4: (ctx, tc, outBytes) => generateIpv4(ctx, tc, outBytes),
    generateIpv6: (ctx, tc, outBytes) => generateIpv6(ctx, tc, outBytes),
    startSpan: (ctx, tc, label) => startSpan(ctx, tc, label),
    stopSpan: (ctx, tc, discard) => stopSpan(ctx, tc, discard),
    newCollection: (ctx, tc, min, max, out) => newCollection(ctx, tc, min, max, out),
    collectionMore: (ctx, tc, collection, out) => collectionMore(ctx, tc, collection, out),
    collectionReject: (ctx, tc, collection, why) =>
      collectionReject(ctx, tc, collection, optionalString(why)),
    collectionFree: (collection) => checked(collectionFree(null, collection), "collectionFree"),
    markComplete: (ctx, tc, status, origin) =>
      markComplete(ctx, tc, status, optionalString(origin)),
    newPool: (ctx, tc, out) => newPool(ctx, tc, out),
    poolAdd: (ctx, tc, pool, out) => poolAdd(ctx, tc, pool, out),
    poolGenerate: (ctx, tc, pool, consume, out) => poolGenerate(ctx, tc, pool, consume, out),
    poolFree: (pool) => checked(poolFree(null, pool), "poolFree"),
    newStateMachine: (ctx, tc, opts, outMachine, outConcurrency) =>
      newStateMachine(
        ctx,
        tc,
        opts.ruleNames.map(nativeString),
        opts.ruleGroups,
        null,
        opts.ruleNames.length,
        opts.invariantNames.map(nativeString),
        opts.invariantAlwaysCheck,
        opts.invariantNames.length,
        opts.minConcurrency,
        opts.maxConcurrency,
        opts.stepCount,
        outMachine,
        outConcurrency,
      ),
    stateMachineNextGroup: (ctx, tc, machine, out) => stateMachineNextGroup(ctx, tc, machine, out),
    stateMachineNextRule: (ctx, tc, machine, workerIndex, out) =>
      stateMachineNextRule(ctx, tc, machine, workerIndex, out),
    stateMachineRuleRejected: (ctx, tc, machine, workerIndex) =>
      stateMachineRuleRejected(ctx, tc, machine, workerIndex),
    stateMachineShouldCheckInvariant: (ctx, tc, machine, invariantIndex, out) =>
      stateMachineShouldCheckInvariant(ctx, tc, machine, invariantIndex, out),
    stateMachineFree: (machine) => checked(stateMachineFree(null, machine), "stateMachineFree"),
    runResultStatus: (r) => {
      const out: number[] = [0];
      checked(runResultStatus(null, r, out), "runResultStatus");
      return out[0];
    },
    runResultError: (r) => {
      const out: Ptr[] = [null];
      checked(runResultError(null, r, out), "runResultError");
      return readOptionalString(out[0]);
    },
    runResultFailureCount: (r) => {
      const out: (number | bigint)[] = [0];
      checked(runResultFailureCount(null, r, out), "runResultFailureCount");
      return Number(out[0]);
    },
    runResultFailure: (r, index) => {
      const out: Ptr[] = [null];
      checked(runResultFailure(null, r, index, out), "runResultFailure");
      return out[0];
    },
    failureFree: (fp) => checked(failureFree(null, fp), "failureFree"),
    failureOrigin: (fp) => {
      const out: NativeBuffer[] = [{ data: null, len: 0 }];
      checked(failureOrigin(null, fp, out), "failureOrigin");
      return readString(out[0]);
    },
    failureReproductionBlob: (fp) => {
      const out: Ptr[] = [null];
      checked(failureReproductionBlob(null, fp, out), "failureReproductionBlob");
      return readOptionalString(out[0]);
    },
    version: () => {
      const out: NativeBuffer[] = [{ data: null, len: 0 }];
      checked(version(null, out), "version");
      return readString(out[0]);
    },
  };
}

const UINT64_MAX = 0xffffffffffffffffn;

function emptyDatetime(): NativeDatetime {
  return {
    date: { year: 0, month: 0, day: 0 },
    time: { hour: 0, minute: 0, second: 0, nanosecond: 0 },
  };
}

function timestampFromRaw(raw: RawTimestamp): NativeTimestamp {
  return { second: BigInt(raw.second), nanosecond: raw.nanosecond };
}

/** Copy an engine-owned `{data, len}` buffer into a JS-owned Buffer. */
function copyNativeBuffer(result: NativeBuffer): Buffer {
  const len = Number(result.len);
  if (!Number.isSafeInteger(len) || len < 0 || (len > 0 && result.data === null)) {
    throw new EngineError("Invalid native buffer result");
  }
  if (len === 0) {
    return Buffer.alloc(0);
  }
  return Buffer.from(koffi.decode(result.data, "uint8_t", len) as unknown as number[]);
}

/**
 * High-level wrapper over the libhegel C ABI.
 */
export class Libhegel implements Engine {
  private readonly fns: Bindings;

  constructor(fns: Bindings) {
    this.fns = fns;
  }

  /** Load libhegel from a shared-library path. */
  static load(path: string): Libhegel {
    return new Libhegel(bindLibrary(koffi.load(path)));
  }

  private requireHandle<H>(value: Ptr, op: string): H {
    if (value === null || value === undefined)
      throw new EngineError(`${op} returned a null handle`);
    return value as H;
  }

  freeStringGenerator(generator: StringGeneratorHandle): void {
    this.fns.stringGeneratorFree(generator);
  }

  version(): string {
    return this.fns.version();
  }

  newContext(): ContextHandle {
    return this.requireHandle<ContextHandle>(this.fns.contextNew(), "hegel_context_new");
  }

  freeContext(ctx: Ptr): void {
    this.fns.contextFree(ctx);
  }

  lastError(ctx: Ptr): string {
    return this.fns.contextLastError(ctx);
  }

  /**
   * Create a settings handle initialized from the default profile. Throws
   * {@link LibhegelError} when the profile cannot be resolved (an unknown
   * `HEGEL_DEFAULT_PROFILE`, a malformed `hegel.toml`).
   */
  newSettings(ctx: Ptr): SettingsHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.settingsNew(ctx, out), "hegel_settings_new");
    return this.requireHandle<SettingsHandle>(out[0], "hegel_settings_new");
  }

  freeSettings(s: Ptr): void {
    this.fns.settingsFree(s);
  }

  setTestCases(s: Ptr, n: number): void {
    this.fns.settingsTestCases(s, n);
  }

  setVerbosity(s: Ptr, v: number): void {
    this.fns.settingsVerbosity(s, v);
  }

  setSeed(s: Ptr, seed: bigint): void {
    this.fns.settingsSeed(s, seed, true);
  }

  setDerandomize(s: Ptr, on: boolean): void {
    this.fns.settingsDerandomize(s, on);
  }

  setDatabase(ctx: Ptr, s: Ptr, db: string | null): void {
    this.fns.settingsDatabase(ctx, s, db);
  }

  setDatabaseKey(ctx: Ptr, s: Ptr, key: string): void {
    this.fns.settingsDatabaseKey(ctx, s, key);
  }

  setSuppressHealthCheck(s: Ptr, checks: number): void {
    this.fns.settingsSuppressHealthCheck(s, checks);
  }

  setReportMultipleFailures(s: Ptr, yes: boolean): void {
    this.fns.settingsReportMultipleFailures(s, yes);
  }

  /** Start a run. Throws {@link LibhegelError} on failure. */
  runStart(ctx: Ptr, settings: Ptr): RunHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.runStart(ctx, settings, out), "hegel_run_start");
    return this.requireHandle<RunHandle>(out[0], "runStart");
  }

  /**
   * Pull the next test case, or `null` when the run is finished. Throws if the
   * engine reported a mid-run error (e.g. the previous case was not completed).
   * The returned handle is owned by the caller — release it with
   * {@link freeTestCase} once the case is complete.
   */
  nextTestCase(ctx: Ptr, run: Ptr): TestCaseHandle | null {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.nextTestCase(ctx, run, out), "hegel_next_test_case");
    return (out[0] ?? null) as TestCaseHandle | null;
  }

  /**
   * Read the aggregated run result: a caller-owned copy, released with
   * {@link freeRunResult}. Throws on failure.
   */
  runResult(ctx: Ptr, run: Ptr): RunResultHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.runResult(ctx, run, out), "hegel_run_result");
    return this.requireHandle<RunResultHandle>(out[0], "runResult");
  }

  freeRunResult(r: Ptr): void {
    this.fns.runResultFree(r);
  }

  freeRun(run: Ptr): void {
    this.fns.runFree(run);
  }

  /**
   * Build a standalone test case that replays a base64 failure blob (from
   * {@link reproductionBlob}). Owned by the caller — release with
   * {@link freeTestCase}. Throws {@link LibhegelError} on a malformed blob.
   */
  testCaseFromBlob(ctx: Ptr, settings: Ptr, blob: string): TestCaseHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.testCaseFromBlob(ctx, settings, blob, out),
      "hegel_test_case_from_blob",
    );
    return this.requireHandle<TestCaseHandle>(out[0], "testCaseFromBlob");
  }

  freeTestCase(tc: Ptr): void {
    this.fns.testCaseFree(tc);
  }

  /**
   * Map a fallible `int`-returning result code to an exception.
   * `HEGEL_E_STOP_TEST` becomes {@link StopTestError}, `HEGEL_E_ASSUME` becomes
   * {@link AssumeError}; any other non-OK code becomes a {@link LibhegelError}
   * carrying the context diagnostic.
   */
  private check(ctx: Ptr, code: number, op: string): void {
    if (code === RESULT_OK) {
      return;
    }
    if (code === RESULT_STOP_TEST) {
      throw new StopTestError();
    }
    if (code === RESULT_ASSUME) {
      // The engine rejected this draw (e.g. a format generator's internal
      // precondition failed); discard the test case like a failed assume().
      throw new AssumeError();
    }
    throw new LibhegelError(`${op} failed: ${this.lastError(ctx)}`, code);
  }

  /** Draw a boolean that is `true` with probability `p`. */
  generateBoolean(ctx: Ptr, tc: Ptr, p: number): boolean {
    const out: boolean[] = [false];
    this.check(ctx, this.fns.generateBoolean(ctx, tc, p, out), "hegel_generate_boolean");
    return out[0];
  }

  /**
   * Draw an integer in `[min, max]`. Both bounds must satisfy
   * {@link fitsInt64}; use {@link generateIntegerBig} otherwise.
   */
  generateInteger(ctx: Ptr, tc: Ptr, min: bigint, max: bigint): bigint {
    const out: (number | bigint)[] = [0];
    this.check(ctx, this.fns.generateInteger(ctx, tc, min, max, out), "hegel_generate_integer");
    return BigInt(out[0]);
  }

  /** Draw an arbitrary-precision integer in `[min, max]`. */
  generateIntegerBig(ctx: Ptr, tc: Ptr, min: bigint, max: bigint): bigint {
    const minBytes = bigIntToTwosComplementLE(min);
    const maxBytes = bigIntToTwosComplementLE(max);
    // The draw is within the bounds, so max(min_len, max_len) always suffices;
    // the engine sign-fills the buffer, so decoding all of it needs no length.
    const outValue = Buffer.alloc(Math.max(minBytes.length, maxBytes.length));
    const outLen: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.generateIntegerBig(ctx, tc, minBytes, maxBytes, outValue, outLen),
      "hegel_generate_integer_big",
    );
    return twosComplementLEToBigInt(outValue);
  }

  /** Draw a float per the given `hegel_generate_float` options. */
  generateFloat(ctx: Ptr, tc: Ptr, opts: NativeFloatOptions): number {
    const out: number[] = [0];
    this.check(ctx, this.fns.generateFloat(ctx, tc, opts, out), "hegel_generate_float");
    return out[0];
  }

  /** Draw a byte string with length in `[minSize, maxSize]` (no bound if omitted). */
  generateBytes(ctx: Ptr, tc: Ptr, minSize: number, maxSize?: number): Buffer {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    const maxArg = maxSize === undefined ? UINT64_MAX : BigInt(maxSize);
    this.check(
      ctx,
      this.fns.generateBytes(ctx, tc, BigInt(minSize), maxArg, out),
      "hegel_generate_bytes",
    );
    try {
      return copyNativeBuffer(out[0]);
    } finally {
      this.fns.generateBytesResultFree(out[0]);
    }
  }

  /** Build a text string generator. Throws {@link LibhegelError} on invalid options. */
  stringGeneratorText(ctx: Ptr, opts: TextGeneratorOptions): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.stringGeneratorText(ctx, opts, out), "hegel_string_generator_text");
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorText");
  }

  /** Build a regex string generator. Throws {@link LibhegelError} on a bad pattern. */
  stringGeneratorRegex(ctx: Ptr, pattern: string, fullmatch: boolean): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.stringGeneratorRegex(ctx, pattern, fullmatch, out),
      "hegel_string_generator_regex",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorRegex");
  }

  /** Build an email-address string generator. */
  stringGeneratorEmail(ctx: Ptr): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.stringGeneratorEmail(ctx, out), "hegel_string_generator_email");
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorEmail");
  }

  /** Build a URL string generator. */
  stringGeneratorUrl(ctx: Ptr): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.stringGeneratorUrl(ctx, out), "hegel_string_generator_url");
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorUrl");
  }

  /** Build a domain-name string generator. Throws on an out-of-range length. */
  stringGeneratorDomain(ctx: Ptr, maxLength: number): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.stringGeneratorDomain(ctx, maxLength, out),
      "hegel_string_generator_domain",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorDomain");
  }

  /** Draw a string from a `hegel_string_generator_t`. */
  generateString(ctx: Ptr, tc: Ptr, generator: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(ctx, this.fns.generateString(ctx, tc, generator, out), "hegel_generate_string");
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.ownedStringFree(out[0]);
    }
  }

  /** Create date options allowing every weekday and month; release with {@link freeDateOptions}. */
  dateOptionsNew(ctx: Ptr): DateOptionsHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.dateOptionsNew(ctx, out), "hegel_date_options_new");
    return this.requireHandle<DateOptionsHandle>(out[0], "dateOptionsNew");
  }

  dateOptionsSetWeekdays(ctx: Ptr, options: Ptr, mask: number): void {
    this.check(
      ctx,
      this.fns.dateOptionsSetWeekdays(ctx, options, mask),
      "hegel_date_options_set_weekdays",
    );
  }

  dateOptionsSetMonths(ctx: Ptr, options: Ptr, mask: number): void {
    this.check(
      ctx,
      this.fns.dateOptionsSetMonths(ctx, options, mask),
      "hegel_date_options_set_months",
    );
  }

  freeDateOptions(options: Ptr): void {
    this.fns.dateOptionsFree(options);
  }

  /** Draw a date in `[min, max]`, within `options` when given. */
  generateDate(ctx: Ptr, tc: Ptr, min: NativeDate, max: NativeDate, options: Ptr): NativeDate {
    const out: NativeDate[] = [{ year: 0, month: 0, day: 0 }];
    this.check(ctx, this.fns.generateDate(ctx, tc, min, max, options, out), "hegel_generate_date");
    return out[0];
  }

  /** Draw a time of day in `[min, max]` at `precision`. */
  generateTime(ctx: Ptr, tc: Ptr, min: NativeTime, max: NativeTime, precision: number): NativeTime {
    const out: NativeTime[] = [{ hour: 0, minute: 0, second: 0, nanosecond: 0 }];
    this.check(
      ctx,
      this.fns.generateTime(ctx, tc, min, max, precision, out),
      "hegel_generate_time",
    );
    return out[0];
  }

  /** Draw a naive datetime in `[min, max]` at `precision`. */
  generateDatetime(
    ctx: Ptr,
    tc: Ptr,
    min: NativeDatetime,
    max: NativeDatetime,
    precision: number,
  ): NativeDatetime {
    const out: NativeDatetime[] = [emptyDatetime()];
    this.check(
      ctx,
      this.fns.generateDatetime(ctx, tc, min, max, precision, out),
      "hegel_generate_datetime",
    );
    return out[0];
  }

  /** Draw an instant in `[min, max]` at `precision`. */
  generateTimestamp(
    ctx: Ptr,
    tc: Ptr,
    min: NativeTimestamp,
    max: NativeTimestamp,
    precision: number,
  ): NativeTimestamp {
    const out: RawTimestamp[] = [{ second: 0, nanosecond: 0 }];
    this.check(
      ctx,
      this.fns.generateTimestamp(ctx, tc, min, max, precision, out),
      "hegel_generate_timestamp",
    );
    return timestampFromRaw(out[0]);
  }

  /** Draw a UTC offset in `[min, max]` seconds at `granularity`. */
  generateOffset(
    ctx: Ptr,
    tc: Ptr,
    min: number,
    max: number,
    granularity: number,
    allowUnknown: boolean,
  ): NativeOffset {
    const out: NativeOffset[] = [{ seconds: 0, known: false }];
    this.check(
      ctx,
      this.fns.generateOffset(ctx, tc, min, max, granularity, allowUnknown, out),
      "hegel_generate_offset",
    );
    return out[0];
  }

  /** Look `name` up in the bundled time zone database; `known` is false for an unknown name. */
  timezoneInfo(ctx: Ptr, name: string): NativeTimeZoneInfo {
    const out: RawTimeZoneInfo[] = [
      { known: false, primary_name: { data: null, len: 0 }, is_link: false, rank: 0, tags: 0 },
    ];
    this.check(ctx, this.fns.timezoneInfo(ctx, name, out), "hegel_timezone_info");
    const info = out[0];
    return {
      known: info.known,
      primaryName: info.known ? wtf8ToString(copyNativeBuffer(info.primary_name)) : "",
      isLink: info.is_link,
      rank: info.rank,
      tags: info.tags,
    };
  }

  /** Create a time zone spec allowing every bundled name; release with {@link freeTimezoneSpec}. */
  timezoneSpecNew(ctx: Ptr): TimeZoneSpecHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.timezoneSpecNew(ctx, out), "hegel_timezone_spec_new");
    return this.requireHandle<TimeZoneSpecHandle>(out[0], "timezoneSpecNew");
  }

  /** Restrict `spec` to `names`. Throws {@link LibhegelError} for a name the database lacks. */
  timezoneSpecSetNames(ctx: Ptr, spec: Ptr, names: readonly string[]): void {
    this.check(
      ctx,
      this.fns.timezoneSpecSetNames(ctx, spec, names),
      "hegel_timezone_spec_set_names",
    );
  }

  freeTimezoneSpec(spec: Ptr): void {
    this.fns.timezoneSpecFree(spec);
  }

  /** Draw a time zone name allowed by `spec`. */
  generateTimezone(ctx: Ptr, tc: Ptr, spec: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(ctx, this.fns.generateTimezone(ctx, tc, spec, out), "hegel_generate_timezone");
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.ownedStringFree(out[0]);
    }
  }

  /** Create a zoned datetime spec at its defaults; release with {@link freeZonedSpec}. */
  zonedSpecNew(ctx: Ptr): ZonedSpecHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.zonedSpecNew(ctx, out), "hegel_zoned_spec_new");
    return this.requireHandle<ZonedSpecHandle>(out[0], "zonedSpecNew");
  }

  zonedSpecSetCivilBounds(ctx: Ptr, spec: Ptr, min: NativeDatetime, max: NativeDatetime): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetCivilBounds(ctx, spec, min, max),
      "hegel_zoned_spec_set_civil_bounds",
    );
  }

  zonedSpecSetInstantBounds(ctx: Ptr, spec: Ptr, min: NativeTimestamp, max: NativeTimestamp): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetInstantBounds(ctx, spec, min, max),
      "hegel_zoned_spec_set_instant_bounds",
    );
  }

  zonedSpecSetPrecision(ctx: Ptr, spec: Ptr, precision: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetPrecision(ctx, spec, precision),
      "hegel_zoned_spec_set_precision",
    );
  }

  zonedSpecSetImaginary(ctx: Ptr, spec: Ptr, imaginary: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetImaginary(ctx, spec, imaginary),
      "hegel_zoned_spec_set_imaginary",
    );
  }

  zonedSpecSetAmbiguous(ctx: Ptr, spec: Ptr, ambiguous: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetAmbiguous(ctx, spec, ambiguous),
      "hegel_zoned_spec_set_ambiguous",
    );
  }

  freeZonedSpec(spec: Ptr): void {
    this.fns.zonedSpecFree(spec);
  }

  /** Draw a zone from `tzSpec` and a wall-clock time in it within `zonedSpec`. */
  generateZonedDatetime(
    ctx: Ptr,
    tc: Ptr,
    tzSpec: Ptr,
    zonedSpec: Ptr,
  ): { value: NativeZonedDatetime; zone: string } {
    const outValue: RawZonedDatetime[] = [
      {
        civil: emptyDatetime(),
        offset_seconds: 0,
        offset_known: false,
        fold: false,
        kind: 0,
        offset_before: 0,
        offset_after: 0,
        timestamp: { second: 0, nanosecond: 0 },
      },
    ];
    const outZone: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(
      ctx,
      this.fns.generateZonedDatetime(ctx, tc, tzSpec, zonedSpec, outValue, outZone),
      "hegel_generate_zoned_datetime",
    );
    try {
      const raw = outValue[0];
      return {
        value: {
          civil: raw.civil,
          offsetSeconds: raw.offset_seconds,
          offsetKnown: raw.offset_known,
          fold: raw.fold,
          kind: raw.kind,
          offsetBefore: raw.offset_before,
          offsetAfter: raw.offset_after,
          timestamp: timestampFromRaw(raw.timestamp),
        },
        zone: wtf8ToString(copyNativeBuffer(outZone[0])),
      };
    } finally {
      this.fns.ownedStringFree(outZone[0]);
    }
  }

  /** Create a span spec with no unit enabled; release with {@link freeSpanSpec}. */
  spanSpecNew(ctx: Ptr): SpanSpecHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.spanSpecNew(ctx, out), "hegel_span_spec_new");
    return this.requireHandle<SpanSpecHandle>(out[0], "spanSpecNew");
  }

  spanSpecSetUnitBounds(ctx: Ptr, spec: Ptr, unit: number, min: bigint, max: bigint): void {
    this.check(
      ctx,
      this.fns.spanSpecSetUnitBounds(ctx, spec, unit, min, max),
      "hegel_span_spec_set_unit_bounds",
    );
  }

  spanSpecSetSign(ctx: Ptr, spec: Ptr, sign: number): void {
    this.check(ctx, this.fns.spanSpecSetSign(ctx, spec, sign), "hegel_span_spec_set_sign");
  }

  freeSpanSpec(spec: Ptr): void {
    this.fns.spanSpecFree(spec);
  }

  /** Draw a calendar span within `spec`'s bounds. */
  generateSpan(ctx: Ptr, tc: Ptr, spec: Ptr): NativeSpan {
    const out: RawSpan[] = [
      {
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
      },
    ];
    this.check(ctx, this.fns.generateSpan(ctx, tc, spec, out), "hegel_generate_span");
    const raw = out[0];
    return {
      years: BigInt(raw.years),
      months: BigInt(raw.months),
      weeks: BigInt(raw.weeks),
      days: BigInt(raw.days),
      hours: BigInt(raw.hours),
      minutes: BigInt(raw.minutes),
      seconds: BigInt(raw.seconds),
      milliseconds: BigInt(raw.milliseconds),
      microseconds: BigInt(raw.microseconds),
      nanoseconds: BigInt(raw.nanoseconds),
    };
  }

  /** Draw a UUID as its 16 big-endian bytes. */
  generateUuid(ctx: Ptr, tc: Ptr, version?: UuidVersion): Buffer {
    const out = Buffer.alloc(16);
    this.check(
      ctx,
      this.fns.generateUuid(ctx, tc, version ?? 0, version !== undefined, out),
      "hegel_generate_uuid",
    );
    return out;
  }

  /** Draw an IPv4 address as its 4 network-order bytes. */
  generateIpv4(ctx: Ptr, tc: Ptr): Buffer {
    const out = Buffer.alloc(4);
    this.check(ctx, this.fns.generateIpv4(ctx, tc, out), "hegel_generate_ipv4");
    return out;
  }

  /** Draw an IPv6 address as its 16 network-order bytes. */
  generateIpv6(ctx: Ptr, tc: Ptr): Buffer {
    const out = Buffer.alloc(16);
    this.check(ctx, this.fns.generateIpv6(ctx, tc, out), "hegel_generate_ipv6");
    return out;
  }

  startSpan(ctx: Ptr, tc: Ptr, label: number): void {
    this.check(ctx, this.fns.startSpan(ctx, tc, label), "hegel_start_span");
  }

  stopSpan(ctx: Ptr, tc: Ptr, discard: boolean): void {
    this.check(ctx, this.fns.stopSpan(ctx, tc, discard), "hegel_stop_span");
  }

  /**
   * Open a collection with the given size bounds. The returned handle is owned
   * by the caller — release it with {@link freeCollection}.
   */
  newCollection(ctx: Ptr, tc: Ptr, min: number, max?: number): CollectionHandle {
    const out: Ptr[] = [null];
    const maxArg = max === undefined ? UINT64_MAX : BigInt(max);
    this.check(ctx, this.fns.newCollection(ctx, tc, min, maxArg, out), "hegel_new_collection");
    return this.requireHandle<CollectionHandle>(out[0], "newCollection");
  }

  collectionMore(ctx: Ptr, tc: Ptr, collection: Ptr): boolean {
    const out: boolean[] = [false];
    this.check(ctx, this.fns.collectionMore(ctx, tc, collection, out), "hegel_collection_more");
    return out[0];
  }

  collectionReject(ctx: Ptr, tc: Ptr, collection: Ptr, why: string | null): void {
    this.check(ctx, this.fns.collectionReject(ctx, tc, collection, why), "hegel_collection_reject");
  }

  freeCollection(collection: Ptr): void {
    this.fns.collectionFree(collection);
  }

  markComplete(ctx: Ptr, tc: Ptr, status: number, origin: string | null): void {
    this.check(ctx, this.fns.markComplete(ctx, tc, status, origin), "hegel_mark_complete");
  }

  /**
   * Open a variable pool for stateful testing. The returned handle is owned by
   * the caller — release it with {@link freePool}.
   */
  newPool(ctx: Ptr, tc: Ptr): PoolHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.newPool(ctx, tc, out), "hegel_new_pool");
    return this.requireHandle<PoolHandle>(out[0], "newPool");
  }

  /** Register a new variable in `pool`, returning its engine-assigned id. */
  poolAdd(ctx: Ptr, tc: Ptr, pool: Ptr): bigint {
    const out: (number | bigint)[] = [0];
    this.check(ctx, this.fns.poolAdd(ctx, tc, pool, out), "hegel_pool_add");
    return BigInt(out[0]);
  }

  /**
   * Draw the id of a variable in `pool`, removing it when `consume` is set.
   * Throws {@link AssumeError} when the pool is empty.
   */
  poolGenerate(ctx: Ptr, tc: Ptr, pool: Ptr, consume: boolean): bigint {
    const out: (number | bigint)[] = [0];
    this.check(ctx, this.fns.poolGenerate(ctx, tc, pool, consume, out), "hegel_pool_generate");
    return BigInt(out[0]);
  }

  freePool(pool: Ptr): void {
    this.fns.poolFree(pool);
  }

  /**
   * Register a state machine on `tc`. The returned handle is owned by the
   * caller — release it with {@link freeStateMachine}. The engine's drawn
   * concurrency level is discarded: this client fixes the bounds at 1.
   */
  newStateMachine(ctx: Ptr, tc: Ptr, opts: StateMachineOptions): StateMachineHandle {
    const outMachine: Ptr[] = [null];
    const outConcurrency: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.newStateMachine(ctx, tc, opts, outMachine, outConcurrency),
      "hegel_new_state_machine",
    );
    return this.requireHandle<StateMachineHandle>(outMachine[0], "newStateMachine");
  }

  /**
   * Start the machine's next round, returning the round's group id, or `null`
   * once the machine is finished. koffi hands the `int64_t` back as a `number`
   * when it is safe and a `bigint` only beyond that — which the
   * `HEGEL_STATE_MACHINE_DONE` sentinel always is.
   */
  stateMachineNextGroup(ctx: Ptr, tc: Ptr, machine: Ptr): number | null {
    const out: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.stateMachineNextGroup(ctx, tc, machine, out),
      "hegel_state_machine_next_group",
    );
    return doneOrIndex(out[0]);
  }

  /**
   * Draw the next rule index for `workerIndex` this round, or `null` at the
   * round's join point.
   */
  stateMachineNextRule(ctx: Ptr, tc: Ptr, machine: Ptr, workerIndex: number): number | null {
    const out: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.stateMachineNextRule(ctx, tc, machine, workerIndex, out),
      "hegel_state_machine_next_rule",
    );
    return doneOrIndex(out[0]);
  }

  /** Report the rule last handed to `workerIndex` as rejected (assumption failed). */
  stateMachineRuleRejected(ctx: Ptr, tc: Ptr, machine: Ptr, workerIndex: number): void {
    this.check(
      ctx,
      this.fns.stateMachineRuleRejected(ctx, tc, machine, workerIndex),
      "hegel_state_machine_rule_rejected",
    );
  }

  /** Whether invariant `invariantIndex` should run at the current join point. */
  stateMachineShouldCheckInvariant(
    ctx: Ptr,
    tc: Ptr,
    machine: Ptr,
    invariantIndex: number,
  ): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.stateMachineShouldCheckInvariant(ctx, tc, machine, invariantIndex, out),
      "hegel_state_machine_should_check_invariant",
    );
    return out[0];
  }

  freeStateMachine(machine: Ptr): void {
    this.fns.stateMachineFree(machine);
  }

  runStatus(r: Ptr): number {
    return this.fns.runResultStatus(r);
  }

  runError(r: Ptr): string | null {
    return this.fns.runResultError(r);
  }

  failureCount(r: Ptr): number {
    return this.fns.runResultFailureCount(r);
  }

  /**
   * Read the `index`th failure: a caller-owned copy, released with
   * {@link freeFailure}.
   */
  failure(r: Ptr, index: number): FailureHandle {
    return this.requireHandle<FailureHandle>(
      this.fns.runResultFailure(r, index),
      "hegel_run_result_failure",
    );
  }

  freeFailure(f: Ptr): void {
    this.fns.failureFree(f);
  }

  failureOrigin(fp: Ptr): string {
    return this.fns.failureOrigin(fp);
  }

  /**
   * The failure's base64 reproduce blob, or `null` if the engine produced none.
   * Replay it via {@link testCaseFromBlob} to surface the test's own error.
   */
  reproductionBlob(fp: Ptr): string | null {
    return this.fns.failureReproductionBlob(fp);
  }
}
