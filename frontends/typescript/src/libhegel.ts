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
 * `hegel_run_start_blob`) and — new in this ABI — every test case from
 * `hegel_next_test_case`, `hegel_test_case_clone` and `hegel_test_case_block`,
 * every printer from `hegel_test_case_printer`, the run result from
 * `hegel_run_result`, each failure
 * from `hegel_run_result_failure`, and each collection, pool, recursion and
 * state machine from `hegel_new_collection` / `hegel_new_pool` /
 * `hegel_new_recursion` / `hegel_new_state_machine`
 * (the runner releases them all in `finally` blocks).
 * String generators are cached within a run and freed when that run ends.
 *
 * @packageDocumentation
 */

import {
  EngineError,
  UINT64_MAX,
  doneOrIndex,
  type Engine,
  type ContextHandle,
  type SettingsHandle,
  type RunHandle,
  type RunResultHandle,
  type TestCaseHandle,
  type FailureHandle,
  type CollectionHandle,
  type RepeatHandle,
  type StringGeneratorHandle,
  type PoolHandle,
  type RecursionHandle,
  type StateMachineHandle,
  type PrinterHandle,
  type PrinterOptionsHandle,
  type GrammarBuilderHandle,
  type GrammarHandle,
  type GrammarDrawHandle,
  type GrammarOptions,
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
  type ZoneSourceHandle,
  type NativeZoneData,
  type NativeZoneType,
  type NativeZoneRuleDate,
  type TextGeneratorOptions,
  type NativeFloatOptions,
  type NativeTestLocation,
  type NativeRunErrorKind,
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

/**
 * A koffi pointer, or a handle {@link Libhegel} wraps one in: `Libhegel`
 * hands out a fresh object per handle and unwraps it on every call, so a
 * freed or foreign handle throws instead of reaching the engine.
 */
export type Ptr = unknown;

/** Relevant `hegel_result_t` codes. */
const RESULT_OK = 0;
const RESULT_STOP_TEST = -1;
const RESULT_ASSUME = -2;
const RESULT_RETRY = -10;
const RESULT_NOT_FOUND = -11;

const INVALID_HANDLE = "Foreign, freed or invalid engine handle";

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

const zoneTypeType: TypeObject = koffi.struct({
  utoff: "int32_t",
  is_dst: "bool",
  abbreviation: stringType,
});
const zoneTransitionType: TypeObject = koffi.struct({
  instant: "int64_t",
  local_type: zoneTypeType,
});
const zoneRuleDateType: TypeObject = koffi.struct({
  kind: "uint32_t",
  day: "uint16_t",
  month: "uint8_t",
  week: "uint8_t",
  weekday: "uint8_t",
  time: "int32_t",
});
const zoneDataType: TypeObject = koffi.struct({
  initial: zoneTypeType,
  transitions: koffi.pointer(zoneTransitionType),
  transition_count: "size_t",
  after: "uint32_t",
  standard: zoneTypeType,
  has_dst: "bool",
  dst: zoneTypeType,
  dst_start: zoneRuleDateType,
  dst_end: zoneRuleDateType,
  last_year: "int32_t",
});

/** A `hegel_zone_type_t` as koffi marshals it, with the C field names. */
interface RawZoneType {
  utoff: number;
  is_dst: boolean;
  abbreviation: NativeBuffer;
}

/** A `hegel_zone_rule_date_t` as koffi marshals it. */
type RawZoneRuleDate = NativeZoneRuleDate;

/** A `hegel_zone_data_t` as koffi marshals it, with the C field names. */
interface RawZoneData {
  initial: RawZoneType;
  transitions: { instant: bigint; local_type: RawZoneType }[];
  transition_count: number;
  after: number;
  standard: RawZoneType;
  has_dst: boolean;
  dst: RawZoneType;
  dst_start: RawZoneRuleDate;
  dst_end: RawZoneRuleDate;
  last_year: number;
}

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
 * {@link bindLibrary}. `hegel_settings_new` and
 * `hegel_settings_new_for_profile` stay fallible: they resolve a settings
 * profile, which fails when the profile is unknown or a `hegel.toml` is
 * malformed. The output callback taken by
 * `hegel_run_start` / `hegel_run_start_blob` is likewise absorbed as NULL
 * (engine output stays on stderr), and so are the options of
 * `hegel_test_case_printer` and `hegel_printer_reflow` (the defaults: a
 * 79-column document).
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
  setUnicodeVersion: (ctx: Ptr, version: string) => void;

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
  settingsNewForProfile: (ctx: Ptr, name: string, out: Ptr[]) => number;
  settingsStopOnFirstFailure: (s: Ptr, yes: boolean) => void;
  settingsShowStatistics: (s: Ptr, yes: boolean) => void;
  settingsUnboundedChoices: (s: Ptr, yes: boolean) => void;
  settingsPrintBlob: (s: Ptr, yes: boolean) => void;
  settingsPhases: (s: Ptr, phases: number) => void;
  settingsBackend: (s: Ptr, backend: number) => void;
  settingsNondeterminismStrictness: (s: Ptr, strictness: number) => void;
  settingsTestLocation: (ctx: Ptr, s: Ptr, location: NativeTestLocation) => void;
  settingsRegisterProfile: (ctx: Ptr, name: string, s: Ptr) => void;
  setDefaultProfile: (ctx: Ptr, name: string) => void;
  settingsGetVerbosity: (s: Ptr) => number;
  settingsGetPhases: (s: Ptr) => number;
  settingsGetPrintBlob: (s: Ptr) => boolean;
  settingsGetReportMultipleFailures: (s: Ptr) => boolean;
  settingsGetTestCases: (s: Ptr) => number;
  settingsGetSeed: (s: Ptr) => bigint | null;
  settingsGetDerandomize: (s: Ptr) => boolean;
  settingsGetDatabase: (s: Ptr) => string | null;
  settingsGetSuppressHealthCheck: (s: Ptr) => number;
  settingsGetStopOnFirstFailure: (s: Ptr) => boolean;
  settingsGetShowStatistics: (s: Ptr) => boolean;
  settingsGetUnboundedChoices: (s: Ptr) => boolean;
  settingsGetBackend: (s: Ptr) => number;
  settingsGetNondeterminismStrictness: (s: Ptr) => number;

  runStart: (ctx: Ptr, settings: Ptr, out: Ptr[]) => number;
  runStartBlob: (ctx: Ptr, settings: Ptr, blob: string, out: Ptr[]) => number;
  nextTestCase: (ctx: Ptr, run: Ptr, out: Ptr[]) => number;
  runResult: (ctx: Ptr, run: Ptr, out: Ptr[]) => number;
  runResultFree: (r: Ptr) => void;
  runFree: (run: Ptr) => void;

  testCaseFree: (tc: Ptr) => void;
  testCaseShouldCapture: (ctx: Ptr, tc: Ptr, out: boolean[]) => number;
  testCaseClone: (ctx: Ptr, tc: Ptr, out: Ptr[]) => number;
  testCaseBlock: (ctx: Ptr, tc: Ptr, indent: number, out: Ptr[]) => number;
  testCaseSetWorker: (ctx: Ptr, tc: Ptr, index: number) => number;
  note: (ctx: Ptr, tc: Ptr, text: string) => number;
  target: (ctx: Ptr, tc: Ptr, value: number, label: string) => number;
  event: (ctx: Ptr, tc: Ptr, label: string) => number;
  eventValue: (ctx: Ptr, tc: Ptr, value: number, label: string) => number;
  labelFromName: (ctx: Ptr, name: string, out: (number | bigint)[]) => number;
  labelCombine: (ctx: Ptr, labels: readonly bigint[], out: (number | bigint)[]) => number;

  testCasePrinter: (ctx: Ptr, tc: Ptr, out: Ptr[]) => number;
  printerOptionsNew: (ctx: Ptr, out: Ptr[]) => number;
  printerOptionsSetMaxWidth: (ctx: Ptr, options: Ptr, width: bigint) => number;
  printerOptionsFree: (options: Ptr) => void;
  printerNew: (ctx: Ptr, options: Ptr | null, out: Ptr[]) => number;
  printerFree: (p: Ptr) => void;
  printerText: (ctx: Ptr, p: Ptr, text: string) => number;
  printerReflow: (ctx: Ptr, p: Ptr, text: string) => number;
  printerHardBreak: (ctx: Ptr, p: Ptr) => number;
  printerBeginGroup: (ctx: Ptr, p: Ptr, indent: bigint, open: string) => number;
  printerEndGroup: (ctx: Ptr, p: Ptr, close: string) => number;
  printerBreakable: (ctx: Ptr, p: Ptr, sep: string) => number;
  printerIfBreak: (ctx: Ptr, p: Ptr, text: string) => number;
  printerComment: (ctx: Ptr, p: Ptr, text: string) => number;
  printerShiftIndent: (ctx: Ptr, p: Ptr, delta: bigint) => number;
  printerBeginSpeculative: (ctx: Ptr, p: Ptr) => number;
  printerCommitSpeculative: (ctx: Ptr, p: Ptr) => number;
  printerAbortSpeculative: (ctx: Ptr, p: Ptr) => number;
  printerDeferred: (ctx: Ptr, p: Ptr, out: Ptr[]) => number;
  printerIsLive: (ctx: Ptr, p: Ptr, out: boolean[]) => number;
  printerResolve: (ctx: Ptr, p: Ptr) => number;
  printerValue: (ctx: Ptr, p: Ptr, out: NativeBuffer[]) => number;
  printerValueResultFree: (result: NativeBuffer) => void;

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
  stringGeneratorRegex: (
    ctx: Ptr,
    pattern: string,
    fullmatch: boolean,
    alphabet: Ptr | null,
    out: Ptr[],
  ) => number;
  stringGeneratorEmail: (ctx: Ptr, out: Ptr[]) => number;
  stringGeneratorUrl: (ctx: Ptr, out: Ptr[]) => number;
  stringGeneratorDomain: (ctx: Ptr, maxLength: number, out: Ptr[]) => number;
  generateString: (ctx: Ptr, tc: Ptr, generator: Ptr, out: NativeBuffer[]) => number;
  ownedStringFree: (result: NativeBuffer) => void;

  grammarBuilderNew: (ctx: Ptr, text: string, out: Ptr[]) => number;
  grammarBuilderFile: (ctx: Ptr, builder: Ptr, index: number, out: NativeBuffer[]) => number;
  grammarBuilderProvide: (ctx: Ptr, builder: Ptr, index: number, text: string | null) => number;
  grammarBuilderFree: (builder: Ptr) => void;
  grammarNew: (ctx: Ptr, builder: Ptr, opts: GrammarOptions, out: Ptr[]) => number;
  grammarFree: (grammar: Ptr) => void;
  grammarDrawNew: (ctx: Ptr, grammar: Ptr, maxSize: bigint, out: Ptr[]) => number;
  grammarDrawNext: (ctx: Ptr, tc: Ptr, draw: Ptr, out: (number | bigint)[]) => number;
  grammarDrawProvide: (ctx: Ptr, draw: Ptr, value: string) => number;
  grammarDrawResult: (ctx: Ptr, draw: Ptr, out: NativeBuffer[]) => number;
  grammarDrawFree: (draw: Ptr) => void;

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
  timezoneSpecSetSource: (ctx: Ptr, spec: Ptr, source: Ptr) => number;
  timezoneSpecSetLinks: (ctx: Ptr, spec: Ptr, links: boolean) => number;
  timezoneSpecSetLegacy: (ctx: Ptr, spec: Ptr, legacy: boolean) => number;
  zoneSourceNew: (ctx: Ptr, out: Ptr[]) => number;
  zoneSourceAddTzif: (ctx: Ptr, source: Ptr, name: string, bytes: Uint8Array) => number;
  zoneSourceAddZone: (ctx: Ptr, source: Ptr, name: string, data: NativeZoneData) => number;
  zoneSourceAddLink: (ctx: Ptr, source: Ptr, name: string, target: string) => number;
  zoneSourceDeclare: (ctx: Ptr, source: Ptr, names: readonly string[]) => number;
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

  startSpan: (ctx: Ptr, tc: Ptr, label: bigint) => number;
  stopSpan: (ctx: Ptr, tc: Ptr, discard: boolean) => number;
  newCollection: (ctx: Ptr, tc: Ptr, min: number, max: bigint, out: Ptr[]) => number;
  collectionMore: (ctx: Ptr, tc: Ptr, collection: Ptr, out: boolean[]) => number;
  collectionReject: (ctx: Ptr, tc: Ptr, collection: Ptr, why: string | null) => number;
  collectionFree: (collection: Ptr) => void;
  newRepeat: (ctx: Ptr, tc: Ptr, out: Ptr[]) => number;
  repeatMore: (ctx: Ptr, tc: Ptr, repeat: Ptr, out: boolean[]) => number;
  repeatFailed: (ctx: Ptr, tc: Ptr, repeat: Ptr) => number;
  repeatFree: (repeat: Ptr) => void;
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
  poolRestore: (ctx: Ptr, tc: Ptr, pool: Ptr, variableId: bigint) => number;
  poolFree: (pool: Ptr) => void;

  newRecursion: (ctx: Ptr, tc: Ptr, maxDepth: bigint, maxLeaves: bigint, out: Ptr[]) => number;
  recursionBranch: (ctx: Ptr, tc: Ptr, recursion: Ptr, depth: bigint, out: boolean[]) => number;
  recursionLeaf: (ctx: Ptr, tc: Ptr, recursion: Ptr) => number;
  recursionRetry: (ctx: Ptr, tc: Ptr, recursion: Ptr) => number;
  recursionFinish: (ctx: Ptr, tc: Ptr, recursion: Ptr) => number;
  recursionFree: (recursion: Ptr) => void;

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
  runResultErrorKind: (r: Ptr) => NativeRunErrorKind;
  runResultFailureCount: (r: Ptr) => number;
  runResultFailure: (r: Ptr, index: number) => Ptr;
  failureFree: (f: Ptr) => void;
  failureOrigin: (f: Ptr) => string;
  failureReproductionBlob: (f: Ptr) => string | null;
  failureCaveat: (f: Ptr) => string | null;

  version: () => string;
  tzdbVersion: () => string;
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
  const setUnicodeVersion = fs("hegel_set_unicode_version", ["void*", stringType]);

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
  const settingsNewForProfile = fs("hegel_settings_new_for_profile", [
    "void*",
    stringType,
    koffi.out(koffi.pointer("void*")),
  ]);
  const settingsStopOnFirstFailure = f(
    "int hegel_settings_set_stop_on_first_failure(void* ctx, void* s, bool yes)",
  );
  const settingsShowStatistics = f(
    "int hegel_settings_set_show_statistics(void* ctx, void* s, bool yes)",
  );
  const settingsUnboundedChoices = f(
    "int hegel_settings_set_unbounded_choices(void* ctx, void* s, bool yes)",
  );
  const settingsPrintBlob = f("int hegel_settings_set_print_blob(void* ctx, void* s, bool yes)");
  const settingsPhases = f("int hegel_settings_set_phases(void* ctx, void* s, uint32_t phases)");
  const settingsBackend = f("int hegel_settings_set_backend(void* ctx, void* s, uint32_t backend)");
  const settingsNondeterminismStrictness = f(
    "int hegel_settings_set_nondeterminism_strictness(void* ctx, void* s, uint32_t strictness)",
  );
  const settingsTestLocation = fs("hegel_settings_set_test_location", [
    "void*",
    "void*",
    stringType,
    "uint32_t",
    stringType,
    stringType,
  ]);
  const settingsRegisterProfile = fs("hegel_settings_register_profile", [
    "void*",
    stringType,
    "void*",
  ]);
  const setDefaultProfile = fs("hegel_set_default_profile", ["void*", stringType]);

  const settingsGetVerbosity = f(
    "int hegel_settings_get_verbosity(void* ctx, void* s, _Out_ uint32_t* out)",
  );
  const settingsGetPhases = f(
    "int hegel_settings_get_phases(void* ctx, void* s, _Out_ uint32_t* out)",
  );
  const settingsGetPrintBlob = f(
    "int hegel_settings_get_print_blob(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetReportMultipleFailures = f(
    "int hegel_settings_get_report_multiple_failures(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetTestCases = f(
    "int hegel_settings_get_test_cases(void* ctx, void* s, _Out_ uint64_t* out)",
  );
  const settingsGetSeed = f(
    "int hegel_settings_get_seed(void* ctx, void* s, _Out_ uint64_t* out_seed, _Out_ bool* out_has_seed)",
  );
  const settingsGetDerandomize = f(
    "int hegel_settings_get_derandomize(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetDatabase = f(
    "int hegel_settings_get_database(void* ctx, void* s, _Out_ void** out_database)",
  );
  const settingsGetSuppressHealthCheck = f(
    "int hegel_settings_get_suppress_health_check(void* ctx, void* s, _Out_ uint32_t* out)",
  );
  const settingsGetStopOnFirstFailure = f(
    "int hegel_settings_get_stop_on_first_failure(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetShowStatistics = f(
    "int hegel_settings_get_show_statistics(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetUnboundedChoices = f(
    "int hegel_settings_get_unbounded_choices(void* ctx, void* s, _Out_ bool* out)",
  );
  const settingsGetBackend = f(
    "int hegel_settings_get_backend(void* ctx, void* s, _Out_ uint32_t* out)",
  );
  const settingsGetNondeterminismStrictness = f(
    "int hegel_settings_get_nondeterminism_strictness(void* ctx, void* s, _Out_ uint32_t* out)",
  );

  const runStart = f(
    "int hegel_run_start(void* ctx, void* settings, void* callback, void* user_data, _Out_ void** out_run)",
  );
  const runStartBlob = fs("hegel_run_start_blob", [
    "void*",
    "void*",
    stringType,
    "void*",
    "void*",
    koffi.out(koffi.pointer("void*")),
  ]);
  const nextTestCase = f("int hegel_next_test_case(void* ctx, void* run, _Out_ void** out_tc)");
  const runResult = f("int hegel_run_result(void* ctx, void* run, _Out_ void** out_result)");
  const runResultFree = f("int hegel_run_result_free(void* ctx, void* r)");
  const runFree = f("int hegel_run_free(void* ctx, void* run)");

  const testCaseFree = f("int hegel_test_case_free(void* ctx, void* tc)");
  const testCaseShouldCapture = f(
    "int hegel_test_case_should_capture(void* ctx, void* tc, _Out_ bool* out)",
  );
  const testCaseClone = f("int hegel_test_case_clone(void* ctx, void* tc, _Out_ void** out)");
  const testCaseBlock = f(
    "int hegel_test_case_block(void* ctx, void* tc, uint64_t indent, _Out_ void** out)",
  );
  const testCaseSetWorker = f(
    "int hegel_test_case_set_worker(void* ctx, void* tc, int64_t worker_index)",
  );
  const note = fs("hegel_note", ["void*", "void*", stringType]);
  const target = fs("hegel_target", ["void*", "void*", "double", stringType]);
  const event = fs("hegel_event", ["void*", "void*", stringType]);
  const eventValue = fs("hegel_event_value", ["void*", "void*", "double", stringType]);
  const labelFromName = fs("hegel_label_from_name", [
    "void*",
    stringType,
    koffi.out(koffi.pointer("uint64_t")),
  ]);
  const labelCombine = f(
    "int hegel_label_combine(void* ctx, const uint64_t* labels, size_t len, _Out_ uint64_t* out)",
  );

  const testCasePrinter = f(
    "int hegel_test_case_printer(void* ctx, void* tc, void* options, _Out_ void** out)",
  );
  const printerOptionsNew = f("int hegel_printer_options_new(void* ctx, _Out_ void** out)");
  const printerOptionsSetMaxWidth = f(
    "int hegel_printer_options_set_max_width(void* ctx, void* options, uint64_t max_width)",
  );
  const printerOptionsFree = f("int hegel_printer_options_free(void* ctx, void* options)");
  const printerNew = f("int hegel_printer_new(void* ctx, void* options, _Out_ void** out)");
  const printerFree = f("int hegel_printer_free(void* ctx, void* printer)");
  const printerText = fs("hegel_printer_text", ["void*", "void*", stringType]);
  const printerReflow = fs("hegel_printer_reflow", ["void*", "void*", "void*", stringType]);
  const printerHardBreak = f("int hegel_printer_hard_break(void* ctx, void* printer)");
  const printerBeginGroup = fs("hegel_printer_begin_group", [
    "void*",
    "void*",
    "uint64_t",
    stringType,
  ]);
  const printerEndGroup = fs("hegel_printer_end_group", ["void*", "void*", stringType]);
  const printerBreakable = fs("hegel_printer_breakable", ["void*", "void*", stringType]);
  const printerIfBreak = fs("hegel_printer_if_break", ["void*", "void*", stringType]);
  const printerComment = fs("hegel_printer_comment", ["void*", "void*", stringType]);
  const printerShiftIndent = f(
    "int hegel_printer_shift_indent(void* ctx, void* printer, int64_t delta)",
  );
  const printerBeginSpeculative = f(
    "int hegel_printer_begin_speculative(void* ctx, void* printer)",
  );
  const printerCommitSpeculative = f(
    "int hegel_printer_commit_speculative(void* ctx, void* printer)",
  );
  const printerAbortSpeculative = f(
    "int hegel_printer_abort_speculative(void* ctx, void* printer)",
  );
  const printerDeferred = f(
    "int hegel_printer_deferred(void* ctx, void* printer, _Out_ void** out)",
  );
  const printerIsLive = f("int hegel_printer_is_live(void* ctx, void* printer, _Out_ bool* out)");
  const printerResolve = f("int hegel_printer_resolve(void* ctx, void* printer)");
  const printerValue = fs("hegel_printer_value", [
    "void*",
    "void*",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const printerValueResultFree = fs("hegel_printer_value_result_free", [
    "void*",
    koffi.pointer(stringType),
  ]);

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

  const grammarBuilderNew = fs("hegel_grammar_builder_new", [
    "void*",
    stringType,
    koffi.out(koffi.pointer("void*")),
  ]);
  const grammarBuilderFile = fs("hegel_grammar_builder_file", [
    "void*",
    "void*",
    "size_t",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const grammarBuilderProvide = fs("hegel_grammar_builder_provide", [
    "void*",
    "void*",
    "size_t",
    koffi.pointer(stringType),
  ]);
  const grammarBuilderFree = f("int hegel_grammar_builder_free(void* ctx, void* builder)");
  const grammarNew = fs("hegel_grammar_new", [
    "void*",
    "void*",
    koffi.pointer(stringType),
    "uint64_t",
    "uint32_t",
    "void*",
    koffi.pointer(stringType),
    "size_t",
    koffi.out(koffi.pointer("void*")),
  ]);
  const grammarFree = f("int hegel_grammar_free(void* ctx, void* grammar)");
  const grammarDrawNew = f(
    "int hegel_grammar_draw_new(void* ctx, void* grammar, uint64_t max_size, _Out_ void** out_draw)",
  );
  const grammarDrawNext = f(
    "int hegel_grammar_draw_next(void* ctx, void* tc, void* draw, _Out_ size_t* out_terminal)",
  );
  const grammarDrawProvide = fs("hegel_grammar_draw_provide", ["void*", "void*", stringType]);
  const grammarDrawResult = fs("hegel_grammar_draw_result", [
    "void*",
    "void*",
    koffi.out(koffi.pointer(stringType)),
  ]);
  const grammarDrawFree = f("int hegel_grammar_draw_free(void* ctx, void* draw)");

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
  const timezoneSpecSetSource = f(
    "int hegel_timezone_spec_set_source(void* ctx, void* spec, void* source)",
  );
  const timezoneSpecSetLinks = f(
    "int hegel_timezone_spec_set_links(void* ctx, void* spec, bool links)",
  );
  const timezoneSpecSetLegacy = f(
    "int hegel_timezone_spec_set_legacy(void* ctx, void* spec, bool legacy)",
  );
  const zoneSourceNew = f("int hegel_zone_source_new(void* ctx, _Out_ void** out)");
  const zoneSourceAddTzif = fs("hegel_zone_source_add_tzif", [
    "void*",
    "void*",
    stringType,
    "uint8_t*",
    "size_t",
  ]);
  const zoneSourceAddZone = fs("hegel_zone_source_add_zone", [
    "void*",
    "void*",
    stringType,
    koffi.pointer(zoneDataType),
  ]);
  const zoneSourceAddLink = fs("hegel_zone_source_add_link", [
    "void*",
    "void*",
    stringType,
    stringType,
  ]);
  const zoneSourceDeclare = fs("hegel_zone_source_declare", [
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
  const newRepeat = f("int hegel_new_repeat(void* ctx, void* tc, _Out_ void** out_repeat)");
  const repeatMore = f("int hegel_repeat_more(void* ctx, void* tc, void* repeat, _Out_ bool* out)");
  const repeatFailed = f("int hegel_repeat_failed(void* ctx, void* tc, void* repeat)");
  const repeatFree = f("int hegel_repeat_free(void* ctx, void* repeat)");
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
  const poolRestore = f(
    "int hegel_pool_restore(void* ctx, void* tc, void* pool, int64_t variable_id)",
  );
  const poolFree = f("int hegel_pool_free(void* ctx, void* pool)");

  const newRecursion = f(
    "int hegel_new_recursion(void* ctx, void* tc, uint64_t max_depth, uint64_t max_leaves, _Out_ void** out_recursion)",
  );
  const recursionBranch = f(
    "int hegel_recursion_branch(void* ctx, void* tc, void* recursion, uint64_t depth, _Out_ bool* out_branch)",
  );
  const recursionLeaf = f("int hegel_recursion_leaf(void* ctx, void* tc, void* recursion)");
  const recursionRetry = f("int hegel_recursion_retry(void* ctx, void* tc, void* recursion)");
  const recursionFinish = f("int hegel_recursion_finish(void* ctx, void* tc, void* recursion)");
  const recursionFree = f("int hegel_recursion_free(void* ctx, void* recursion)");

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
  const runResultErrorKind = f(
    "int hegel_run_result_error_kind(void* ctx, void* r, _Out_ uint32_t* kind, _Out_ uint32_t* health_check)",
  );
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
  const failureCaveat = f("int hegel_failure_caveat(void* ctx, void* f, _Out_ void** out)");
  const version = fs("hegel_version", ["void*", koffi.out(koffi.pointer(stringType))]);
  const tzdbVersion = fs("hegel_tzdb_version", ["void*", koffi.out(koffi.pointer(stringType))]);

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

  const rawZoneType = (type: NativeZoneType): RawZoneType => ({
    utoff: type.utoff,
    is_dst: type.isDst,
    abbreviation: nativeString(type.abbreviation),
  });
  const rawZoneData = (data: NativeZoneData): RawZoneData => ({
    initial: rawZoneType(data.initial),
    transitions: data.transitions.map((t) => ({
      instant: t.instant,
      local_type: rawZoneType(t.type),
    })),
    transition_count: data.transitions.length,
    after: data.after,
    standard: rawZoneType(data.standard),
    has_dst: data.hasDst,
    dst: rawZoneType(data.dst),
    dst_start: data.dstStart,
    dst_end: data.dstEnd,
    last_year: data.lastYear,
  });

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
    setUnicodeVersion: (ctx, version) =>
      checked(setUnicodeVersion(ctx, nativeString(version)), "setUnicodeVersion", ctx),
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
    settingsNewForProfile: (ctx, name, out) => settingsNewForProfile(ctx, nativeString(name), out),
    settingsStopOnFirstFailure: (s, yes) =>
      checked(settingsStopOnFirstFailure(null, s, yes), "settingsStopOnFirstFailure"),
    settingsShowStatistics: (s, yes) =>
      checked(settingsShowStatistics(null, s, yes), "settingsShowStatistics"),
    settingsUnboundedChoices: (s, yes) =>
      checked(settingsUnboundedChoices(null, s, yes), "settingsUnboundedChoices"),
    settingsPrintBlob: (s, yes) => checked(settingsPrintBlob(null, s, yes), "settingsPrintBlob"),
    settingsPhases: (s, phases) => checked(settingsPhases(null, s, phases), "settingsPhases"),
    settingsBackend: (s, backend) => checked(settingsBackend(null, s, backend), "settingsBackend"),
    settingsNondeterminismStrictness: (s, strictness) =>
      checked(
        settingsNondeterminismStrictness(null, s, strictness),
        "settingsNondeterminismStrictness",
      ),
    settingsTestLocation: (ctx, s, location) =>
      checked(
        settingsTestLocation(
          ctx,
          s,
          nativeString(location.file),
          location.beginLine,
          nativeString(location.className),
          nativeString(location.function),
        ),
        "settingsTestLocation",
        ctx,
      ),
    settingsRegisterProfile: (ctx, name, s) =>
      checked(settingsRegisterProfile(ctx, nativeString(name), s), "settingsRegisterProfile", ctx),
    setDefaultProfile: (ctx, name) =>
      checked(setDefaultProfile(ctx, nativeString(name)), "setDefaultProfile", ctx),
    settingsGetVerbosity: (s) => {
      const out: number[] = [0];
      checked(settingsGetVerbosity(null, s, out), "settingsGetVerbosity");
      return out[0];
    },
    settingsGetPhases: (s) => {
      const out: number[] = [0];
      checked(settingsGetPhases(null, s, out), "settingsGetPhases");
      return out[0];
    },
    settingsGetPrintBlob: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetPrintBlob(null, s, out), "settingsGetPrintBlob");
      return out[0];
    },
    settingsGetReportMultipleFailures: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetReportMultipleFailures(null, s, out), "settingsGetReportMultipleFailures");
      return out[0];
    },
    settingsGetTestCases: (s) => {
      const out: (number | bigint)[] = [0];
      checked(settingsGetTestCases(null, s, out), "settingsGetTestCases");
      return Number(out[0]);
    },
    settingsGetSeed: (s) => {
      const seed: (number | bigint)[] = [0];
      const hasSeed: boolean[] = [false];
      checked(settingsGetSeed(null, s, seed, hasSeed), "settingsGetSeed");
      return hasSeed[0] ? BigInt(seed[0]) : null;
    },
    settingsGetDerandomize: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetDerandomize(null, s, out), "settingsGetDerandomize");
      return out[0];
    },
    settingsGetDatabase: (s) => {
      const out: Ptr[] = [null];
      checked(settingsGetDatabase(null, s, out), "settingsGetDatabase");
      return readOptionalString(out[0]);
    },
    settingsGetSuppressHealthCheck: (s) => {
      const out: number[] = [0];
      checked(settingsGetSuppressHealthCheck(null, s, out), "settingsGetSuppressHealthCheck");
      return out[0];
    },
    settingsGetStopOnFirstFailure: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetStopOnFirstFailure(null, s, out), "settingsGetStopOnFirstFailure");
      return out[0];
    },
    settingsGetShowStatistics: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetShowStatistics(null, s, out), "settingsGetShowStatistics");
      return out[0];
    },
    settingsGetUnboundedChoices: (s) => {
      const out: boolean[] = [false];
      checked(settingsGetUnboundedChoices(null, s, out), "settingsGetUnboundedChoices");
      return out[0];
    },
    settingsGetBackend: (s) => {
      const out: number[] = [0];
      checked(settingsGetBackend(null, s, out), "settingsGetBackend");
      return out[0];
    },
    settingsGetNondeterminismStrictness: (s) => {
      const out: number[] = [0];
      checked(
        settingsGetNondeterminismStrictness(null, s, out),
        "settingsGetNondeterminismStrictness",
      );
      return out[0];
    },
    runStart: (ctx, s, out) => runStart(ctx, s, null, null, out),
    runStartBlob: (ctx, s, blob, out) => runStartBlob(ctx, s, nativeString(blob), null, null, out),
    nextTestCase: (ctx, run, out) => nextTestCase(ctx, run, out),
    runResult: (ctx, run, out) => runResult(ctx, run, out),
    runResultFree: (r) => checked(runResultFree(null, r), "runResultFree"),
    runFree: (run) => checked(runFree(null, run), "runFree"),
    testCaseFree: (tc) => checked(testCaseFree(null, tc), "testCaseFree"),
    testCaseShouldCapture: (ctx, tc, out) => testCaseShouldCapture(ctx, tc, out),
    testCaseClone: (ctx, tc, out) => testCaseClone(ctx, tc, out),
    testCaseBlock: (ctx, tc, indent, out) => testCaseBlock(ctx, tc, indent, out),
    testCaseSetWorker: (ctx, tc, index) => testCaseSetWorker(ctx, tc, index),
    note: (ctx, tc, text) => note(ctx, tc, nativeString(text)),
    target: (ctx, tc, value, label) => target(ctx, tc, value, nativeString(label)),
    event: (ctx, tc, label) => event(ctx, tc, nativeString(label)),
    eventValue: (ctx, tc, value, label) => eventValue(ctx, tc, value, nativeString(label)),
    labelFromName: (ctx, name, out) => labelFromName(ctx, nativeString(name), out),
    labelCombine: (ctx, labels, out) => labelCombine(ctx, [...labels], labels.length, out),
    testCasePrinter: (ctx, tc, out) => testCasePrinter(ctx, tc, null, out),
    printerOptionsNew: (ctx, out) => printerOptionsNew(ctx, out),
    printerOptionsSetMaxWidth: (ctx, options, width) =>
      printerOptionsSetMaxWidth(ctx, options, width),
    printerOptionsFree: (options) =>
      checked(printerOptionsFree(null, options), "printerOptionsFree"),
    printerNew: (ctx, options, out) => printerNew(ctx, options, out),
    printerFree: (p) => checked(printerFree(null, p), "printerFree"),
    printerText: (ctx, p, text) => printerText(ctx, p, nativeString(text)),
    printerReflow: (ctx, p, text) => printerReflow(ctx, p, null, nativeString(text)),
    printerHardBreak: (ctx, p) => printerHardBreak(ctx, p),
    printerBeginGroup: (ctx, p, indent, open) =>
      printerBeginGroup(ctx, p, indent, nativeString(open)),
    printerEndGroup: (ctx, p, close) => printerEndGroup(ctx, p, nativeString(close)),
    printerBreakable: (ctx, p, sep) => printerBreakable(ctx, p, nativeString(sep)),
    printerIfBreak: (ctx, p, text) => printerIfBreak(ctx, p, nativeString(text)),
    printerComment: (ctx, p, text) => printerComment(ctx, p, nativeString(text)),
    printerShiftIndent: (ctx, p, delta) => printerShiftIndent(ctx, p, delta),
    printerBeginSpeculative: (ctx, p) => printerBeginSpeculative(ctx, p),
    printerCommitSpeculative: (ctx, p) => printerCommitSpeculative(ctx, p),
    printerAbortSpeculative: (ctx, p) => printerAbortSpeculative(ctx, p),
    printerDeferred: (ctx, p, out) => printerDeferred(ctx, p, out),
    printerIsLive: (ctx, p, out) => printerIsLive(ctx, p, out),
    printerResolve: (ctx, p) => printerResolve(ctx, p),
    printerValue: (ctx, p, out) => printerValue(ctx, p, out),
    printerValueResultFree: (result) =>
      checked(printerValueResultFree(null, result), "printerValueResultFree"),
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
    stringGeneratorRegex: (ctx, pattern, fullmatch, alphabet, out) =>
      stringGeneratorRegex(ctx, nativeString(pattern), fullmatch, alphabet, out),
    stringGeneratorEmail: (ctx, out) => stringGeneratorEmail(ctx, out),
    stringGeneratorUrl: (ctx, out) => stringGeneratorUrl(ctx, out),
    stringGeneratorDomain: (ctx, maxLength, out) => stringGeneratorDomain(ctx, maxLength, out),
    generateString: (ctx, tc, generator, out) => generateString(ctx, tc, generator, out),
    ownedStringFree: (result) => checked(ownedStringFree(null, result), "ownedStringFree"),
    grammarBuilderNew: (ctx, text, out) => grammarBuilderNew(ctx, nativeString(text), out),
    grammarBuilderFile: (ctx, builder, index, out) => grammarBuilderFile(ctx, builder, index, out),
    grammarBuilderProvide: (ctx, builder, index, text) =>
      grammarBuilderProvide(ctx, builder, index, optionalString(text)),
    grammarBuilderFree: (builder) =>
      checked(grammarBuilderFree(null, builder), "hegel_grammar_builder_free"),
    grammarNew: (ctx, builder, opts, out) =>
      grammarNew(
        ctx,
        builder,
        optionalString(opts.start),
        BigInt(opts.maxSize),
        opts.maxDepth,
        opts.alphabet,
        opts.explicitNames.map(nativeString),
        opts.explicitNames.length,
        out,
      ),
    grammarFree: (grammar) => checked(grammarFree(null, grammar), "hegel_grammar_free"),
    grammarDrawNew: (ctx, grammar, maxSize, out) => grammarDrawNew(ctx, grammar, maxSize, out),
    grammarDrawNext: (ctx, tc, draw, out) => grammarDrawNext(ctx, tc, draw, out),
    grammarDrawProvide: (ctx, draw, value) => grammarDrawProvide(ctx, draw, nativeString(value)),
    grammarDrawResult: (ctx, draw, out) => grammarDrawResult(ctx, draw, out),
    grammarDrawFree: (draw) => checked(grammarDrawFree(null, draw), "hegel_grammar_draw_free"),
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
    timezoneSpecSetSource: (ctx, spec, source) => timezoneSpecSetSource(ctx, spec, source),
    timezoneSpecSetLinks: (ctx, spec, links) => timezoneSpecSetLinks(ctx, spec, links),
    timezoneSpecSetLegacy: (ctx, spec, legacy) => timezoneSpecSetLegacy(ctx, spec, legacy),
    zoneSourceNew: (ctx, out) => zoneSourceNew(ctx, out),
    zoneSourceAddTzif: (ctx, source, name, bytes) =>
      zoneSourceAddTzif(ctx, source, nativeString(name), bytes, bytes.length),
    zoneSourceAddZone: (ctx, source, name, data) =>
      zoneSourceAddZone(ctx, source, nativeString(name), rawZoneData(data)),
    zoneSourceAddLink: (ctx, source, name, target) =>
      zoneSourceAddLink(ctx, source, nativeString(name), nativeString(target)),
    zoneSourceDeclare: (ctx, source, names) =>
      zoneSourceDeclare(ctx, source, names.map(nativeString), names.length),
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
    newRepeat: (ctx, tc, out) => newRepeat(ctx, tc, out),
    repeatMore: (ctx, tc, repeat, out) => repeatMore(ctx, tc, repeat, out),
    repeatFailed: (ctx, tc, repeat) => repeatFailed(ctx, tc, repeat),
    repeatFree: (repeat) => checked(repeatFree(null, repeat), "repeatFree"),
    markComplete: (ctx, tc, status, origin) =>
      markComplete(ctx, tc, status, optionalString(origin)),
    newPool: (ctx, tc, out) => newPool(ctx, tc, out),
    poolAdd: (ctx, tc, pool, out) => poolAdd(ctx, tc, pool, out),
    poolGenerate: (ctx, tc, pool, consume, out) => poolGenerate(ctx, tc, pool, consume, out),
    poolRestore: (ctx, tc, pool, variableId) => poolRestore(ctx, tc, pool, variableId),
    poolFree: (pool) => checked(poolFree(null, pool), "poolFree"),
    newRecursion: (ctx, tc, maxDepth, maxLeaves, out) =>
      newRecursion(ctx, tc, maxDepth, maxLeaves, out),
    recursionBranch: (ctx, tc, recursion, depth, out) =>
      recursionBranch(ctx, tc, recursion, depth, out),
    recursionLeaf: (ctx, tc, recursion) => recursionLeaf(ctx, tc, recursion),
    recursionRetry: (ctx, tc, recursion) => recursionRetry(ctx, tc, recursion),
    recursionFinish: (ctx, tc, recursion) => recursionFinish(ctx, tc, recursion),
    recursionFree: (recursion) => checked(recursionFree(null, recursion), "recursionFree"),
    newStateMachine: (ctx, tc, opts, outMachine, outConcurrency) =>
      newStateMachine(
        ctx,
        tc,
        opts.ruleNames.map(nativeString),
        opts.ruleGroups,
        opts.ruleWeights,
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
    runResultErrorKind: (r) => {
      const kind: number[] = [0];
      const healthCheck: number[] = [0];
      checked(runResultErrorKind(null, r, kind, healthCheck), "runResultErrorKind");
      return { kind: kind[0], healthCheck: healthCheck[0] };
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
    failureCaveat: (fp) => {
      const out: Ptr[] = [null];
      checked(failureCaveat(null, fp, out), "failureCaveat");
      return readOptionalString(out[0]);
    },
    version: () => {
      const out: NativeBuffer[] = [{ data: null, len: 0 }];
      checked(version(null, out), "version");
      return readString(out[0]);
    },
    tzdbVersion: () => {
      const out: NativeBuffer[] = [{ data: null, len: 0 }];
      checked(tzdbVersion(null, out), "tzdbVersion");
      return readString(out[0]);
    },
  };
}

function emptyDatetime(): NativeDatetime {
  return {
    date: { year: 0, month: 0, day: 0 },
    time: { hour: 0, minute: 0, second: 0, nanosecond: 0 },
  };
}

function timestampFromRaw(raw: RawTimestamp): NativeTimestamp {
  return { second: BigInt(raw.second), nanosecond: raw.nanosecond };
}

/** Copy an engine-owned `{data, len}` buffer into a JS-owned Uint8Array. */
function copyNativeBuffer(result: NativeBuffer): Uint8Array {
  const len = Number(result.len);
  if (!Number.isSafeInteger(len) || len < 0 || (len > 0 && result.data === null)) {
    throw new EngineError("Invalid native buffer result");
  }
  if (len === 0) {
    return new Uint8Array(0);
  }
  return Uint8Array.from(koffi.decode(result.data, "uint8_t", len) as unknown as number[]);
}

/**
 * High-level wrapper over the libhegel C ABI.
 */
export class Libhegel implements Engine {
  private readonly fns: Bindings;
  private readonly pointers = new WeakMap<object, Ptr>();

  constructor(fns: Bindings) {
    this.fns = fns;
  }

  /** Load libhegel from a shared-library path. */
  static load(path: string): Libhegel {
    return new Libhegel(bindLibrary(koffi.load(path)));
  }

  /**
   * Wrap a pointer the engine handed out in a fresh handle object, so that a
   * handle used after it is freed is caught here rather than reaching the
   * engine as a dangling pointer.
   */
  private requireHandle<H>(value: Ptr, op: string): H {
    if (value === null || value === undefined)
      throw new EngineError(`${op} returned a null handle`);
    const handle = Object.freeze({});
    this.pointers.set(handle, value);
    return handle as H;
  }

  private ptr(handle: Ptr): Ptr {
    const pointer = this.pointers.get(handle as object);
    if (pointer === undefined) throw new EngineError(INVALID_HANDLE);
    return pointer;
  }

  private release(handle: Ptr, free: (pointer: Ptr) => void): void {
    free(this.ptr(handle));
    this.pointers.delete(handle as object);
  }

  freeStringGenerator(generator: StringGeneratorHandle): void {
    this.release(generator, (pointer) => this.fns.stringGeneratorFree(pointer));
  }

  version(): string {
    return this.fns.version();
  }

  tzdbVersion(): string {
    return this.fns.tzdbVersion();
  }

  newContext(): ContextHandle {
    return this.requireHandle<ContextHandle>(this.fns.contextNew(), "hegel_context_new");
  }

  setUnicodeVersion(ctx: Ptr, version: string): void {
    this.fns.setUnicodeVersion(this.ptr(ctx), version);
  }

  freeContext(ctx: Ptr): void {
    this.release(ctx, (pointer) => this.fns.contextFree(pointer));
  }

  lastError(ctx: Ptr): string {
    return this.fns.contextLastError(this.ptr(ctx));
  }

  /**
   * Create a settings handle initialized from the default profile. Throws
   * {@link LibhegelError} when the profile cannot be resolved (an unknown
   * `HEGEL_DEFAULT_PROFILE`, a malformed `hegel.toml`).
   */
  newSettings(ctx: Ptr): SettingsHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.settingsNew(this.ptr(ctx), out), "hegel_settings_new");
    return this.requireHandle<SettingsHandle>(out[0], "hegel_settings_new");
  }

  freeSettings(s: Ptr): void {
    this.release(s, (pointer) => this.fns.settingsFree(pointer));
  }

  setTestCases(s: Ptr, n: number): void {
    this.fns.settingsTestCases(this.ptr(s), n);
  }

  setVerbosity(s: Ptr, v: number): void {
    this.fns.settingsVerbosity(this.ptr(s), v);
  }

  /**
   * Create a settings handle initialized from the named profile. Throws
   * {@link LibhegelError} when the profile cannot be resolved.
   */
  newSettingsForProfile(ctx: Ptr, name: string): SettingsHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.settingsNewForProfile(this.ptr(ctx), name, out),
      "hegel_settings_new_for_profile",
    );
    return this.requireHandle<SettingsHandle>(out[0], "hegel_settings_new_for_profile");
  }

  setSeed(s: Ptr, seed: bigint | null): void {
    this.fns.settingsSeed(this.ptr(s), seed ?? 0n, seed !== null);
  }

  setDerandomize(s: Ptr, on: boolean): void {
    this.fns.settingsDerandomize(this.ptr(s), on);
  }

  setDatabase(ctx: Ptr, s: Ptr, db: string | null): void {
    this.fns.settingsDatabase(this.ptr(ctx), this.ptr(s), db);
  }

  setDatabaseKey(ctx: Ptr, s: Ptr, key: string): void {
    this.fns.settingsDatabaseKey(this.ptr(ctx), this.ptr(s), key);
  }

  setSuppressHealthCheck(s: Ptr, checks: number): void {
    this.fns.settingsSuppressHealthCheck(this.ptr(s), checks);
  }

  setReportMultipleFailures(s: Ptr, yes: boolean): void {
    this.fns.settingsReportMultipleFailures(this.ptr(s), yes);
  }

  setStopOnFirstFailure(s: Ptr, yes: boolean): void {
    this.fns.settingsStopOnFirstFailure(this.ptr(s), yes);
  }

  setShowStatistics(s: Ptr, yes: boolean): void {
    this.fns.settingsShowStatistics(this.ptr(s), yes);
  }

  setUnboundedChoices(s: Ptr, yes: boolean): void {
    this.fns.settingsUnboundedChoices(this.ptr(s), yes);
  }

  setPrintBlob(s: Ptr, yes: boolean): void {
    this.fns.settingsPrintBlob(this.ptr(s), yes);
  }

  setPhases(s: Ptr, phases: number): void {
    this.fns.settingsPhases(this.ptr(s), phases);
  }

  setBackend(s: Ptr, backend: number): void {
    this.fns.settingsBackend(this.ptr(s), backend);
  }

  setNondeterminismStrictness(s: Ptr, strictness: number): void {
    this.fns.settingsNondeterminismStrictness(this.ptr(s), strictness);
  }

  setTestLocation(ctx: Ptr, s: Ptr, location: NativeTestLocation): void {
    this.fns.settingsTestLocation(this.ptr(ctx), this.ptr(s), location);
  }

  registerProfile(ctx: Ptr, name: string, s: Ptr): void {
    this.fns.settingsRegisterProfile(this.ptr(ctx), name, this.ptr(s));
  }

  setDefaultProfile(ctx: Ptr, name: string): void {
    this.fns.setDefaultProfile(this.ptr(ctx), name);
  }

  getVerbosity(s: Ptr): number {
    return this.fns.settingsGetVerbosity(this.ptr(s));
  }

  getPhases(s: Ptr): number {
    return this.fns.settingsGetPhases(this.ptr(s));
  }

  getPrintBlob(s: Ptr): boolean {
    return this.fns.settingsGetPrintBlob(this.ptr(s));
  }

  getReportMultipleFailures(s: Ptr): boolean {
    return this.fns.settingsGetReportMultipleFailures(this.ptr(s));
  }

  getTestCases(s: Ptr): number {
    return this.fns.settingsGetTestCases(this.ptr(s));
  }

  getSeed(s: Ptr): bigint | null {
    return this.fns.settingsGetSeed(this.ptr(s));
  }

  getDerandomize(s: Ptr): boolean {
    return this.fns.settingsGetDerandomize(this.ptr(s));
  }

  getDatabase(s: Ptr): string | null {
    return this.fns.settingsGetDatabase(this.ptr(s));
  }

  getSuppressHealthCheck(s: Ptr): number {
    return this.fns.settingsGetSuppressHealthCheck(this.ptr(s));
  }

  getStopOnFirstFailure(s: Ptr): boolean {
    return this.fns.settingsGetStopOnFirstFailure(this.ptr(s));
  }

  getShowStatistics(s: Ptr): boolean {
    return this.fns.settingsGetShowStatistics(this.ptr(s));
  }

  getUnboundedChoices(s: Ptr): boolean {
    return this.fns.settingsGetUnboundedChoices(this.ptr(s));
  }

  getBackend(s: Ptr): number {
    return this.fns.settingsGetBackend(this.ptr(s));
  }

  getNondeterminismStrictness(s: Ptr): number {
    return this.fns.settingsGetNondeterminismStrictness(this.ptr(s));
  }

  /** Start a run. Throws {@link LibhegelError} on failure. */
  runStart(ctx: Ptr, settings: Ptr): RunHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.runStart(this.ptr(ctx), this.ptr(settings), out), "hegel_run_start");
    return this.requireHandle<RunHandle>(out[0], "runStart");
  }

  /**
   * Start a run replaying a reproduce blob. Throws {@link LibhegelError} for a
   * blob that is not valid.
   */
  runStartBlob(ctx: Ptr, settings: Ptr, blob: string): RunHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.runStartBlob(this.ptr(ctx), this.ptr(settings), blob, out),
      "hegel_run_start_blob",
    );
    return this.requireHandle<RunHandle>(out[0], "runStartBlob");
  }

  /**
   * Pull the next test case, or `null` when the run is finished. Throws if the
   * engine reported a mid-run error (e.g. the previous case was not completed).
   * The returned handle is owned by the caller — release it with
   * {@link freeTestCase} once the case is complete.
   */
  nextTestCase(ctx: Ptr, run: Ptr): TestCaseHandle | null {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.nextTestCase(this.ptr(ctx), this.ptr(run), out),
      "hegel_next_test_case",
    );
    return out[0] === null
      ? null
      : this.requireHandle<TestCaseHandle>(out[0], "hegel_next_test_case");
  }

  /**
   * Read the aggregated run result: a caller-owned copy, released with
   * {@link freeRunResult}. Throws on failure.
   */
  runResult(ctx: Ptr, run: Ptr): RunResultHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.runResult(this.ptr(ctx), this.ptr(run), out), "hegel_run_result");
    return this.requireHandle<RunResultHandle>(out[0], "runResult");
  }

  freeRunResult(r: Ptr): void {
    this.release(r, (pointer) => this.fns.runResultFree(pointer));
  }

  freeRun(run: Ptr): void {
    this.release(run, (pointer) => this.fns.runFree(pointer));
  }

  freeTestCase(tc: Ptr): void {
    this.release(tc, (pointer) => this.fns.testCaseFree(pointer));
  }

  shouldCapture(ctx: Ptr, tc: Ptr): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.testCaseShouldCapture(this.ptr(ctx), this.ptr(tc), out),
      "hegel_test_case_should_capture",
    );
    return out[0];
  }

  /** A clone of `tc`: owned by the caller, released with {@link freeTestCase}. */
  cloneTestCase(ctx: Ptr, tc: Ptr): TestCaseHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.testCaseClone(this.ptr(ctx), this.ptr(tc), out),
      "hegel_test_case_clone",
    );
    return this.requireHandle<TestCaseHandle>(out[0], "hegel_test_case_clone");
  }

  /** A block handle on `tc`: owned by the caller, released with {@link freeTestCase}. */
  blockTestCase(ctx: Ptr, tc: Ptr, indent: number): TestCaseHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.testCaseBlock(this.ptr(ctx), this.ptr(tc), indent, out),
      "hegel_test_case_block",
    );
    return this.requireHandle<TestCaseHandle>(out[0], "hegel_test_case_block");
  }

  setWorker(ctx: Ptr, tc: Ptr, index: number): void {
    this.check(
      ctx,
      this.fns.testCaseSetWorker(this.ptr(ctx), this.ptr(tc), index),
      "hegel_test_case_set_worker",
    );
  }

  note(ctx: Ptr, tc: Ptr, text: string): void {
    this.check(ctx, this.fns.note(this.ptr(ctx), this.ptr(tc), text), "hegel_note");
  }

  target(ctx: Ptr, tc: Ptr, value: number, label: string): void {
    this.check(ctx, this.fns.target(this.ptr(ctx), this.ptr(tc), value, label), "hegel_target");
  }

  event(ctx: Ptr, tc: Ptr, label: string): void {
    this.check(ctx, this.fns.event(this.ptr(ctx), this.ptr(tc), label), "hegel_event");
  }

  eventValue(ctx: Ptr, tc: Ptr, value: number, label: string): void {
    this.check(
      ctx,
      this.fns.eventValue(this.ptr(ctx), this.ptr(tc), value, label),
      "hegel_event_value",
    );
  }

  labelFromName(ctx: Ptr, name: string): bigint {
    const out: (number | bigint)[] = [0];
    this.check(ctx, this.fns.labelFromName(this.ptr(ctx), name, out), "hegel_label_from_name");
    return BigInt.asUintN(64, BigInt(out[0]));
  }

  labelCombine(ctx: Ptr, labels: readonly bigint[]): bigint {
    const out: (number | bigint)[] = [0];
    this.check(ctx, this.fns.labelCombine(this.ptr(ctx), labels, out), "hegel_label_combine");
    return BigInt.asUintN(64, BigInt(out[0]));
  }

  /** A printer onto `tc`'s print region: released with {@link freePrinter}. */
  testCasePrinter(ctx: Ptr, tc: Ptr): PrinterHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.testCasePrinter(this.ptr(ctx), this.ptr(tc), out),
      "hegel_test_case_printer",
    );
    return this.requireHandle<PrinterHandle>(out[0], "hegel_test_case_printer");
  }

  /** Printer options, every one at its default: released with {@link freePrinterOptions}. */
  newPrinterOptions(ctx: Ptr): PrinterOptionsHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.printerOptionsNew(this.ptr(ctx), out), "hegel_printer_options_new");
    return this.requireHandle<PrinterOptionsHandle>(out[0], "hegel_printer_options_new");
  }

  setPrinterMaxWidth(ctx: Ptr, options: Ptr, width: bigint): void {
    this.check(
      ctx,
      this.fns.printerOptionsSetMaxWidth(this.ptr(ctx), this.ptr(options), width),
      "hegel_printer_options_set_max_width",
    );
  }

  freePrinterOptions(options: Ptr): void {
    this.release(options, (pointer) => this.fns.printerOptionsFree(pointer));
  }

  /** A standalone document: released with {@link freePrinter}. */
  newPrinter(ctx: Ptr, options: PrinterOptionsHandle | null): PrinterHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.printerNew(this.ptr(ctx), options === null ? null : this.ptr(options), out),
      "hegel_printer_new",
    );
    return this.requireHandle<PrinterHandle>(out[0], "hegel_printer_new");
  }

  freePrinter(p: Ptr): void {
    this.release(p, (pointer) => this.fns.printerFree(pointer));
  }

  printerText(ctx: Ptr, p: Ptr, text: string): void {
    this.check(ctx, this.fns.printerText(this.ptr(ctx), this.ptr(p), text), "hegel_printer_text");
  }

  printerHardBreak(ctx: Ptr, p: Ptr): void {
    this.check(
      ctx,
      this.fns.printerHardBreak(this.ptr(ctx), this.ptr(p)),
      "hegel_printer_hard_break",
    );
  }

  printerBeginGroup(ctx: Ptr, p: Ptr, indent: number, open: string): void {
    this.check(
      ctx,
      this.fns.printerBeginGroup(this.ptr(ctx), this.ptr(p), BigInt(indent), open),
      "hegel_printer_begin_group",
    );
  }

  printerEndGroup(ctx: Ptr, p: Ptr, close: string): void {
    this.check(
      ctx,
      this.fns.printerEndGroup(this.ptr(ctx), this.ptr(p), close),
      "hegel_printer_end_group",
    );
  }

  printerBreakable(ctx: Ptr, p: Ptr, sep: string): void {
    this.check(
      ctx,
      this.fns.printerBreakable(this.ptr(ctx), this.ptr(p), sep),
      "hegel_printer_breakable",
    );
  }

  printerIfBreak(ctx: Ptr, p: Ptr, text: string): void {
    this.check(
      ctx,
      this.fns.printerIfBreak(this.ptr(ctx), this.ptr(p), text),
      "hegel_printer_if_break",
    );
  }

  printerComment(ctx: Ptr, p: Ptr, text: string): void {
    this.check(
      ctx,
      this.fns.printerComment(this.ptr(ctx), this.ptr(p), text),
      "hegel_printer_comment",
    );
  }

  printerShiftIndent(ctx: Ptr, p: Ptr, delta: number): void {
    this.check(
      ctx,
      this.fns.printerShiftIndent(this.ptr(ctx), this.ptr(p), BigInt(delta)),
      "hegel_printer_shift_indent",
    );
  }

  printerReflow(ctx: Ptr, p: Ptr, text: string): void {
    this.check(
      ctx,
      this.fns.printerReflow(this.ptr(ctx), this.ptr(p), text),
      "hegel_printer_reflow",
    );
  }

  printerBeginSpeculative(ctx: Ptr, p: Ptr): void {
    this.check(
      ctx,
      this.fns.printerBeginSpeculative(this.ptr(ctx), this.ptr(p)),
      "hegel_printer_begin_speculative",
    );
  }

  printerCommitSpeculative(ctx: Ptr, p: Ptr): void {
    this.check(
      ctx,
      this.fns.printerCommitSpeculative(this.ptr(ctx), this.ptr(p)),
      "hegel_printer_commit_speculative",
    );
  }

  printerAbortSpeculative(ctx: Ptr, p: Ptr): void {
    this.check(
      ctx,
      this.fns.printerAbortSpeculative(this.ptr(ctx), this.ptr(p)),
      "hegel_printer_abort_speculative",
    );
  }

  /** A child region at `p`'s current position: released with {@link freePrinter}. */
  printerDeferred(ctx: Ptr, p: Ptr): PrinterHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.printerDeferred(this.ptr(ctx), this.ptr(p), out),
      "hegel_printer_deferred",
    );
    return this.requireHandle<PrinterHandle>(out[0], "hegel_printer_deferred");
  }

  printerIsLive(ctx: Ptr, p: Ptr): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.printerIsLive(this.ptr(ctx), this.ptr(p), out),
      "hegel_printer_is_live",
    );
    return out[0];
  }

  printerResolve(ctx: Ptr, p: Ptr): void {
    this.check(ctx, this.fns.printerResolve(this.ptr(ctx), this.ptr(p)), "hegel_printer_resolve");
  }

  printerValue(ctx: Ptr, p: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(ctx, this.fns.printerValue(this.ptr(ctx), this.ptr(p), out), "hegel_printer_value");
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.printerValueResultFree(out[0]);
    }
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
    this.check(
      ctx,
      this.fns.generateBoolean(this.ptr(ctx), this.ptr(tc), p, out),
      "hegel_generate_boolean",
    );
    return out[0];
  }

  /**
   * Draw an integer in `[min, max]`. Both bounds must satisfy
   * {@link fitsInt64}; use {@link generateIntegerBig} otherwise.
   */
  generateInteger(ctx: Ptr, tc: Ptr, min: bigint, max: bigint): bigint {
    const out: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.generateInteger(this.ptr(ctx), this.ptr(tc), min, max, out),
      "hegel_generate_integer",
    );
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
      this.fns.generateIntegerBig(
        this.ptr(ctx),
        this.ptr(tc),
        minBytes,
        maxBytes,
        outValue,
        outLen,
      ),
      "hegel_generate_integer_big",
    );
    return twosComplementLEToBigInt(outValue);
  }

  /** Draw a float per the given `hegel_generate_float` options. */
  generateFloat(ctx: Ptr, tc: Ptr, opts: NativeFloatOptions): number {
    const out: number[] = [0];
    this.check(
      ctx,
      this.fns.generateFloat(this.ptr(ctx), this.ptr(tc), opts, out),
      "hegel_generate_float",
    );
    return out[0];
  }

  /** Draw a byte string with length in `[minSize, maxSize]` (see {@link Engine.generateBytes}). */
  generateBytes(ctx: Ptr, tc: Ptr, minSize: number, maxSize: bigint): Uint8Array {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(
      ctx,
      this.fns.generateBytes(this.ptr(ctx), this.ptr(tc), BigInt(minSize), maxSize, out),
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
    this.check(
      ctx,
      this.fns.stringGeneratorText(this.ptr(ctx), opts, out),
      "hegel_string_generator_text",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorText");
  }

  /** Build a regex string generator. Throws {@link LibhegelError} on a bad pattern. */
  stringGeneratorRegex(
    ctx: Ptr,
    pattern: string,
    fullmatch: boolean,
    alphabet: StringGeneratorHandle | null,
  ): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.stringGeneratorRegex(
        this.ptr(ctx),
        pattern,
        fullmatch,
        alphabet === null ? null : this.ptr(alphabet),
        out,
      ),
      "hegel_string_generator_regex",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorRegex");
  }

  /** Build an email-address string generator. */
  stringGeneratorEmail(ctx: Ptr): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.stringGeneratorEmail(this.ptr(ctx), out),
      "hegel_string_generator_email",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorEmail");
  }

  /** Build a URL string generator. */
  stringGeneratorUrl(ctx: Ptr): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.stringGeneratorUrl(this.ptr(ctx), out), "hegel_string_generator_url");
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorUrl");
  }

  /** Build a domain-name string generator. Throws on an out-of-range length. */
  stringGeneratorDomain(ctx: Ptr, maxLength: number): StringGeneratorHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.stringGeneratorDomain(this.ptr(ctx), maxLength, out),
      "hegel_string_generator_domain",
    );
    return this.requireHandle<StringGeneratorHandle>(out[0], "stringGeneratorDomain");
  }

  /** Draw a string from a `hegel_string_generator_t`. */
  generateString(ctx: Ptr, tc: Ptr, generator: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(
      ctx,
      this.fns.generateString(this.ptr(ctx), this.ptr(tc), this.ptr(generator), out),
      "hegel_generate_string",
    );
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.ownedStringFree(out[0]);
    }
  }

  /** Start gathering a grammar. Throws {@link LibhegelError} for text that does not parse. */
  grammarBuilderNew(ctx: Ptr, text: string): GrammarBuilderHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.grammarBuilderNew(this.ptr(ctx), text, out),
      "hegel_grammar_builder_new",
    );
    return this.requireHandle<GrammarBuilderHandle>(out[0], "grammarBuilderNew");
  }

  /** The `index`th file the grammar needs, or `null` past the end of the list. */
  grammarBuilderFile(ctx: Ptr, builder: Ptr, index: number): string | null {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    const code = this.fns.grammarBuilderFile(this.ptr(ctx), this.ptr(builder), index, out);
    if (code === RESULT_NOT_FOUND) return null;
    this.check(ctx, code, "hegel_grammar_builder_file");
    return wtf8ToString(copyNativeBuffer(out[0]));
  }

  grammarBuilderProvide(ctx: Ptr, builder: Ptr, index: number, text: string | null): void {
    this.check(
      ctx,
      this.fns.grammarBuilderProvide(this.ptr(ctx), this.ptr(builder), index, text),
      "hegel_grammar_builder_provide",
    );
  }

  freeGrammarBuilder(builder: Ptr): void {
    this.release(builder, (pointer) => this.fns.grammarBuilderFree(pointer));
  }

  /** Compile a gathered grammar. Throws {@link LibhegelError} with the engine's diagnostic. */
  newGrammar(ctx: Ptr, builder: Ptr, opts: GrammarOptions): GrammarHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.grammarNew(
        this.ptr(ctx),
        this.ptr(builder),
        {
          ...opts,
          alphabet:
            opts.alphabet === null ? null : (this.ptr(opts.alphabet) as StringGeneratorHandle),
        },
        out,
      ),
      "hegel_grammar_new",
    );
    return this.requireHandle<GrammarHandle>(out[0], "newGrammar");
  }

  freeGrammar(grammar: Ptr): void {
    this.release(grammar, (pointer) => this.fns.grammarFree(pointer));
  }

  grammarDrawNew(ctx: Ptr, grammar: Ptr, maxSize: number): GrammarDrawHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.grammarDrawNew(this.ptr(ctx), this.ptr(grammar), BigInt(maxSize), out),
      "hegel_grammar_draw_new",
    );
    return this.requireHandle<GrammarDrawHandle>(out[0], "grammarDrawNew");
  }

  /** The explicit terminal whose value is due, or `null` once the string is complete. */
  grammarDrawNext(ctx: Ptr, tc: Ptr, draw: Ptr): number | null {
    const out: (number | bigint)[] = [0];
    const code = this.fns.grammarDrawNext(this.ptr(ctx), this.ptr(tc), this.ptr(draw), out);
    if (code === RESULT_NOT_FOUND) return null;
    this.check(ctx, code, "hegel_grammar_draw_next");
    return Number(out[0]);
  }

  grammarDrawProvide(ctx: Ptr, draw: Ptr, value: string): void {
    this.check(
      ctx,
      this.fns.grammarDrawProvide(this.ptr(ctx), this.ptr(draw), value),
      "hegel_grammar_draw_provide",
    );
  }

  grammarDrawResult(ctx: Ptr, draw: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(
      ctx,
      this.fns.grammarDrawResult(this.ptr(ctx), this.ptr(draw), out),
      "hegel_grammar_draw_result",
    );
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.ownedStringFree(out[0]);
    }
  }

  freeGrammarDraw(draw: Ptr): void {
    this.release(draw, (pointer) => this.fns.grammarDrawFree(pointer));
  }

  /** Create date options allowing every weekday and month; release with {@link freeDateOptions}. */
  dateOptionsNew(ctx: Ptr): DateOptionsHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.dateOptionsNew(this.ptr(ctx), out), "hegel_date_options_new");
    return this.requireHandle<DateOptionsHandle>(out[0], "dateOptionsNew");
  }

  dateOptionsSetWeekdays(ctx: Ptr, options: Ptr, mask: number): void {
    this.check(
      ctx,
      this.fns.dateOptionsSetWeekdays(this.ptr(ctx), this.ptr(options), mask),
      "hegel_date_options_set_weekdays",
    );
  }

  dateOptionsSetMonths(ctx: Ptr, options: Ptr, mask: number): void {
    this.check(
      ctx,
      this.fns.dateOptionsSetMonths(this.ptr(ctx), this.ptr(options), mask),
      "hegel_date_options_set_months",
    );
  }

  freeDateOptions(options: Ptr): void {
    this.release(options, (pointer) => this.fns.dateOptionsFree(pointer));
  }

  /** Draw a date in `[min, max]`, within `options` when given. */
  generateDate(ctx: Ptr, tc: Ptr, min: NativeDate, max: NativeDate, options: Ptr): NativeDate {
    const out: NativeDate[] = [{ year: 0, month: 0, day: 0 }];
    this.check(
      ctx,
      this.fns.generateDate(
        this.ptr(ctx),
        this.ptr(tc),
        min,
        max,
        options === null ? null : this.ptr(options),
        out,
      ),
      "hegel_generate_date",
    );
    return out[0];
  }

  /** Draw a time of day in `[min, max]` at `precision`. */
  generateTime(ctx: Ptr, tc: Ptr, min: NativeTime, max: NativeTime, precision: number): NativeTime {
    const out: NativeTime[] = [{ hour: 0, minute: 0, second: 0, nanosecond: 0 }];
    this.check(
      ctx,
      this.fns.generateTime(this.ptr(ctx), this.ptr(tc), min, max, precision, out),
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
      this.fns.generateDatetime(this.ptr(ctx), this.ptr(tc), min, max, precision, out),
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
      this.fns.generateTimestamp(this.ptr(ctx), this.ptr(tc), min, max, precision, out),
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
      this.fns.generateOffset(
        this.ptr(ctx),
        this.ptr(tc),
        min,
        max,
        granularity,
        allowUnknown,
        out,
      ),
      "hegel_generate_offset",
    );
    return out[0];
  }

  /** Look `name` up in the bundled time zone database; `known` is false for an unknown name. */
  timezoneInfo(ctx: Ptr, name: string): NativeTimeZoneInfo {
    const out: RawTimeZoneInfo[] = [
      { known: false, primary_name: { data: null, len: 0 }, is_link: false, rank: 0, tags: 0 },
    ];
    this.check(ctx, this.fns.timezoneInfo(this.ptr(ctx), name, out), "hegel_timezone_info");
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
    this.check(ctx, this.fns.timezoneSpecNew(this.ptr(ctx), out), "hegel_timezone_spec_new");
    return this.requireHandle<TimeZoneSpecHandle>(out[0], "timezoneSpecNew");
  }

  /** Restrict `spec` to `names`. Throws {@link LibhegelError} for a name the database lacks. */
  timezoneSpecSetNames(ctx: Ptr, spec: Ptr, names: readonly string[]): void {
    this.check(
      ctx,
      this.fns.timezoneSpecSetNames(this.ptr(ctx), this.ptr(spec), names),
      "hegel_timezone_spec_set_names",
    );
  }

  /** Draw from and evaluate against `source`. */
  timezoneSpecSetSource(ctx: Ptr, spec: Ptr, source: Ptr): void {
    this.check(
      ctx,
      this.fns.timezoneSpecSetSource(this.ptr(ctx), this.ptr(spec), this.ptr(source)),
      "hegel_timezone_spec_set_source",
    );
  }

  timezoneSpecSetLinks(ctx: Ptr, spec: Ptr, links: boolean): void {
    this.check(
      ctx,
      this.fns.timezoneSpecSetLinks(this.ptr(ctx), this.ptr(spec), links),
      "hegel_timezone_spec_set_links",
    );
  }

  timezoneSpecSetLegacy(ctx: Ptr, spec: Ptr, legacy: boolean): void {
    this.check(
      ctx,
      this.fns.timezoneSpecSetLegacy(this.ptr(ctx), this.ptr(spec), legacy),
      "hegel_timezone_spec_set_legacy",
    );
  }

  freeTimezoneSpec(spec: Ptr): void {
    this.release(spec, (pointer) => this.fns.timezoneSpecFree(pointer));
  }

  /** Create an empty zone source; it lives as long as the library. */
  zoneSourceNew(ctx: Ptr): ZoneSourceHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.zoneSourceNew(this.ptr(ctx), out), "hegel_zone_source_new");
    return this.requireHandle<ZoneSourceHandle>(out[0], "zoneSourceNew");
  }

  /** Add the zone `name` from a TZif file. Throws {@link LibhegelError} for a malformed file. */
  zoneSourceAddTzif(ctx: Ptr, source: Ptr, name: string, bytes: Uint8Array): void {
    this.check(
      ctx,
      this.fns.zoneSourceAddTzif(this.ptr(ctx), this.ptr(source), name, bytes),
      "hegel_zone_source_add_tzif",
    );
  }

  /** Add the zone `name` from its data. Throws {@link LibhegelError} for invalid data. */
  zoneSourceAddZone(ctx: Ptr, source: Ptr, name: string, data: NativeZoneData): void {
    this.check(
      ctx,
      this.fns.zoneSourceAddZone(this.ptr(ctx), this.ptr(source), name, data),
      "hegel_zone_source_add_zone",
    );
  }

  zoneSourceAddLink(ctx: Ptr, source: Ptr, name: string, target: string): void {
    this.check(
      ctx,
      this.fns.zoneSourceAddLink(this.ptr(ctx), this.ptr(source), name, target),
      "hegel_zone_source_add_link",
    );
  }

  zoneSourceDeclare(ctx: Ptr, source: Ptr, names: readonly string[]): void {
    this.check(
      ctx,
      this.fns.zoneSourceDeclare(this.ptr(ctx), this.ptr(source), names),
      "hegel_zone_source_declare",
    );
  }

  /** Draw a time zone name allowed by `spec`. */
  generateTimezone(ctx: Ptr, tc: Ptr, spec: Ptr): string {
    const out: NativeBuffer[] = [{ data: null, len: 0 }];
    this.check(
      ctx,
      this.fns.generateTimezone(this.ptr(ctx), this.ptr(tc), this.ptr(spec), out),
      "hegel_generate_timezone",
    );
    try {
      return wtf8ToString(copyNativeBuffer(out[0]));
    } finally {
      this.fns.ownedStringFree(out[0]);
    }
  }

  /** Create a zoned datetime spec at its defaults; release with {@link freeZonedSpec}. */
  zonedSpecNew(ctx: Ptr): ZonedSpecHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.zonedSpecNew(this.ptr(ctx), out), "hegel_zoned_spec_new");
    return this.requireHandle<ZonedSpecHandle>(out[0], "zonedSpecNew");
  }

  zonedSpecSetCivilBounds(ctx: Ptr, spec: Ptr, min: NativeDatetime, max: NativeDatetime): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetCivilBounds(this.ptr(ctx), this.ptr(spec), min, max),
      "hegel_zoned_spec_set_civil_bounds",
    );
  }

  zonedSpecSetInstantBounds(ctx: Ptr, spec: Ptr, min: NativeTimestamp, max: NativeTimestamp): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetInstantBounds(this.ptr(ctx), this.ptr(spec), min, max),
      "hegel_zoned_spec_set_instant_bounds",
    );
  }

  zonedSpecSetPrecision(ctx: Ptr, spec: Ptr, precision: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetPrecision(this.ptr(ctx), this.ptr(spec), precision),
      "hegel_zoned_spec_set_precision",
    );
  }

  zonedSpecSetImaginary(ctx: Ptr, spec: Ptr, imaginary: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetImaginary(this.ptr(ctx), this.ptr(spec), imaginary),
      "hegel_zoned_spec_set_imaginary",
    );
  }

  zonedSpecSetAmbiguous(ctx: Ptr, spec: Ptr, ambiguous: number): void {
    this.check(
      ctx,
      this.fns.zonedSpecSetAmbiguous(this.ptr(ctx), this.ptr(spec), ambiguous),
      "hegel_zoned_spec_set_ambiguous",
    );
  }

  freeZonedSpec(spec: Ptr): void {
    this.release(spec, (pointer) => this.fns.zonedSpecFree(pointer));
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
      this.fns.generateZonedDatetime(
        this.ptr(ctx),
        this.ptr(tc),
        this.ptr(tzSpec),
        this.ptr(zonedSpec),
        outValue,
        outZone,
      ),
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
    this.check(ctx, this.fns.spanSpecNew(this.ptr(ctx), out), "hegel_span_spec_new");
    return this.requireHandle<SpanSpecHandle>(out[0], "spanSpecNew");
  }

  spanSpecSetUnitBounds(ctx: Ptr, spec: Ptr, unit: number, min: bigint, max: bigint): void {
    this.check(
      ctx,
      this.fns.spanSpecSetUnitBounds(this.ptr(ctx), this.ptr(spec), unit, min, max),
      "hegel_span_spec_set_unit_bounds",
    );
  }

  spanSpecSetSign(ctx: Ptr, spec: Ptr, sign: number): void {
    this.check(
      ctx,
      this.fns.spanSpecSetSign(this.ptr(ctx), this.ptr(spec), sign),
      "hegel_span_spec_set_sign",
    );
  }

  freeSpanSpec(spec: Ptr): void {
    this.release(spec, (pointer) => this.fns.spanSpecFree(pointer));
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
    this.check(
      ctx,
      this.fns.generateSpan(this.ptr(ctx), this.ptr(tc), this.ptr(spec), out),
      "hegel_generate_span",
    );
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
      this.fns.generateUuid(this.ptr(ctx), this.ptr(tc), version ?? 0, version !== undefined, out),
      "hegel_generate_uuid",
    );
    return out;
  }

  /** Draw an IPv4 address as its 4 network-order bytes. */
  generateIpv4(ctx: Ptr, tc: Ptr): Buffer {
    const out = Buffer.alloc(4);
    this.check(ctx, this.fns.generateIpv4(this.ptr(ctx), this.ptr(tc), out), "hegel_generate_ipv4");
    return out;
  }

  /** Draw an IPv6 address as its 16 network-order bytes. */
  generateIpv6(ctx: Ptr, tc: Ptr): Buffer {
    const out = Buffer.alloc(16);
    this.check(ctx, this.fns.generateIpv6(this.ptr(ctx), this.ptr(tc), out), "hegel_generate_ipv6");
    return out;
  }

  startSpan(ctx: Ptr, tc: Ptr, label: bigint): void {
    this.check(ctx, this.fns.startSpan(this.ptr(ctx), this.ptr(tc), label), "hegel_start_span");
  }

  stopSpan(ctx: Ptr, tc: Ptr, discard: boolean): void {
    this.check(ctx, this.fns.stopSpan(this.ptr(ctx), this.ptr(tc), discard), "hegel_stop_span");
  }

  /**
   * Open a collection with the given size bounds. The returned handle is owned
   * by the caller — release it with {@link freeCollection}.
   */
  newCollection(ctx: Ptr, tc: Ptr, min: number, max?: number): CollectionHandle {
    const out: Ptr[] = [null];
    const maxArg = max === undefined ? UINT64_MAX : BigInt(max);
    this.check(
      ctx,
      this.fns.newCollection(this.ptr(ctx), this.ptr(tc), min, maxArg, out),
      "hegel_new_collection",
    );
    return this.requireHandle<CollectionHandle>(out[0], "newCollection");
  }

  collectionMore(ctx: Ptr, tc: Ptr, collection: Ptr): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.collectionMore(this.ptr(ctx), this.ptr(tc), this.ptr(collection), out),
      "hegel_collection_more",
    );
    return out[0];
  }

  collectionReject(ctx: Ptr, tc: Ptr, collection: Ptr, why: string | null): void {
    this.check(
      ctx,
      this.fns.collectionReject(this.ptr(ctx), this.ptr(tc), this.ptr(collection), why),
      "hegel_collection_reject",
    );
  }

  freeCollection(collection: Ptr): void {
    this.release(collection, (pointer) => this.fns.collectionFree(pointer));
  }

  /**
   * Start a repeat loop. The returned handle is owned by the caller — release
   * it with {@link freeRepeat}.
   */
  newRepeat(ctx: Ptr, tc: Ptr): RepeatHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.newRepeat(this.ptr(ctx), this.ptr(tc), out), "hegel_new_repeat");
    return this.requireHandle<RepeatHandle>(out[0], "newRepeat");
  }

  repeatMore(ctx: Ptr, tc: Ptr, repeat: Ptr): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.repeatMore(this.ptr(ctx), this.ptr(tc), this.ptr(repeat), out),
      "hegel_repeat_more",
    );
    return out[0];
  }

  repeatFailed(ctx: Ptr, tc: Ptr, repeat: Ptr): void {
    this.check(
      ctx,
      this.fns.repeatFailed(this.ptr(ctx), this.ptr(tc), this.ptr(repeat)),
      "hegel_repeat_failed",
    );
  }

  freeRepeat(repeat: Ptr): void {
    this.release(repeat, (pointer) => this.fns.repeatFree(pointer));
  }

  markComplete(ctx: Ptr, tc: Ptr, status: number, origin: string | null): void {
    this.check(
      ctx,
      this.fns.markComplete(this.ptr(ctx), this.ptr(tc), status, origin),
      "hegel_mark_complete",
    );
  }

  /**
   * Open a variable pool for stateful testing. The returned handle is owned by
   * the caller — release it with {@link freePool}.
   */
  newPool(ctx: Ptr, tc: Ptr): PoolHandle {
    const out: Ptr[] = [null];
    this.check(ctx, this.fns.newPool(this.ptr(ctx), this.ptr(tc), out), "hegel_new_pool");
    return this.requireHandle<PoolHandle>(out[0], "newPool");
  }

  /** Register a new variable in `pool`, returning its engine-assigned id. */
  poolAdd(ctx: Ptr, tc: Ptr, pool: Ptr): bigint {
    const out: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.poolAdd(this.ptr(ctx), this.ptr(tc), this.ptr(pool), out),
      "hegel_pool_add",
    );
    return BigInt(out[0]);
  }

  /**
   * Draw the id of a variable in `pool`, removing it when `consume` is set.
   * Throws {@link AssumeError} when the pool is empty.
   */
  poolGenerate(ctx: Ptr, tc: Ptr, pool: Ptr, consume: boolean): bigint {
    const out: (number | bigint)[] = [0];
    this.check(
      ctx,
      this.fns.poolGenerate(this.ptr(ctx), this.ptr(tc), this.ptr(pool), consume, out),
      "hegel_pool_generate",
    );
    return BigInt(out[0]);
  }

  /** Put back a variable a consuming {@link poolGenerate} removed from `pool`. */
  poolRestore(ctx: Ptr, tc: Ptr, pool: Ptr, variableId: bigint): void {
    this.check(
      ctx,
      this.fns.poolRestore(this.ptr(ctx), this.ptr(tc), this.ptr(pool), variableId),
      "hegel_pool_restore",
    );
  }

  freePool(pool: Ptr): void {
    this.release(pool, (pointer) => this.fns.poolFree(pointer));
  }

  /**
   * Open a recursive generation scope. The returned handle is owned by the
   * caller — release it with {@link freeRecursion}.
   */
  newRecursion(ctx: Ptr, tc: Ptr, maxDepth: number, maxLeaves: number): RecursionHandle {
    const out: Ptr[] = [null];
    this.check(
      ctx,
      this.fns.newRecursion(this.ptr(ctx), this.ptr(tc), BigInt(maxDepth), BigInt(maxLeaves), out),
      "hegel_new_recursion",
    );
    return this.requireHandle<RecursionHandle>(out[0], "newRecursion");
  }

  recursionBranch(ctx: Ptr, tc: Ptr, recursion: Ptr, depth: number): boolean {
    const out: boolean[] = [false];
    this.check(
      ctx,
      this.fns.recursionBranch(
        this.ptr(ctx),
        this.ptr(tc),
        this.ptr(recursion),
        BigInt(depth),
        out,
      ),
      "hegel_recursion_branch",
    );
    return out[0];
  }

  /** `false` when the attempt outgrew its leaf budget (`HEGEL_E_RETRY`). */
  recursionLeaf(ctx: Ptr, tc: Ptr, recursion: Ptr): boolean {
    return this.retryable(
      ctx,
      this.fns.recursionLeaf(this.ptr(ctx), this.ptr(tc), this.ptr(recursion)),
      "hegel_recursion_leaf",
    );
  }

  recursionRetry(ctx: Ptr, tc: Ptr, recursion: Ptr): void {
    this.check(
      ctx,
      this.fns.recursionRetry(this.ptr(ctx), this.ptr(tc), this.ptr(recursion)),
      "hegel_recursion_retry",
    );
  }

  /** `false` when the engine discarded the attempt as mispriced (`HEGEL_E_RETRY`). */
  recursionFinish(ctx: Ptr, tc: Ptr, recursion: Ptr): boolean {
    return this.retryable(
      ctx,
      this.fns.recursionFinish(this.ptr(ctx), this.ptr(tc), this.ptr(recursion)),
      "hegel_recursion_finish",
    );
  }

  freeRecursion(recursion: Ptr): void {
    this.release(recursion, (pointer) => this.fns.recursionFree(pointer));
  }

  private retryable(ctx: Ptr, code: number, op: string): boolean {
    if (code === RESULT_RETRY) return false;
    this.check(ctx, code, op);
    return true;
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
      this.fns.newStateMachine(this.ptr(ctx), this.ptr(tc), opts, outMachine, outConcurrency),
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
      this.fns.stateMachineNextGroup(this.ptr(ctx), this.ptr(tc), this.ptr(machine), out),
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
      this.fns.stateMachineNextRule(
        this.ptr(ctx),
        this.ptr(tc),
        this.ptr(machine),
        workerIndex,
        out,
      ),
      "hegel_state_machine_next_rule",
    );
    return doneOrIndex(out[0]);
  }

  /** Report the rule last handed to `workerIndex` as rejected (assumption failed). */
  stateMachineRuleRejected(ctx: Ptr, tc: Ptr, machine: Ptr, workerIndex: number): void {
    this.check(
      ctx,
      this.fns.stateMachineRuleRejected(
        this.ptr(ctx),
        this.ptr(tc),
        this.ptr(machine),
        workerIndex,
      ),
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
      this.fns.stateMachineShouldCheckInvariant(
        this.ptr(ctx),
        this.ptr(tc),
        this.ptr(machine),
        invariantIndex,
        out,
      ),
      "hegel_state_machine_should_check_invariant",
    );
    return out[0];
  }

  freeStateMachine(machine: Ptr): void {
    this.release(machine, (pointer) => this.fns.stateMachineFree(pointer));
  }

  runStatus(r: Ptr): number {
    return this.fns.runResultStatus(this.ptr(r));
  }

  runError(r: Ptr): string | null {
    return this.fns.runResultError(this.ptr(r));
  }

  runErrorKind(r: Ptr): NativeRunErrorKind {
    return this.fns.runResultErrorKind(this.ptr(r));
  }

  failureCount(r: Ptr): number {
    return this.fns.runResultFailureCount(this.ptr(r));
  }

  /**
   * Read the `index`th failure: a caller-owned copy, released with
   * {@link freeFailure}.
   */
  failure(r: Ptr, index: number): FailureHandle {
    return this.requireHandle<FailureHandle>(
      this.fns.runResultFailure(this.ptr(r), index),
      "hegel_run_result_failure",
    );
  }

  freeFailure(f: Ptr): void {
    this.release(f, (pointer) => this.fns.failureFree(pointer));
  }

  failureOrigin(fp: Ptr): string {
    return this.fns.failureOrigin(this.ptr(fp));
  }

  /**
   * The failure's base64 reproduce blob, or `null` if the engine produced none.
   * Replay it with {@link runStartBlob}.
   */
  reproductionBlob(fp: Ptr): string | null {
    return this.fns.failureReproductionBlob(this.ptr(fp));
  }

  failureCaveat(fp: Ptr): string | null {
    return this.fns.failureCaveat(this.ptr(fp));
  }
}
