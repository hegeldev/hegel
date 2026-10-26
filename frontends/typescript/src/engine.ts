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

declare const recursionBrand: unique symbol;
export type RecursionHandle = { readonly [recursionBrand]: never };

declare const stateMachineBrand: unique symbol;
export type StateMachineHandle = { readonly [stateMachineBrand]: never };

declare const printerBrand: unique symbol;
export type PrinterHandle = { readonly [printerBrand]: never };

declare const grammarBuilderBrand: unique symbol;
export type GrammarBuilderHandle = { readonly [grammarBuilderBrand]: never };

declare const grammarBrand: unique symbol;
export type GrammarHandle = { readonly [grammarBrand]: never };

declare const grammarDrawBrand: unique symbol;
export type GrammarDrawHandle = { readonly [grammarDrawBrand]: never };

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

/** `hegel_run_error_kind_t` — why a run ended in `RunStatus.ERROR`. */
export const RunErrorKind = {
  NONE: 0,
  HEALTH_CHECK: 1,
  FLAKY: 2,
  NONDETERMINISTIC: 3,
  UNSATISFIABLE: 4,
  USAGE: 5,
  INTERNAL: 6,
} as const;

/** `hegel_phase_t` bit flags. */
export const NativePhase = {
  EXPLICIT: 1 << 0,
  REUSE: 1 << 1,
  GENERATE: 1 << 2,
  TARGET: 1 << 3,
  SHRINK: 1 << 4,
} as const;

/** `hegel_health_check_t` bit flags. */
export const NativeHealthCheck = {
  FILTER_TOO_MUCH: 1 << 0,
  TOO_SLOW: 1 << 1,
  TEST_CASES_TOO_LARGE: 1 << 2,
  LARGE_INITIAL_TEST_CASE: 1 << 3,
} as const;

/** `hegel_backend_t`. */
export const NativeBackend = {
  DEFAULT: 1,
  URANDOM: 2,
} as const;

/** `hegel_nondeterminism_strictness_t`. */
export const NativeNondeterminismStrictness = {
  QUIET: 0,
  WARN: 1,
  ERROR: 2,
} as const;

/** The arguments of `hegel_settings_set_test_location`. */
export interface NativeTestLocation {
  file: string;
  beginLine: number;
  className: string;
  function: string;
}

/** `hegel_run_result_error_kind`'s two outputs. */
export interface NativeRunErrorKind {
  kind: number;
  healthCheck: number;
}

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

/** Options for `hegel_grammar_new`. */
export interface GrammarOptions {
  /** The rule to generate strings of, or `null` for `start`. */
  start: string | null;
  /** The most characters a string may have, or 0 for the engine's default. */
  maxSize: number;
  /** How deeply rules may nest, or 0 for the engine's default. */
  maxDepth: number;
  /** A text generator every character a terminal's pattern draws must come from. */
  alphabet: StringGeneratorHandle | null;
  /** The terminals whose values the caller supplies, in index order. */
  explicitNames: readonly string[];
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

/** `UINT64_MAX`: as a size bound, no upper bound at all. */
export const UINT64_MAX = 0xffffffffffffffffn;

/**
 * `HEGEL_MAX_SIZE_DEFAULT` (`UINT64_MAX - 1`): the `max_size` of a text or
 * binary draw that asks the engine for its default maximum. The value is
 * reserved, so an explicit maximum at or above it is passed as
 * {@link UINT64_MAX} instead (see {@link maxSizeArgument}).
 */
export const MAX_SIZE_DEFAULT = 0xfffffffffffffffen;

/**
 * `HEGEL_FILTER_ATTEMPTS`: how many times a filtered draw tries a value before
 * it rejects the test case.
 */
export const FILTER_ATTEMPTS = 3;

/**
 * The `max_size` to pass for a text or binary draw: {@link MAX_SIZE_DEFAULT}
 * when the user gave no maximum, so the engine applies its default, and the
 * user's maximum otherwise, clamped off the reserved sentinel.
 */
export function maxSizeArgument(maxSize: number | undefined): bigint {
  if (maxSize === undefined) return MAX_SIZE_DEFAULT;
  const value = BigInt(maxSize);
  return value >= MAX_SIZE_DEFAULT ? UINT64_MAX : value;
}

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
  /**
   * Make the string generators follow the host's Unicode version, process-wide.
   * Throws for a version the engine has no tables for.
   */
  setUnicodeVersion(ctx: ContextHandle, version: string): void;
  freeContext(ctx: ContextHandle): void;
  lastError(ctx: ContextHandle): string;
  /**
   * Create a settings handle initialized from the engine's default profile.
   * Fallible: the engine reports an unknown `HEGEL_DEFAULT_PROFILE` or a
   * malformed `hegel.toml` through `ctx`.
   */
  newSettings(ctx: ContextHandle): SettingsHandle;
  /**
   * Create a settings handle initialized from the named profile. Fallible: an
   * unknown profile or a malformed `hegel.toml` is reported through `ctx`.
   */
  newSettingsForProfile(ctx: ContextHandle, name: string): SettingsHandle;
  freeSettings(s: SettingsHandle): void;
  setTestCases(s: SettingsHandle, n: number): void;
  setVerbosity(s: SettingsHandle, v: number): void;
  /** Fix the seed, or clear it (`null`) so the engine picks one per run. */
  setSeed(s: SettingsHandle, seed: bigint | null): void;
  setDerandomize(s: SettingsHandle, on: boolean): void;
  setDatabase(ctx: ContextHandle, s: SettingsHandle, db: string | null): void;
  setDatabaseKey(ctx: ContextHandle, s: SettingsHandle, key: string): void;
  setSuppressHealthCheck(s: SettingsHandle, checks: number): void;
  setReportMultipleFailures(s: SettingsHandle, yes: boolean): void;
  setStopOnFirstFailure(s: SettingsHandle, yes: boolean): void;
  setShowStatistics(s: SettingsHandle, yes: boolean): void;
  setUnboundedChoices(s: SettingsHandle, yes: boolean): void;
  setPrintBlob(s: SettingsHandle, yes: boolean): void;
  setPhases(s: SettingsHandle, phases: number): void;
  setBackend(s: SettingsHandle, backend: number): void;
  setNondeterminismStrictness(s: SettingsHandle, strictness: number): void;
  setTestLocation(ctx: ContextHandle, s: SettingsHandle, location: NativeTestLocation): void;
  /** Register a snapshot of `s` as the profile `name`, process-wide. */
  registerProfile(ctx: ContextHandle, name: string, s: SettingsHandle): void;
  /** Make `name` the default profile process-wide; `""` clears it. */
  setDefaultProfile(ctx: ContextHandle, name: string): void;
  /** The resolved `hegel_verbosity_t`. */
  getVerbosity(s: SettingsHandle): number;
  /** The resolved `hegel_phase_t` bit set. */
  getPhases(s: SettingsHandle): number;
  getPrintBlob(s: SettingsHandle): boolean;
  getReportMultipleFailures(s: SettingsHandle): boolean;
  getTestCases(s: SettingsHandle): number;
  /** The fixed seed, or `null` when the engine picks one per run. */
  getSeed(s: SettingsHandle): bigint | null;
  getDerandomize(s: SettingsHandle): boolean;
  /** `null` for the default database, `""` for none, else its directory. */
  getDatabase(s: SettingsHandle): string | null;
  /** The suppressed `hegel_health_check_t` bit set. */
  getSuppressHealthCheck(s: SettingsHandle): number;
  getStopOnFirstFailure(s: SettingsHandle): boolean;
  getShowStatistics(s: SettingsHandle): boolean;
  getUnboundedChoices(s: SettingsHandle): boolean;
  /** The resolved `hegel_backend_t`. */
  getBackend(s: SettingsHandle): number;
  /** The resolved `hegel_nondeterminism_strictness_t`. */
  getNondeterminismStrictness(s: SettingsHandle): number;
  runStart(ctx: ContextHandle, settings: SettingsHandle): RunHandle;
  /**
   * Start a run that replays a reproduce blob instead of exploring. Throws on
   * a blob that is not valid.
   */
  runStartBlob(ctx: ContextHandle, settings: SettingsHandle, blob: string): RunHandle;
  nextTestCase(ctx: ContextHandle, run: RunHandle): TestCaseHandle | null;
  runResult(ctx: ContextHandle, run: RunHandle): RunResultHandle;
  freeRunResult(r: RunResultHandle): void;
  freeRun(run: RunHandle): void;
  freeTestCase(tc: TestCaseHandle): void;
  /** Whether the engine stamped `tc` for capture: its output may be reported. */
  shouldCapture(ctx: ContextHandle, tc: TestCaseHandle): boolean;
  /** A new handle onto an independent choice stream of the same test case. */
  cloneTestCase(ctx: ContextHandle, tc: TestCaseHandle): TestCaseHandle;
  /** A new handle onto `tc`'s choice stream, printing `indent` columns further in. */
  blockTestCase(ctx: ContextHandle, tc: TestCaseHandle, indent: number): TestCaseHandle;
  /** Attribute `tc`'s output lines to concurrent worker `index`. */
  setWorker(ctx: ContextHandle, tc: TestCaseHandle, index: number): void;
  note(ctx: ContextHandle, tc: TestCaseHandle, text: string): void;
  target(ctx: ContextHandle, tc: TestCaseHandle, value: number, label: string): void;
  event(ctx: ContextHandle, tc: TestCaseHandle, label: string): void;
  eventValue(ctx: ContextHandle, tc: TestCaseHandle, value: number, label: string): void;
  /** The span label for a generator identified by `name`. */
  labelFromName(ctx: ContextHandle, name: string): bigint;
  /** The span label for a generator built from others: `labels` hashed in order. */
  labelCombine(ctx: ContextHandle, labels: readonly bigint[]): bigint;
  /** A printer onto `tc`'s region of its family's document, at the default width. */
  testCasePrinter(ctx: ContextHandle, tc: TestCaseHandle): PrinterHandle;
  freePrinter(p: PrinterHandle): void;
  printerText(ctx: ContextHandle, p: PrinterHandle, text: string): void;
  printerHardBreak(ctx: ContextHandle, p: PrinterHandle): void;
  /** Emit `open` and indent later break points `indent` columns further. */
  printerBeginGroup(ctx: ContextHandle, p: PrinterHandle, indent: number, open: string): void;
  /** Undo the innermost group's indentation and emit `close`. */
  printerEndGroup(ctx: ContextHandle, p: PrinterHandle, close: string): void;
  /** A break point: `sep` when its group fits on a line, else a newline. */
  printerBreakable(ctx: ContextHandle, p: PrinterHandle, sep: string): void;
  /** Text emitted only when the innermost group breaks. */
  printerIfBreak(ctx: ContextHandle, p: PrinterHandle, text: string): void;
  /** A comment at the end of the current line, breaking every open group. */
  printerComment(ctx: ContextHandle, p: PrinterHandle, text: string): void;
  /** Shift the indentation of later break points by `delta` columns. */
  printerShiftIndent(ctx: ContextHandle, p: PrinterHandle, delta: number): void;
  /**
   * Emit a one-line representation, letting the engine recover its bracket
   * structure so that it wraps like a value printed structurally.
   */
  printerReflow(ctx: ContextHandle, p: PrinterHandle, text: string): void;
  printerBeginSpeculative(ctx: ContextHandle, p: PrinterHandle): void;
  printerCommitSpeculative(ctx: ContextHandle, p: PrinterHandle): void;
  printerAbortSpeculative(ctx: ContextHandle, p: PrinterHandle): void;
  /** Splice every deferred region (clones, blocks) into the document. */
  printerResolve(ctx: ContextHandle, p: PrinterHandle): void;
  /** Read (and seal) the document of a root printer. */
  printerValue(ctx: ContextHandle, p: PrinterHandle): string;
  generateBoolean(ctx: ContextHandle, tc: TestCaseHandle, p: number): boolean;
  generateInteger(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint;
  generateIntegerBig(ctx: ContextHandle, tc: TestCaseHandle, min: bigint, max: bigint): bigint;
  generateFloat(ctx: ContextHandle, tc: TestCaseHandle, opts: NativeFloatOptions): number;
  /**
   * Draw a byte string with length in `[minSize, maxSize]`. `maxSize` is a raw
   * `max_size`: {@link MAX_SIZE_DEFAULT} for the engine's default maximum,
   * {@link UINT64_MAX} for none.
   */
  generateBytes(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    minSize: number,
    maxSize: bigint,
  ): Uint8Array;
  stringGeneratorText(ctx: ContextHandle, opts: TextGeneratorOptions): StringGeneratorHandle;
  /** `alphabet` is borrowed: the caller still frees it. */
  stringGeneratorRegex(
    ctx: ContextHandle,
    pattern: string,
    fullmatch: boolean,
    alphabet: StringGeneratorHandle | null,
  ): StringGeneratorHandle;
  stringGeneratorEmail(ctx: ContextHandle): StringGeneratorHandle;
  stringGeneratorUrl(ctx: ContextHandle): StringGeneratorHandle;
  stringGeneratorDomain(ctx: ContextHandle, maxLength: number): StringGeneratorHandle;
  generateString(ctx: ContextHandle, tc: TestCaseHandle, generator: StringGeneratorHandle): string;
  /**
   * Start gathering a grammar in Lark's syntax and the files it imports;
   * released with {@link freeGrammarBuilder}. Throws for text that does not parse.
   */
  grammarBuilderNew(ctx: ContextHandle, text: string): GrammarBuilderHandle;
  /**
   * The path of the `index`th file the grammar needs, or `null` past the end
   * of the list (`HEGEL_E_NOT_FOUND`), which grows as files are provided.
   */
  grammarBuilderFile(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    index: number,
  ): string | null;
  /** Supply the `index`th file the grammar needs: its text, or `null` for a missing file. */
  grammarBuilderProvide(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    index: number,
    text: string | null,
  ): void;
  freeGrammarBuilder(builder: GrammarBuilderHandle): void;
  /**
   * Compile a gathered grammar; released with {@link freeGrammar}. Throws with
   * the engine's diagnostic for a bad grammar or option.
   */
  newGrammar(
    ctx: ContextHandle,
    builder: GrammarBuilderHandle,
    opts: GrammarOptions,
  ): GrammarHandle;
  freeGrammar(grammar: GrammarHandle): void;
  /**
   * Start one draw of a string of `grammar`, of at most `maxSize` characters
   * (0 for the grammar's own cap); released with {@link freeGrammarDraw}.
   */
  grammarDrawNew(ctx: ContextHandle, grammar: GrammarHandle, maxSize: number): GrammarDrawHandle;
  /**
   * Advance a grammar draw: the index of the explicit terminal whose value is
   * due, or `null` once the string is complete (`HEGEL_E_NOT_FOUND`). Throws
   * `AssumeError` when the engine rejects the test case.
   */
  grammarDrawNext(ctx: ContextHandle, tc: TestCaseHandle, draw: GrammarDrawHandle): number | null;
  /** Supply the value of the explicit terminal {@link grammarDrawNext} asked for. */
  grammarDrawProvide(ctx: ContextHandle, draw: GrammarDrawHandle, value: string): void;
  /** The finished string, once {@link grammarDrawNext} has returned `null`. */
  grammarDrawResult(ctx: ContextHandle, draw: GrammarDrawHandle): string;
  freeGrammarDraw(draw: GrammarDrawHandle): void;
  generateDate(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDate,
    max: NativeDate,
  ): NativeDate;
  generateTime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeTime,
    max: NativeTime,
  ): NativeTime;
  generateDatetime(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    min: NativeDatetime,
    max: NativeDatetime,
  ): NativeDatetime;
  generateUuid(ctx: ContextHandle, tc: TestCaseHandle, version?: UuidVersion): Uint8Array;
  generateIpv4(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array;
  generateIpv6(ctx: ContextHandle, tc: TestCaseHandle): Uint8Array;
  startSpan(ctx: ContextHandle, tc: TestCaseHandle, label: bigint): void;
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
  /** Put back a variable a consuming {@link poolGenerate} removed from `pool`. */
  poolRestore(ctx: ContextHandle, tc: TestCaseHandle, pool: PoolHandle, variableId: bigint): void;
  freePool(pool: PoolHandle): void;
  /**
   * Open a recursive generation scope for one recursive value; released with
   * {@link freeRecursion}.
   */
  newRecursion(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    maxDepth: number,
    maxLeaves: number,
  ): RecursionHandle;
  /** Whether the sub-value at `depth` is a branch (`true`) or a leaf. */
  recursionBranch(
    ctx: ContextHandle,
    tc: TestCaseHandle,
    recursion: RecursionHandle,
    depth: number,
  ): boolean;
  /**
   * Count one leaf against the attempt's budget: `false` (`HEGEL_E_RETRY`) when
   * the attempt has outgrown it and must be unwound and retried.
   */
  recursionLeaf(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): boolean;
  /**
   * Discard an attempt that outgrew its leaf budget. Throws `AssumeError` once
   * the engine gives up on the test case.
   */
  recursionRetry(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): void;
  /**
   * Report the recursive value finished: `false` (`HEGEL_E_RETRY`) when the
   * engine discarded the attempt and the value must be drawn again.
   */
  recursionFinish(ctx: ContextHandle, tc: TestCaseHandle, recursion: RecursionHandle): boolean;
  freeRecursion(recursion: RecursionHandle): void;
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
  runErrorKind(r: RunResultHandle): NativeRunErrorKind;
  failureCount(r: RunResultHandle): number;
  failure(r: RunResultHandle, index: number): FailureHandle;
  freeFailure(f: FailureHandle): void;
  failureOrigin(fp: FailureHandle): string;
  reproductionBlob(fp: FailureHandle): string | null;
  /** The failure's confirmation caveat, or `null` for a deterministic failure. */
  failureCaveat(fp: FailureHandle): string | null;
  freeStringGenerator(generator: StringGeneratorHandle): void;
}
