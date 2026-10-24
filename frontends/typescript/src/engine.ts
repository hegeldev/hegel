/** Copied values and opaque ownership handles shared by native and Wasm adapters. */
declare const contextBrand: unique symbol;
export type ContextHandle = { readonly [contextBrand]: never };

declare const settingsBrand: unique symbol;
export type SettingsHandle = { readonly [settingsBrand]: never };

declare const runBrand: unique symbol;
export type RunHandle = { readonly [runBrand]: never };

declare const runResultBrand: unique symbol;
export type RunResultHandle = { readonly [runResultBrand]: never };

declare const failureBrand: unique symbol;
export type FailureHandle = { readonly [failureBrand]: never };

declare const testCaseBrand: unique symbol;
export type TestCaseHandle = { readonly [testCaseBrand]: never };

declare const collectionBrand: unique symbol;
export type CollectionHandle = { readonly [collectionBrand]: never };

declare const stringGeneratorBrand: unique symbol;
export type StringGeneratorHandle = { readonly [stringGeneratorBrand]: never };

declare const poolBrand: unique symbol;
export type PoolHandle = { readonly [poolBrand]: never };

declare const stateMachineBrand: unique symbol;
export type StateMachineHandle = { readonly [stateMachineBrand]: never };

declare const dateOptionsBrand: unique symbol;
export type DateOptionsHandle = { readonly [dateOptionsBrand]: never };

declare const timeZoneSpecBrand: unique symbol;
export type TimeZoneSpecHandle = { readonly [timeZoneSpecBrand]: never };

declare const zonedSpecBrand: unique symbol;
export type ZonedSpecHandle = { readonly [zonedSpecBrand]: never };

declare const spanSpecBrand: unique symbol;
export type SpanSpecHandle = { readonly [spanSpecBrand]: never };

/** `hegel_status_t` — outcome of a single test case. */
export const Status = {
  VALID: 0,
  INVALID: 1,
  OVERRUN: 2,
  INTERESTING: 3,
} as const;

/**
 * `hegel_run_status_t` — aggregate outcome of a finished run. Value 3
 * (`FAILED_NONDETERMINISTIC`, removed in the 0.35 ABI break) is retired and
 * must never be reused: a failing nondeterministic run now reports plain
 * `FAILED`, its failures carrying a caveat and, when confirmed, a reproduce
 * blob.
 */
export const RunStatus = {
  PASSED: 0,
  FAILED: 1,
  ERROR: 2,
} as const;

/** `hegel_verbosity_t`. */
export const NativeVerbosity = {
  NORMAL: 0,
  QUIET: 1,
  VERBOSE: 2,
  DEBUG: 3,
} as const;

/** A `hegel_date_t`: a proleptic Gregorian calendar date. */
export interface NativeDate {
  year: number;
  month: number;
  day: number;
}

/** A `hegel_time_t`: a time of day with nanosecond precision. */
export interface NativeTime {
  hour: number;
  minute: number;
  second: number;
  nanosecond: number;
}

/** A `hegel_datetime_t`: a naive datetime (no timezone). */
export interface NativeDatetime {
  date: NativeDate;
  time: NativeTime;
}

/** A `hegel_timestamp_t`: seconds since the Unix epoch plus a nanosecond in `[0, 999999999]`. */
export interface NativeTimestamp {
  second: bigint;
  nanosecond: number;
}

/** A `hegel_offset_t`: seconds east of UTC, 0 when not `known`. */
export interface NativeOffset {
  seconds: number;
  known: boolean;
}

/** A `hegel_span_t`: one `int64_t` per calendar unit. */
export interface NativeSpan {
  years: bigint;
  months: bigint;
  weeks: bigint;
  days: bigint;
  hours: bigint;
  minutes: bigint;
  seconds: bigint;
  milliseconds: bigint;
  microseconds: bigint;
  nanoseconds: bigint;
}

/** A `hegel_zoned_datetime_t`: a wall-clock time resolved in the drawn zone. */
export interface NativeZonedDatetime {
  civil: NativeDatetime;
  offsetSeconds: number;
  offsetKnown: boolean;
  fold: boolean;
  /** A {@link NativeTimeZoneKind} value. */
  kind: number;
  offsetBefore: number;
  offsetAfter: number;
  timestamp: NativeTimestamp;
}

/** A `hegel_timezone_info_t`: what the bundled database knows about a name. */
export interface NativeTimeZoneInfo {
  known: boolean;
  primaryName: string;
  isLink: boolean;
  rank: number;
  /** A bitwise OR of the `HEGEL_TIMEZONE_TAG_*` bits; see {@link NativeTimeZoneTag}. */
  tags: number;
}

/** `hegel_precision_t`. */
export const NativePrecision = {
  NANOSECONDS: 0,
  MICROSECONDS: 1,
  MILLISECONDS: 2,
  SECONDS: 3,
  MINUTES: 4,
  HOURS: 5,
} as const;

/** `hegel_offset_granularity_t`. */
export const NativeGranularity = {
  HOURS: 0,
  QUARTER_HOURS: 1,
  MINUTES: 2,
  SECONDS: 3,
} as const;

/** `hegel_imaginary_t`. */
export const NativeImaginary = { ALLOW: 0, AVOID: 1 } as const;

/** `hegel_ambiguous_t`. */
export const NativeAmbiguous = { BOTH: 0, EARLIER: 1, LATER: 2 } as const;

/** `hegel_span_unit_t`. */
export const NativeSpanUnit = {
  YEARS: 0,
  MONTHS: 1,
  WEEKS: 2,
  DAYS: 3,
  HOURS: 4,
  MINUTES: 5,
  SECONDS: 6,
  MILLISECONDS: 7,
  MICROSECONDS: 8,
  NANOSECONDS: 9,
} as const;

/** `hegel_span_sign_t`. */
export const NativeSpanSign = { UNIFORM: 0, MIXED: 1, NONNEGATIVE: 2 } as const;

/** `hegel_timezone_kind_t`. */
export const NativeTimeZoneKind = { UNIQUE: 0, GAP: 1, FOLD: 2 } as const;

/** The `HEGEL_TIMEZONE_TAG_*` bits this client reads. */
export const NativeTimeZoneTag = { LINK: 1 << 13, LEGACY: 1 << 14 } as const;

/** RFC 4122 UUID versions supported by the public generator. */
export type UuidVersion = 1 | 2 | 3 | 4 | 5;

/** Options for `hegel_string_generator_text`. */
export interface TextGeneratorOptions {
  minSize: number;
  maxSize: bigint;
  codec: string | null;
  minCodepoint: number;
  maxCodepoint: number;
  categories: readonly string[] | null;
  excludeCategories: readonly string[] | null;
  includeCharacters: Uint8Array | null;
  excludeCharacters: Uint8Array | null;
}

/** Options for `hegel_generate_float`. */
export interface NativeFloatOptions {
  width: number;
  minValue: number;
  maxValue: number;
  allowNan: boolean;
  allowInfinity: boolean;
  excludeMin: boolean;
  excludeMax: boolean;
  smallestNonzeroMagnitude: number;
}

/**
 * Options for `hegel_new_state_machine`. `ruleGroups` and
 * `invariantAlwaysCheck` are parallel to `ruleNames` / `invariantNames`.
 */
export interface StateMachineOptions {
  ruleNames: readonly string[];
  ruleGroups: readonly number[];
  invariantNames: readonly string[];
  invariantAlwaysCheck: readonly boolean[];
  minConcurrency: number;
  maxConcurrency: number;
  stepCount: number;
}

/**
 * `HEGEL_STATE_MACHINE_DONE` (`INT64_MIN`): the sentinel
 * `hegel_state_machine_next_group` / `hegel_state_machine_next_rule` write
 * instead of a group / rule index once the machine (or the worker's round) is
 * finished.
 */
export const STATE_MACHINE_DONE = -0x8000000000000000n;

/**
 * Decode an `int64_t` written by the state-machine calls: `null` for the
 * `HEGEL_STATE_MACHINE_DONE` sentinel, the (small, non-negative) index
 * otherwise.
 */
export function doneOrIndex(value: number | bigint): number | null {
  return BigInt(value) === STATE_MACHINE_DONE ? null : Number(value);
}

/** Engine/adapter faults must never be submitted to the shrinker. */
export class EngineError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "EngineError";
  }
}

export interface Engine {
  version(): string;
  newContext(): ContextHandle;
  freeContext(ctx: ContextHandle): void;
  lastError(ctx: ContextHandle): string;
  /**
   * Create a settings handle initialized from the engine's default profile.
   * Fallible: the engine reports an unknown `HEGEL_DEFAULT_PROFILE` or a
   * malformed `hegel.toml` through `ctx`.
   */
  newSettings(ctx: ContextHandle): SettingsHandle;
  freeSettings(s: SettingsHandle): void;
  setTestCases(s: SettingsHandle, n: number): void;
  setVerbosity(s: SettingsHandle, v: number): void;
  setSeed(s: SettingsHandle, seed: bigint): void;
  setDerandomize(s: SettingsHandle, on: boolean): void;
  setDatabase(ctx: ContextHandle, s: SettingsHandle, db: string | null): void;
  setDatabaseKey(ctx: ContextHandle, s: SettingsHandle, key: string): void;
  setSuppressHealthCheck(s: SettingsHandle, checks: number): void;
  setReportMultipleFailures(s: SettingsHandle, yes: boolean): void;
  runStart(ctx: ContextHandle, settings: SettingsHandle): RunHandle;
  nextTestCase(ctx: ContextHandle, run: RunHandle): TestCaseHandle | null;
  runResult(ctx: ContextHandle, run: RunHandle): RunResultHandle;
  freeRunResult(r: RunResultHandle): void;
  freeRun(run: RunHandle): void;
  testCaseFromBlob(ctx: ContextHandle, settings: SettingsHandle, blob: string): TestCaseHandle;
  freeTestCase(tc: TestCaseHandle): void;
  generateBoolean(ctx: ContextHandle, tc: TestCaseHandle, p: number): boolean;
  generateInteger(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint;
  generateIntegerBig(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint;
  generateFloat(ctx: ContextHandle, tc: TestCaseHandle, opts: NativeFloatOptions): number;
  generateBytes(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    minSize: number,
    maxSize?: number,
  ): Uint8Array;
  stringGeneratorText(ctx: ContextHandle, opts: TextGeneratorOptions): StringGeneratorHandle;
  stringGeneratorRegex(
    ctx: ContextHandle,
    pattern: string,
    fullmatch: boolean,
  ): StringGeneratorHandle;
  stringGeneratorEmail(ctx: ContextHandle): StringGeneratorHandle;
  stringGeneratorUrl(ctx: ContextHandle): StringGeneratorHandle;
  stringGeneratorDomain(ctx: ContextHandle, maxLength: number): StringGeneratorHandle;
  generateString(ctx: ContextHandle, tc: TestCaseHandle, generator: StringGeneratorHandle): string;
  /** Create date options allowing every weekday and month; released with {@link freeDateOptions}. */
  dateOptionsNew(ctx: ContextHandle): DateOptionsHandle;
  /** Restrict `options` to the weekdays in `mask` (bit 0 Monday to bit 6 Sunday). */
  dateOptionsSetWeekdays(ctx: ContextHandle, options: DateOptionsHandle, mask: number): void;
  /** Restrict `options` to the months in `mask` (bit 0 January to bit 11 December). */
  dateOptionsSetMonths(ctx: ContextHandle, options: DateOptionsHandle, mask: number): void;
  freeDateOptions(options: DateOptionsHandle): void;
  /** Draw a date in `[min, max]`, within `options` when not `null`. */
  generateDate(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDate,
    max: NativeDate,
    options: DateOptionsHandle | null,
  ): NativeDate;
  /** Draw a time of day in `[min, max]` that is a whole number of `precision` units. */
  generateTime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTime,
    max: NativeTime,
    precision: number,
  ): NativeTime;
  /** Draw a naive datetime in `[min, max]` whose time is a whole number of `precision` units. */
  generateDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDatetime,
    max: NativeDatetime,
    precision: number,
  ): NativeDatetime;
  /** Draw an instant in `[min, max]` that is a whole number of `precision` units since the epoch. */
  generateTimestamp(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTimestamp,
    max: NativeTimestamp,
    precision: number,
  ): NativeTimestamp;
  /**
   * Draw a UTC offset in `[min, max]` seconds that is a whole number of
   * `granularity` units; with `allowUnknown` the draw may yield the unknown
   * offset, reported with `known` false.
   */
  generateOffset(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: number,
    max: number,
    granularity: number,
    allowUnknown: boolean,
  ): NativeOffset;
  /** What the bundled time zone database knows about `name`; never fails. */
  timezoneInfo(ctx: ContextHandle, name: string): NativeTimeZoneInfo;
  /** Create a time zone spec allowing every bundled name; released with {@link freeTimezoneSpec}. */
  timezoneSpecNew(ctx: ContextHandle): TimeZoneSpecHandle;
  /** Restrict `spec` to `names`; a name the database lacks is an error that leaves `spec` unchanged. */
  timezoneSpecSetNames(
    ctx: ContextHandle,
    spec: TimeZoneSpecHandle,
    names: readonly string[],
  ): void;
  freeTimezoneSpec(spec: TimeZoneSpecHandle): void;
  /** Draw a time zone name allowed by `spec`. */
  generateTimezone(ctx: ContextHandle, tc: TestCaseHandle, spec: TimeZoneSpecHandle): string;
  /** Create a zoned datetime spec with every option at its default; released with {@link freeZonedSpec}. */
  zonedSpecNew(ctx: ContextHandle): ZonedSpecHandle;
  zonedSpecSetCivilBounds(
    ctx: ContextHandle,
    spec: ZonedSpecHandle,
    min: NativeDatetime,
    max: NativeDatetime,
  ): void;
  zonedSpecSetInstantBounds(
    ctx: ContextHandle,
    spec: ZonedSpecHandle,
    min: NativeTimestamp,
    max: NativeTimestamp,
  ): void;
  zonedSpecSetPrecision(ctx: ContextHandle, spec: ZonedSpecHandle, precision: number): void;
  zonedSpecSetImaginary(ctx: ContextHandle, spec: ZonedSpecHandle, imaginary: number): void;
  zonedSpecSetAmbiguous(ctx: ContextHandle, spec: ZonedSpecHandle, ambiguous: number): void;
  freeZonedSpec(spec: ZonedSpecHandle): void;
  /** Draw a zone allowed by `tzSpec` and a wall-clock time in it within `zonedSpec`'s bounds. */
  generateZonedDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    tzSpec: TimeZoneSpecHandle,
    zonedSpec: ZonedSpecHandle,
  ): { value: NativeZonedDatetime; zone: string };
  /** Create a span spec with no unit enabled; released with {@link freeSpanSpec}. */
  spanSpecNew(ctx: ContextHandle): SpanSpecHandle;
  /** Enable `unit` (a {@link NativeSpanUnit}) with the inclusive bounds `[min, max]`. */
  spanSpecSetUnitBounds(
    ctx: ContextHandle,
    spec: SpanSpecHandle,
    unit: number,
    min: bigint,
    max: bigint,
  ): void;
  /** Set how the units' signs are drawn (a {@link NativeSpanSign}). */
  spanSpecSetSign(ctx: ContextHandle, spec: SpanSpecHandle, sign: number): void;
  freeSpanSpec(spec: SpanSpecHandle): void;
  /** Draw a calendar span within `spec`'s bounds. */
  generateSpan(ctx: ContextHandle, tc: TestCaseHandle, spec: SpanSpecHandle): NativeSpan;
  generateUuid(ctx: ContextHandle, tc: TestCaseHandle, version?: UuidVersion): Uint8Array;
  generateIpv4(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array;
  generateIpv6(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array;
  startSpan(ctx: ContextHandle, tc: TestCaseHandle, label: number): void;
  stopSpan(ctx: ContextHandle, tc: TestCaseHandle, discard: boolean): void;
  newCollection(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: number,
    max?: number,
  ): CollectionHandle;
  collectionMore(ctx: ContextHandle, tc: TestCaseHandle, collection: CollectionHandle): boolean;
  collectionReject(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    collection: CollectionHandle,
    why: string | null,
  ): void;
  freeCollection(collection: CollectionHandle): void;
  markComplete(ctx: ContextHandle, tc: TestCaseHandle, status: number, origin: string | null): void;
  /** Open a variable pool for stateful testing; released with {@link freePool}. */
  newPool(ctx: ContextHandle, tc: TestCaseHandle): PoolHandle;
  /** Register a new variable in `pool`, returning its engine-assigned id. */
  poolAdd(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle): bigint;
  /**
   * Draw the id of a variable in `pool`, removing it when `consume` is set.
   * Throws `AssumeError` when the pool is empty.
   */
  poolGenerate(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle, consume: boolean): bigint;
  freePool(pool: PoolHandle): void;
  /**
   * Register a state machine on `tc`; released with {@link freeStateMachine}.
   * The engine's drawn concurrency level is discarded: this client fixes the
   * bounds at 1.
   */
  newStateMachine(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    opts: StateMachineOptions,
  ): StateMachineHandle;
  /**
   * Start the machine's next round, returning the round's group id, or `null`
   * once the machine is finished.
   */
  stateMachineNextGroup(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
  ): number | null;
  /**
   * Draw the next rule index for `workerIndex` this round, or `null` at the
   * round's join point.
   */
  stateMachineNextRule(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    workerIndex: number,
  ): number | null;
  /** Report the rule last handed to `workerIndex` as rejected (assumption failed). */
  stateMachineRuleRejected(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    workerIndex: number,
  ): void;
  /** Whether invariant `invariantIndex` should run at the current join point. */
  stateMachineShouldCheckInvariant(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    machine: StateMachineHandle,
    invariantIndex: number,
  ): boolean;
  freeStateMachine(machine: StateMachineHandle): void;
  runStatus(r: RunResultHandle): number;
  runError(r: RunResultHandle): string | null;
  failureCount(r: RunResultHandle): number;
  failure(r: RunResultHandle, index: number): FailureHandle;
  freeFailure(f: FailureHandle): void;
  failureOrigin(fp: FailureHandle): string;
  reproductionBlob(fp: FailureHandle): string | null;
  freeStringGenerator(generator: StringGeneratorHandle): void;
}
