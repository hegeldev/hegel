/**
 * Test runner: the `hegel.test` / `hegel.testAsync` entry points, Settings, and
 * the portable test lifecycle driving the engine run loop.
 *
 * @packageDocumentation
 */

import { generateValue, StringGeneratorCache } from "./generate.js";
import {
  TestCase,
  StopTestError,
  AssumeError,
  type DataSource,
  type StateMachineSpec,
} from "./testCase.js";
import {
  EngineError,
  Status,
  RunStatus,
  type Engine,
  type ContextHandle,
  type TestCaseHandle,
  type CollectionHandle,
  type PoolHandle,
  type StateMachineHandle,
} from "./engine.js";
import { portableDiagnostics, type Diagnostics, type RuntimeServices } from "./runtime.js";
import { applySettings, resolveSettings, type Settings, type TestLocation } from "./settings.js";
import { runError } from "./errors.js";
import { callSite, databaseKeyOf, locationOf, type CallSite } from "./callSite.js";

export {
  Verbosity,
  HealthCheck,
  Phase,
  Backend,
  NondeterminismStrictness,
  Database,
} from "./settings.js";
export type { Settings, TestLocation } from "./settings.js";

// ---------------------------------------------------------------------------
// EngineDataSource
// ---------------------------------------------------------------------------

/**
 * {@link DataSource} backed by an opaque engine test case. Schema draws are
 * interpreted by {@link generateValue}; adapters own all ABI marshalling.
 *
 * The engine's collection, pool and state-machine handles behind the
 * {@link DataSource}'s numeric ids are owned by this object — the runner calls
 * {@link dispose} once the test case is over to release them.
 */
export class EngineDataSource implements DataSource {
  private readonly lib: Engine;
  private readonly ctx: ContextHandle;
  private readonly tc: TestCaseHandle;
  private readonly collections: CollectionHandle[] = [];
  private readonly pools: PoolHandle[] = [];
  private readonly machines: StateMachineHandle[] = [];

  private fault: EngineError | undefined;
  private completed = false;
  constructor(
    lib: Engine,
    ctx: ContextHandle,
    tc: TestCaseHandle,
    private readonly cache = new StringGeneratorCache(lib),
    readonly diagnostics: Diagnostics = portableDiagnostics,
    private readonly ownsCache = true,
  ) {
    this.lib = lib;
    this.ctx = ctx;
    this.tc = tc;
  }

  generate(schema: Record<string, unknown>): unknown {
    return this.call(() => generateValue(this.lib, this.ctx, this.tc, schema, this.cache));
  }

  startSpan(label: number): void {
    this.call(() => this.lib.startSpan(this.ctx, this.tc, label));
  }

  stopSpan(discard: boolean): void {
    this.call(() => this.lib.stopSpan(this.ctx, this.tc, discard));
  }

  newCollection(minSize: number, maxSize?: number): number {
    this.collections.push(
      this.call(() => this.lib.newCollection(this.ctx, this.tc, minSize, maxSize)),
    );
    return this.collections.length - 1;
  }

  collectionMore(collectionId: number): boolean {
    return this.call(() =>
      this.lib.collectionMore(this.ctx, this.tc, this.collections[collectionId]),
    );
  }

  collectionReject(collectionId: number, why?: string): void {
    this.call(() =>
      this.lib.collectionReject(this.ctx, this.tc, this.collections[collectionId], why ?? null),
    );
  }

  markComplete(status: number, origin: string | null): void {
    if (this.completed) throw new EngineError("Test case completion was already attempted");
    this.completed = true;
    this.call(() => this.lib.markComplete(this.ctx, this.tc, status, origin));
  }

  newPool(): number {
    this.pools.push(this.call(() => this.lib.newPool(this.ctx, this.tc)));
    return this.pools.length - 1;
  }

  poolAdd(poolId: number): bigint {
    return this.call(() => this.lib.poolAdd(this.ctx, this.tc, this.pools[poolId]));
  }

  poolGenerate(poolId: number, consume: boolean): bigint {
    return this.call(() => this.lib.poolGenerate(this.ctx, this.tc, this.pools[poolId], consume));
  }

  newStateMachine(spec: StateMachineSpec): number {
    this.machines.push(
      this.call(() =>
        this.lib.newStateMachine(this.ctx, this.tc, {
          ruleNames: spec.ruleNames,
          // One concurrency group, so rules never need to be kept apart.
          ruleGroups: spec.ruleNames.map(() => 0),
          invariantNames: spec.invariantNames,
          invariantAlwaysCheck: spec.invariantAlwaysCheck,
          minConcurrency: 1,
          maxConcurrency: 1,
          stepCount: spec.stepCount,
        }),
      ),
    );
    return this.machines.length - 1;
  }

  stateMachineNextRound(machineId: number): boolean {
    // With a single group the id it reports carries no information; only
    // whether the machine has another round to run matters.
    return this.call(
      () => this.lib.stateMachineNextGroup(this.ctx, this.tc, this.machines[machineId]) !== null,
    );
  }

  stateMachineNextRule(machineId: number): number | null {
    return this.call(() =>
      this.lib.stateMachineNextRule(this.ctx, this.tc, this.machines[machineId], 0),
    );
  }

  stateMachineRuleRejected(machineId: number): void {
    this.call(() =>
      this.lib.stateMachineRuleRejected(this.ctx, this.tc, this.machines[machineId], 0),
    );
  }

  stateMachineShouldCheckInvariant(machineId: number, invariantIndex: number): boolean {
    return this.call(() =>
      this.lib.stateMachineShouldCheckInvariant(
        this.ctx,
        this.tc,
        this.machines[machineId],
        invariantIndex,
      ),
    );
  }

  assertHealthy(): void {
    if (this.fault !== undefined) throw this.fault;
  }

  private call<T>(operation: () => T): T {
    try {
      return operation();
    } catch (error) {
      if (error instanceof AssumeError || error instanceof StopTestError) throw error;
      const fault =
        error instanceof EngineError ? error : new EngineError(String(error), { cause: error });
      this.fault ??= fault;
      throw fault;
    }
  }

  /** Release every owned handle even when a destructor fails. */
  dispose(): void {
    const errors: unknown[] = [];
    const release = <H>(handles: H[], free: (handle: H) => void): void => {
      for (const handle of handles.splice(0)) {
        try {
          free(handle);
        } catch (error) {
          errors.push(error);
        }
      }
    };
    release(this.collections, (collection) => this.lib.freeCollection(collection));
    release(this.pools, (pool) => this.lib.freePool(pool));
    release(this.machines, (machine) => this.lib.freeStateMachine(machine));
    try {
      if (this.ownsCache) this.cache.dispose();
    } catch (error) {
      errors.push(error);
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) throw new AggregateError(errors, "Test case cleanup failed");
  }
}

// ---------------------------------------------------------------------------
// Per-test-case execution
// ---------------------------------------------------------------------------

export type TestCaseResult =
  | { status: "valid" }
  | { status: "invalid" }
  | { status: "overrun" }
  | { status: "interesting"; error: unknown; origin: string };

const RESULT_TO_STATUS: Record<TestCaseResult["status"], number> = {
  valid: Status.VALID,
  invalid: Status.INVALID,
  overrun: Status.OVERRUN,
  interesting: Status.INTERESTING,
};

/**
 * Extract a stable origin for a thrown error: the first stack frame outside
 * `node_modules` (the user's test code). The shrinker groups failing inputs by
 * this origin, so it must be stable across calls.
 */
function extractOrigin(error: unknown): string {
  if (!(error instanceof Error) || !error.stack) return "<unknown>";
  const lines = error.stack.split("\n");
  for (const line of lines) {
    const trimmed = line.trim();
    if (
      (trimmed.startsWith("at ") || /@.+:\d+(?::\d+)?$/.test(trimmed)) &&
      !trimmed.includes("node_modules")
    ) {
      return trimmed;
    }
  }
  return "<unknown>";
}

function classifyResult(
  e: unknown,
  isFinal: boolean,
  diagnostics: Diagnostics,
): { result: TestCaseResult; origin: string | null } {
  if (e instanceof AssumeError) return { result: { status: "invalid" }, origin: null };
  if (e instanceof StopTestError) return { result: { status: "overrun" }, origin: null };

  if (e instanceof EngineError) throw e;
  if (isFinal) diagnostics.reportFinalError(e);
  const origin = extractOrigin(e);
  return { result: { status: "interesting", error: e, origin }, origin };
}

function finalizeTestCase(
  dataSource: DataSource,
  result: TestCaseResult,
  origin: string | null,
): void {
  dataSource.markComplete(RESULT_TO_STATUS[result.status], origin);
}

export async function runTestCaseAsync(
  dataSource: DataSource,
  testFn: (tc: TestCase) => void | Promise<void>,
  isFinal: boolean,
): Promise<TestCaseResult> {
  const tc = new TestCase(dataSource, isFinal);

  let result: TestCaseResult;
  let origin: string | null = null;

  try {
    await testFn(tc);
    dataSource.assertHealthy?.();
    result = { status: "valid" };
  } catch (e: unknown) {
    try {
      dataSource.assertHealthy?.();
      ({ result, origin } = classifyResult(
        e,
        isFinal,
        dataSource.diagnostics ?? portableDiagnostics,
      ));
    } catch (fault) {
      // Abort the active case, but never mark it interesting or retry completion.
      try {
        dataSource.markComplete(Status.INVALID, null);
      } catch (completionError) {
        throw new AggregateError(
          [fault, completionError],
          "Engine failure during case completion",
          { cause: completionError },
        );
      }
      throw fault;
    }
  }

  finalizeTestCase(dataSource, result, origin);
  return result;
}

export function runTestCase(
  dataSource: DataSource,
  testFn: (tc: TestCase) => void,
  isFinal: boolean,
): TestCaseResult {
  const tc = new TestCase(dataSource, isFinal);

  let result: TestCaseResult;
  let origin: string | null = null;

  try {
    testFn(tc);
    dataSource.assertHealthy?.();
    result = { status: "valid" };
  } catch (e: unknown) {
    try {
      dataSource.assertHealthy?.();
      ({ result, origin } = classifyResult(
        e,
        isFinal,
        dataSource.diagnostics ?? portableDiagnostics,
      ));
    } catch (fault) {
      // Abort the active case, but never mark it interesting or retry completion.
      try {
        dataSource.markComplete(Status.INVALID, null);
      } catch (completionError) {
        throw new AggregateError(
          [fault, completionError],
          "Engine failure during case completion",
          { cause: completionError },
        );
      }
      throw fault;
    }
  }

  finalizeTestCase(dataSource, result, origin);
  return result;
}

/** Preserve the pending failure while attempting every destructor in ownership order. */
function finishCleanup(errors: unknown[], ...operations: (() => void)[]): void {
  for (const operation of operations) {
    try {
      operation();
    } catch (error) {
      errors.push(error);
    }
  }
  if (errors.length === 1) throw errors[0];
  if (errors.length > 1) throw new AggregateError(errors, "Hegel run cleanup failed");
}

/**
 * Throw the errors of a failed run: the test's own error when there is one,
 * else an `AggregateError` of every distinct failure.
 */
function throwFailures(errors: unknown[]): never {
  if (errors.length === 1) throw errors[0];
  throw new AggregateError(errors, `Hegel found ${errors.length} distinct failures`);
}

let installedRuntime: RuntimeServices | null = null;

/**
 * Set the runtime a {@link Hegel} built without one runs on: what the Node and
 * browser entry points do when loaded.
 *
 * @internal
 */
export function installRuntime(runtime: RuntimeServices): void {
  installedRuntime = runtime;
}

/**
 * A property-based test, configured with builder methods and then run with
 * {@link Hegel.run | run} (for an asynchronous test body) or
 * {@link Hegel.runSync | runSync}. `hegel.test` and `hegel.testAsync` are
 * shorthands for a test that needs no more than settings.
 *
 * @example
 * ```ts
 * new hegel.Hegel((tc) => {
 *   tc.draw(gs.integers());
 * })
 *   .settings({ testCases: 500 })
 *   .databaseKey("integers")
 *   .runSync();
 * ```
 */
export class Hegel {
  private readonly testFn: (tc: TestCase) => void | Promise<void>;
  private readonly runtime: RuntimeServices;
  private readonly site: CallSite | null;
  private _settings: Settings = {};
  private _testLocation: TestLocation | null = null;
  private _databaseKey: string | null = null;

  constructor(testFn: (tc: TestCase) => void | Promise<void>);
  /** @internal */
  constructor(
    testFn: (tc: TestCase) => void | Promise<void>,
    runtime: RuntimeServices,
    site: CallSite | null,
  );
  constructor(
    testFn: (tc: TestCase) => void | Promise<void>,
    runtime?: RuntimeServices,
    site?: CallSite | null,
  ) {
    const resolved = runtime ?? installedRuntime;
    if (resolved === null) throw new Error("Hegel has no runtime: import @hegeldev/hegel first");
    this.testFn = testFn;
    this.runtime = resolved;
    this.site = site === undefined ? callSite(new.target) : site;
  }

  /** Set settings over the engine's resolved ones. Returns this for chaining. */
  settings(s: Settings): this {
    Object.assign(this._settings, s);
    return this;
  }

  /**
   * Set where the test is defined, in place of the location derived from the
   * call site. Inside Antithesis, the engine reports the run's verdict as an
   * `always` assertion at this location.
   */
  testLocation(location: TestLocation): this {
    this._testLocation = location;
    return this;
  }

  /**
   * Set the key the test's failures are stored under in the database, in
   * place of the one derived from its location (its file plus its name).
   */
  databaseKey(key: string): this {
    this._databaseKey = key;
    return this;
  }

  private location(): TestLocation | null {
    if (this._testLocation !== null) return this._testLocation;
    return this.site === null ? null : locationOf(this.site, this.testFn);
  }

  /**
   * Generator that drives the libhegel run loop, yielding a
   * {@link EngineDataSource} (plus an `isFinal` flag) for each test case the
   * driver ({@link run} or {@link runSync}) should execute the body against. The
   * driver runs the user's body and resumes the generator with the
   * {@link TestCaseResult}. This factoring lets the sync and async drivers share
   * one loop implementation.
   *
   * The engine only *explores* (generate / shrink), so every pumped case is
   * non-final. The client owns the final replays: once the loop drains and the
   * run has failed, each discovered counterexample's reproduce blob is replayed
   * (via `hegel_test_case_from_blob`) as a final case, so that the test's own
   * error is what the run throws.
   */
  private *runSteps(): Generator<{ ds: EngineDataSource; isFinal: boolean }, void, TestCaseResult> {
    this.runtime.validateSettings(this._settings);
    const lib = this.runtime.getEngine();
    const ctx = lib.newContext();
    const contextErrors: unknown[] = [];
    try {
      const settings = resolveSettings(lib, ctx, this._settings);
      const cache = new StringGeneratorCache(lib);
      const settingsErrors: unknown[] = [];
      try {
        applySettings(lib, ctx, settings, this._settings);
        if (!this.runtime.persistence) lib.setDatabase(ctx, settings, "");
        const location = this.location();
        if (location !== null) {
          lib.setTestLocation(ctx, settings, {
            file: location.file,
            beginLine: location.beginLine,
            className: location.class,
            function: location.function,
          });
        }
        const key =
          this._databaseKey ??
          (location !== null ? databaseKeyOf(location) : this.testFn.name || null);
        if (key !== null) lib.setDatabaseKey(ctx, settings, key);
        const run = lib.runStart(ctx, settings);
        const runErrors: unknown[] = [];
        const explored = new Map<string, unknown>();
        try {
          for (;;) {
            const tc = lib.nextTestCase(ctx, run);
            if (tc === null) break;
            const ds = new EngineDataSource(lib, ctx, tc, cache, this.runtime, false);
            try {
              const outcome = yield { ds, isFinal: false };
              if (outcome.status === "interesting") explored.set(outcome.origin, outcome.error);
            } finally {
              finishCleanup(
                [],
                () => ds.dispose(),
                () => lib.freeTestCase(tc),
              );
            }
          }

          const result = lib.runResult(ctx, run);
          const resultErrors: unknown[] = [];
          try {
            const status = lib.runStatus(result);
            if (status === RunStatus.PASSED) {
              return;
            }
            if (status === RunStatus.ERROR) {
              throw runError(lib.runError(result), lib.runErrorKind(result));
            }
            if (status !== RunStatus.FAILED) {
              throw new EngineError(`Unknown Hegel run status: ${status}`);
            }
            // RunStatus.FAILED: replay each distinct counterexample's blob as a
            // final, client-owned case. A genuine counterexample re-fails on replay,
            // so the body throws its own error again, which is what the run throws.
            const count = lib.failureCount(result);
            const errors: unknown[] = [];
            for (let i = 0; i < count; i++) {
              const failure = lib.failure(result, i);
              let blob: string | null = null;
              let origin = "";
              let caveat: string | null = null;
              const failureErrors: unknown[] = [];
              try {
                origin = lib.failureOrigin(failure);
                blob = lib.reproductionBlob(failure);
                caveat = lib.failureCaveat(failure);
              } catch (error) {
                failureErrors.push(error);
              } finally {
                finishCleanup(failureErrors, () => lib.freeFailure(failure));
              }
              if (caveat !== null) this.runtime.note(caveat);
              if (blob === null) {
                if (caveat !== null && explored.has(origin)) {
                  errors.push(explored.get(origin));
                  continue;
                }
                throw new EngineError(
                  "Hegel failure has no reproduction blob; deterministic replay is unavailable",
                );
              }
              const replayTc = lib.testCaseFromBlob(ctx, settings, blob);
              const ds = new EngineDataSource(lib, ctx, replayTc, cache, this.runtime, false);
              const replayErrors: unknown[] = [];
              try {
                const replay = yield { ds, isFinal: true };
                if (replay.status === "interesting") {
                  errors.push(replay.error);
                } else if (caveat !== null && explored.has(origin)) {
                  errors.push(explored.get(origin));
                } else {
                  throw new Error(
                    `Hegel failure did not reproduce during final replay (status: ${replay.status})`,
                  );
                }
              } catch (error) {
                replayErrors.push(error);
              } finally {
                finishCleanup(
                  replayErrors,
                  () => ds.dispose(),
                  () => lib.freeTestCase(replayTc),
                );
              }
            }
            throwFailures(errors);
          } catch (error) {
            resultErrors.push(error);
          } finally {
            finishCleanup(resultErrors, () => lib.freeRunResult(result));
          }
        } catch (error) {
          runErrors.push(error);
        } finally {
          finishCleanup(runErrors, () => lib.freeRun(run));
        }
      } catch (error) {
        settingsErrors.push(error);
      } finally {
        finishCleanup(
          settingsErrors,
          () => cache.dispose(),
          () => lib.freeSettings(settings),
        );
      }
    } catch (error) {
      contextErrors.push(error);
    } finally {
      finishCleanup(contextErrors, () => lib.freeContext(ctx));
    }
  }

  /** Run the test, awaiting an asynchronous test body. */
  async run(): Promise<void> {
    const gen = this.runSteps();
    const errors: unknown[] = [];
    try {
      let next = gen.next();
      while (!next.done) {
        const { ds, isFinal } = next.value;
        const result = await runTestCaseAsync(ds, this.testFn, isFinal);
        next = gen.next(result);
      }
    } catch (error) {
      errors.push(error);
    } finally {
      finishCleanup(errors, () => {
        gen.return();
      });
    }
  }

  /** Run the test synchronously. The test body must not be asynchronous. */
  runSync(): void {
    const gen = this.runSteps();
    const errors: unknown[] = [];
    try {
      let next = gen.next();
      while (!next.done) {
        const { ds, isFinal } = next.value;
        const result = runTestCase(ds, this.testFn as (tc: TestCase) => void, isFinal);
        next = gen.next(result);
      }
    } catch (error) {
      errors.push(error);
    } finally {
      finishCleanup(errors, () => {
        gen.return();
      });
    }
  }
}

/** Compose the same synchronous and asynchronous drivers with a runtime. */
export interface Runner {
  test(testFn: (tc: TestCase) => void, settings?: Settings): void;
  testAsync(testFn: (tc: TestCase) => void | Promise<void>, settings?: Settings): Promise<void>;
  registerProfile(name: string, settings?: Settings): void;
  setDefaultProfile(name: string | null): void;
  Hegel: new (testFn: (tc: TestCase) => void | Promise<void>) => Hegel;
}

export function createRunner(runtime: RuntimeServices): Runner {
  class RuntimeHegel extends Hegel {
    constructor(
      testFn: (tc: TestCase) => void | Promise<void>,
      site: CallSite | null = callSite(new.target),
    ) {
      super(testFn, runtime, site);
    }
  }

  function withContext(body: (lib: Engine, ctx: ContextHandle) => void): void {
    const lib = runtime.getEngine();
    const ctx = lib.newContext();
    const errors: unknown[] = [];
    try {
      body(lib, ctx);
    } catch (error) {
      errors.push(error);
    } finally {
      finishCleanup(errors, () => lib.freeContext(ctx));
    }
  }

  /**
   * Run a property-based test.
   *
   * If your property is async, see {@link testAsync} instead.
   *
   * @example
   * ```ts
   * import { test } from 'vitest';
   * import * as hegel from '@hegeldev/hegel';
   * import * as gs from '@hegeldev/hegel/generators';
   *
   * test('addition is commutative', () =>
   *   hegel.test((tc) => {
   *     const x = tc.draw(gs.integers());
   *     const y = tc.draw(gs.integers());
   *     expect(x + y).toBe(y + x);
   *   }),
   * );
   * ```
   */
  function test(testFn: (tc: TestCase) => void, settings?: Settings): void {
    if (testFn.constructor.name === "AsyncFunction") {
      throw new TypeError("hegel.test received an async test body. Use hegel.testAsync instead.");
    }
    const h = new RuntimeHegel(testFn, callSite(test));
    if (settings) h.settings(settings);
    h.runSync();
  }

  /**
   * Run a property-based test with an asynchronous test body.
   *
   * Returns a `Promise<void>` that resolves when the test completes and
   * rejects if any test case fails.
   *
   * @example
   * ```ts
   * import { test } from 'vitest';
   * import * as hegel from '@hegeldev/hegel';
   * import * as gs from '@hegeldev/hegel/generators';
   *
   * test('my async test', () =>
   *   hegel.testAsync(async (tc) => {
   *     const x = tc.draw(gs.integers());
   *     const result = await someAsyncOperation(x);
   *     expect(result).toEqual(expected(x));
   *   }),
   * );
   * ```
   */
  async function testAsync(
    testFn: (tc: TestCase) => void | Promise<void>,
    settings?: Settings,
  ): Promise<void> {
    const h = new RuntimeHegel(testFn, callSite(testAsync));
    if (settings) h.settings(settings);
    await h.run();
  }

  /**
   * Register `settings` as the profile `name`, process-wide, replacing any
   * earlier registration. The profile snapshots the settings the engine
   * resolves for `settings` (its `profile`, else the default one) with the
   * given fields set over them. Select it with the `profile` setting, or make
   * it the default with {@link setDefaultProfile}; a `hegel.toml` section for
   * `name` still applies over it.
   */
  function registerProfile(name: string, settings: Settings = {}): void {
    withContext((lib, ctx) => {
      const handle = resolveSettings(lib, ctx, settings);
      const errors: unknown[] = [];
      try {
        applySettings(lib, ctx, handle, settings);
        lib.registerProfile(ctx, name, handle);
      } catch (error) {
        errors.push(error);
      } finally {
        finishCleanup(errors, () => lib.freeSettings(handle));
      }
    });
  }

  /**
   * Make `name` the default profile for the whole process, over
   * `HEGEL_DEFAULT_PROFILE`, `hegel.toml` and environment detection; `null`
   * clears an earlier call.
   */
  function setDefaultProfile(name: string | null): void {
    withContext((lib, ctx) => lib.setDefaultProfile(ctx, name ?? ""));
  }

  return { test, testAsync, registerProfile, setDefaultProfile, Hegel: RuntimeHegel };
}
