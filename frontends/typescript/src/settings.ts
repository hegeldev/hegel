/**
 * Run settings. Every field is optional: a run starts from the engine's
 * resolved settings (the selected profile, `hegel.toml` and the `HEGEL_*`
 * environment variables) and only the fields given here are set over them.
 *
 * @packageDocumentation
 */

import {
  NativeBackend,
  NativeHealthCheck,
  NativeNondeterminismStrictness,
  NativePhase,
  NativeVerbosity,
  type ContextHandle,
  type Engine,
  type SettingsHandle,
} from "./engine.js";

/** How much the engine prints while a test runs. */
export enum Verbosity {
  Quiet = "quiet",
  Normal = "normal",
  Verbose = "verbose",
  Debug = "debug",
}

/** A health check, for {@link Settings.suppressHealthCheck}. */
export enum HealthCheck {
  FilterTooMuch = "filter_too_much",
  TooSlow = "too_slow",
  TestCasesTooLarge = "test_cases_too_large",
  LargeInitialTestCase = "large_initial_test_case",
}

/** A phase of a run, for {@link Settings.phases}. */
export enum Phase {
  /** Run explicit examples. */
  Explicit = "explicit",
  /** Replay failures saved in the database by earlier runs. */
  Reuse = "reuse",
  /** Generate fresh test cases. */
  Generate = "generate",
  /** Optimise towards targeted scores. */
  Target = "target",
  /** Shrink the failures found. */
  Shrink = "shrink",
}

/** The source of randomness the engine draws from. */
export enum Backend {
  /** A seeded PRNG: runs are reproducible from their seed. */
  Default = "default",
  /**
   * Fresh entropy on every draw (from Antithesis when running under it, else
   * the operating system). Selected by the shipped `workload` profile; you
   * almost certainly don't want it otherwise.
   */
  Urandom = "urandom",
}

/** How a run reacts when the test behaves nondeterministically. */
export enum NondeterminismStrictness {
  /** Confirm failures by repeated replay, silently. */
  Quiet = "quiet",
  /** As `Quiet`, printing a one-line notice once per run. */
  Warn = "warn",
  /** Abort the run with a {@link FlakyError}. */
  Error = "error",
}

/**
 * Where failing examples are stored between runs. A run with no `database`
 * setting uses whatever the engine resolved: `.hegel/examples` locally,
 * disabled by the shipped `ci` and `workload` profiles.
 */
export type Database = { kind: "default" } | { kind: "disabled" } | { kind: "path"; path: string };

export const Database = {
  /** The engine's default location, `.hegel/examples`, whatever the profile says. */
  default: { kind: "default" } as Database,
  /** No database: failures are neither saved nor replayed. */
  disabled: { kind: "disabled" } as Database,
  /** A database rooted at `path`, created if it does not exist. */
  fromPath: (path: string): Database => ({ kind: "path", path }),
};

/**
 * Settings for a run. Every field is optional; a field left out keeps the
 * value the engine resolved from the settings profile (see {@link profile}),
 * `hegel.toml` and the `HEGEL_*` environment variables.
 */
export interface Settings {
  /**
   * The profile to start from instead of the default one: a shipped profile
   * (`development`, `ci`, `workload`), one defined in `hegel.toml`, or one
   * registered with {@link registerProfile}. `base` is the plain base
   * settings.
   */
  profile?: string;
  /** The number of valid test cases to run. 100 in the base settings. */
  testCases?: number;
  /**
   * A fixed seed, an integer in `[0, 2^64)`, so that a run is reproducible;
   * `null` clears a seed set by the profile.
   */
  seed?: number | bigint | null;
  /** How much the engine prints. */
  verbosity?: Verbosity;
  /**
   * Derive the seed from the test's identity rather than fresh randomness,
   * when no seed is set. On by default in the `ci` profile.
   */
  derandomize?: boolean;
  /** Where failing examples are stored between runs. */
  database?: Database;
  /** The health checks to turn off. `[]` turns every one back on. */
  suppressHealthCheck?: HealthCheck[];
  /** Keep searching after the first failure, and report every distinct one. */
  reportMultipleFailures?: boolean;
  /**
   * Stop generating once a failure is found (the default). With `false`, the
   * run uses its whole `testCases` budget first.
   */
  stopOnFirstFailure?: boolean;
  /** The phases to run. All of them by default. */
  phases?: Phase[];
  /** Print a reproduction blob with each failure. On by default. */
  printBlob?: boolean;
  /** Print statistics of the test's events at the end of the run. */
  showStatistics?: boolean;
  /**
   * Let a test case make any number of choices. By default one is cut off as
   * too large after 2^20.
   */
  unboundedChoices?: boolean;
  /** How the run reacts to a nondeterministic test. */
  nondeterminismStrictness?: NondeterminismStrictness;
  /** The source of randomness. */
  backend?: Backend;
}

/**
 * Where a test is defined: reported to [Antithesis](https://antithesis.com/)
 * as the location of the test's `always` assertion when running inside it.
 */
export interface TestLocation {
  /** The name of the test function. */
  function: string;
  /** The source file the test is defined in. */
  file: string;
  /** The module (or class) enclosing the test. */
  class: string;
  /** The line in `file` where the test's definition begins. */
  beginLine: number;
}

const VERBOSITY_TO_NATIVE: Record<Verbosity, number> = {
  [Verbosity.Quiet]: NativeVerbosity.QUIET,
  [Verbosity.Normal]: NativeVerbosity.NORMAL,
  [Verbosity.Verbose]: NativeVerbosity.VERBOSE,
  [Verbosity.Debug]: NativeVerbosity.DEBUG,
};

export const HEALTH_CHECK_TO_NATIVE: Record<HealthCheck, number> = {
  [HealthCheck.FilterTooMuch]: NativeHealthCheck.FILTER_TOO_MUCH,
  [HealthCheck.TooSlow]: NativeHealthCheck.TOO_SLOW,
  [HealthCheck.TestCasesTooLarge]: NativeHealthCheck.TEST_CASES_TOO_LARGE,
  [HealthCheck.LargeInitialTestCase]: NativeHealthCheck.LARGE_INITIAL_TEST_CASE,
};

const PHASE_TO_NATIVE: Record<Phase, number> = {
  [Phase.Explicit]: NativePhase.EXPLICIT,
  [Phase.Reuse]: NativePhase.REUSE,
  [Phase.Generate]: NativePhase.GENERATE,
  [Phase.Target]: NativePhase.TARGET,
  [Phase.Shrink]: NativePhase.SHRINK,
};

const BACKEND_TO_NATIVE: Record<Backend, number> = {
  [Backend.Default]: NativeBackend.DEFAULT,
  [Backend.Urandom]: NativeBackend.URANDOM,
};

const STRICTNESS_TO_NATIVE: Record<NondeterminismStrictness, number> = {
  [NondeterminismStrictness.Quiet]: NativeNondeterminismStrictness.QUIET,
  [NondeterminismStrictness.Warn]: NativeNondeterminismStrictness.WARN,
  [NondeterminismStrictness.Error]: NativeNondeterminismStrictness.ERROR,
};

function lookup<K extends string>(table: Record<K, number>, value: K, setting: string): number {
  const native = Object.prototype.hasOwnProperty.call(table, value) ? table[value] : undefined;
  if (native === undefined) throw new TypeError(`Invalid ${setting}: ${String(value)}`);
  return native;
}

function mask<K extends string>(table: Record<K, number>, values: K[], setting: string): number {
  return values.reduce((bits, value) => bits | lookup(table, value, setting), 0);
}

function nativeSeed(seed: number | bigint | null): bigint | null {
  if (seed === null) return null;
  if (typeof seed === "number" && !Number.isSafeInteger(seed)) {
    throw new RangeError(`Invalid seed ${seed}: expected an integer in [0, 2^64)`);
  }
  const value = BigInt(seed);
  if (value < 0n || value > 0xffffffffffffffffn) {
    throw new RangeError(`Invalid seed ${seed}: expected an integer in [0, 2^64)`);
  }
  return value;
}

/** Create a settings handle from the engine's resolved profile. */
export function resolveSettings(lib: Engine, ctx: ContextHandle, s: Settings): SettingsHandle {
  return s.profile === undefined ? lib.newSettings(ctx) : lib.newSettingsForProfile(ctx, s.profile);
}

/**
 * Set every field the user gave on `handle`, and nothing else. Throws for an
 * invalid value before the engine sees it.
 */
export function applySettings(
  lib: Engine,
  ctx: ContextHandle,
  handle: SettingsHandle,
  s: Settings,
): void {
  if (s.testCases !== undefined) lib.setTestCases(handle, s.testCases);
  if (s.seed !== undefined) lib.setSeed(handle, nativeSeed(s.seed));
  if (s.verbosity !== undefined)
    lib.setVerbosity(handle, lookup(VERBOSITY_TO_NATIVE, s.verbosity, "verbosity"));
  if (s.derandomize !== undefined) lib.setDerandomize(handle, s.derandomize);
  if (s.database !== undefined) {
    const db = s.database;
    lib.setDatabase(ctx, handle, db.kind === "default" ? null : db.kind === "path" ? db.path : "");
  }
  if (s.suppressHealthCheck !== undefined)
    lib.setSuppressHealthCheck(
      handle,
      mask(HEALTH_CHECK_TO_NATIVE, s.suppressHealthCheck, "health check"),
    );
  if (s.reportMultipleFailures !== undefined)
    lib.setReportMultipleFailures(handle, s.reportMultipleFailures);
  if (s.stopOnFirstFailure !== undefined) lib.setStopOnFirstFailure(handle, s.stopOnFirstFailure);
  if (s.phases !== undefined) lib.setPhases(handle, mask(PHASE_TO_NATIVE, s.phases, "phase"));
  if (s.printBlob !== undefined) lib.setPrintBlob(handle, s.printBlob);
  if (s.showStatistics !== undefined) lib.setShowStatistics(handle, s.showStatistics);
  if (s.unboundedChoices !== undefined) lib.setUnboundedChoices(handle, s.unboundedChoices);
  if (s.nondeterminismStrictness !== undefined)
    lib.setNondeterminismStrictness(
      handle,
      lookup(STRICTNESS_TO_NATIVE, s.nondeterminismStrictness, "nondeterminism strictness"),
    );
  if (s.backend !== undefined)
    lib.setBackend(handle, lookup(BACKEND_TO_NATIVE, s.backend, "backend"));
}
