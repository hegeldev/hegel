/**
 * Test runner: the `hegel.test` / `hegel.testAsync` entry points, Settings, and
 * the portable test lifecycle driving the engine run loop.
 *
 * @packageDocumentation
 */

import { generateValue, StringGeneratorCache, withCleanup, type GrammarSpec } from "./generate.js";
import {
  TestCase,
  StopTestError,
  AssumeError,
  LoopDone,
  ExplicitValues,
  ExplicitExampleError,
  type DataSource,
  type StateMachineSpec,
} from "./testCase.js";
import {
  EngineError,
  Status,
  RunStatus,
  NativePhase,
  NativeVerbosity,
  type Engine,
  type ContextHandle,
  type SettingsHandle,
  type RunHandle,
  type TestCaseHandle,
  type FailureHandle,
  type CollectionHandle,
  type PoolHandle,
  type RecursionHandle,
  type StateMachineHandle,
  type PrinterHandle,
} from "./engine.js";
import type { RuntimeServices } from "./runtime.js";
import { applySettings, resolveSettings, type Settings, type TestLocation } from "./settings.js";
import { runError, DidNotReproduceError } from "./errors.js";
import { resolveLabel, type LabelSpec } from "./labels.js";
import {
  EngineDocument,
  anyLive,
  enginePrinter,
  installDocumentEngine,
  printedLine,
  splitLines,
  type Printer,
} from "./pretty.js";
import { ExplicitDataSource } from "./explicit.js";
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
 * What every {@link EngineDataSource} of one test case shares: the engine
 * handles behind the {@link DataSource}'s numeric ids, the handles derived
 * from the test case's own (clones, blocks, printers), and the first engine
 * fault any of them hit.
 */
interface Family {
  readonly cache: StringGeneratorCache;
  readonly ownsCache: boolean;
  readonly collections: CollectionHandle[];
  readonly pools: PoolHandle[];
  readonly recursions: RecursionHandle[];
  readonly machines: StateMachineHandle[];
  readonly derived: TestCaseHandle[];
  readonly printers: PrinterHandle[];
  fault: EngineError | undefined;
  /** The deferred printer regions opened in the test case's document. */
  readonly regions: PrinterHandle[];
  /** Whether the document has been read or released: printing then does nothing. */
  released: boolean;
}

/**
 * {@link DataSource} backed by an opaque engine test case. Schema draws are
 * interpreted by {@link generateValue}; adapters own all ABI marshalling.
 *
 * The engine's collection, pool, recursion, state-machine, printer and derived test-case
 * handles behind this data source and those derived from it by
 * {@link clone} / {@link block} are owned by the root one: the runner calls
 * {@link dispose} on it once the test case is over to release them.
 */
export class EngineDataSource implements DataSource {
  readonly family: Family;
  private completed = false;

  constructor(
    private readonly lib: Engine,
    private readonly ctx: ContextHandle,
    private readonly tc: TestCaseHandle,
    cache = new StringGeneratorCache(lib),
    ownsCache = true,
    family?: Family,
  ) {
    this.family = family ?? {
      cache,
      ownsCache,
      collections: [],
      pools: [],
      recursions: [],
      machines: [],
      derived: [],
      printers: [],
      fault: undefined,
      regions: [],
      released: false,
    };
  }

  generate(schema: Record<string, unknown>): unknown {
    return this.call(() => generateValue(this.lib, this.ctx, this.tc, schema, this.family.cache));
  }

  label(spec: LabelSpec): bigint {
    return this.call(() => resolveLabel(this.lib, this.ctx, spec));
  }

  startSpan(label: bigint): void {
    this.call(() => this.lib.startSpan(this.ctx, this.tc, label));
  }

  stopSpan(discard: boolean): void {
    this.call(() => this.lib.stopSpan(this.ctx, this.tc, discard));
  }

  newCollection(minSize: number, maxSize?: number): number {
    this.family.collections.push(
      this.call(() => this.lib.newCollection(this.ctx, this.tc, minSize, maxSize)),
    );
    return this.family.collections.length - 1;
  }

  collectionMore(collectionId: number): boolean {
    return this.call(() =>
      this.lib.collectionMore(this.ctx, this.tc, this.family.collections[collectionId]),
    );
  }

  collectionReject(collectionId: number, why?: string): void {
    this.call(() =>
      this.lib.collectionReject(
        this.ctx,
        this.tc,
        this.family.collections[collectionId],
        why ?? null,
      ),
    );
  }

  markComplete(status: number, origin: string | null): void {
    if (this.completed) throw new EngineError("Test case completion was already attempted");
    this.completed = true;
    this.call(() => this.lib.markComplete(this.ctx, this.tc, status, origin));
  }

  newPool(): number {
    this.family.pools.push(this.call(() => this.lib.newPool(this.ctx, this.tc)));
    return this.family.pools.length - 1;
  }

  poolAdd(poolId: number): bigint {
    return this.call(() => this.lib.poolAdd(this.ctx, this.tc, this.family.pools[poolId]));
  }

  poolGenerate(poolId: number, consume: boolean): bigint {
    return this.call(() =>
      this.lib.poolGenerate(this.ctx, this.tc, this.family.pools[poolId], consume),
    );
  }

  poolRestore(poolId: number, variableId: bigint): void {
    this.call(() => this.lib.poolRestore(this.ctx, this.tc, this.family.pools[poolId], variableId));
  }

  newRecursion(maxDepth: number, maxLeaves: number): number {
    this.family.recursions.push(
      this.call(() => this.lib.newRecursion(this.ctx, this.tc, maxDepth, maxLeaves)),
    );
    return this.family.recursions.length - 1;
  }

  recursionBranch(recursionId: number, depth: number): boolean {
    return this.call(() =>
      this.lib.recursionBranch(this.ctx, this.tc, this.family.recursions[recursionId], depth),
    );
  }

  recursionLeaf(recursionId: number): boolean {
    return this.call(() =>
      this.lib.recursionLeaf(this.ctx, this.tc, this.family.recursions[recursionId]),
    );
  }

  recursionRetry(recursionId: number): void {
    this.call(() =>
      this.lib.recursionRetry(this.ctx, this.tc, this.family.recursions[recursionId]),
    );
  }

  recursionFinish(recursionId: number): boolean {
    return this.call(() =>
      this.lib.recursionFinish(this.ctx, this.tc, this.family.recursions[recursionId]),
    );
  }

  drawGrammar(spec: GrammarSpec, terminal: (index: number) => string): string {
    const { lib, ctx, tc } = this;
    const cache = this.family.cache;
    const grammar = this.call(() => cache.borrowGrammar(ctx, spec));
    try {
      const draw = this.call(() => lib.grammarDrawNew(ctx, grammar, 0));
      return withCleanup(
        () => {
          for (;;) {
            const index = this.call(() => lib.grammarDrawNext(ctx, tc, draw));
            if (index === null) return this.call(() => lib.grammarDrawResult(ctx, draw));
            const value = terminal(index);
            this.call(() => lib.grammarDrawProvide(ctx, draw, value));
          }
        },
        () => this.call(() => lib.freeGrammarDraw(draw)),
      );
    } finally {
      cache.releaseGrammar(spec);
    }
  }

  newStateMachine(spec: StateMachineSpec): number {
    this.family.machines.push(
      this.call(() =>
        this.lib.newStateMachine(this.ctx, this.tc, {
          ruleNames: spec.ruleNames,
          // One concurrency group, so rules never need to be kept apart.
          ruleGroups: spec.ruleNames.map(() => 0),
          ruleWeights: spec.ruleWeights,
          invariantNames: spec.invariantNames,
          invariantAlwaysCheck: spec.invariantAlwaysCheck,
          minConcurrency: 1,
          maxConcurrency: 1,
          stepCount: spec.stepCount,
        }),
      ),
    );
    return this.family.machines.length - 1;
  }

  stateMachineNextRound(machineId: number): boolean {
    // With a single group the id it reports carries no information; only
    // whether the machine has another round to run matters.
    return this.call(
      () =>
        this.lib.stateMachineNextGroup(this.ctx, this.tc, this.family.machines[machineId]) !== null,
    );
  }

  stateMachineNextRule(machineId: number): number | null {
    return this.call(() =>
      this.lib.stateMachineNextRule(this.ctx, this.tc, this.family.machines[machineId], 0),
    );
  }

  stateMachineRuleRejected(machineId: number): void {
    this.call(() =>
      this.lib.stateMachineRuleRejected(this.ctx, this.tc, this.family.machines[machineId], 0),
    );
  }

  stateMachineShouldCheckInvariant(machineId: number, invariantIndex: number): boolean {
    return this.call(() =>
      this.lib.stateMachineShouldCheckInvariant(
        this.ctx,
        this.tc,
        this.family.machines[machineId],
        invariantIndex,
      ),
    );
  }

  note(message: string): void {
    this.call(() => this.lib.note(this.ctx, this.tc, message));
  }

  target(score: number, label: string): void {
    this.call(() => this.lib.target(this.ctx, this.tc, score, label));
  }

  event(label: string): void {
    this.call(() => this.lib.event(this.ctx, this.tc, label));
  }

  eventValue(label: string, value: number): void {
    this.call(() => this.lib.eventValue(this.ctx, this.tc, value, label));
  }

  private derive(handle: TestCaseHandle): EngineDataSource {
    this.family.derived.push(handle);
    return new EngineDataSource(this.lib, this.ctx, handle, this.family.cache, false, this.family);
  }

  clone(): EngineDataSource {
    return this.derive(this.call(() => this.lib.cloneTestCase(this.ctx, this.tc)));
  }

  block(indent: number): EngineDataSource {
    return this.derive(this.call(() => this.lib.blockTestCase(this.ctx, this.tc, indent)));
  }

  setWorker(index: number): void {
    this.call(() => this.lib.setWorker(this.ctx, this.tc, index));
  }

  private newPrinter(): PrinterHandle {
    const handle = this.call(() => this.lib.testCasePrinter(this.ctx, this.tc));
    this.family.printers.push(handle);
    return handle;
  }

  printer(): Printer {
    const family = this.family;
    return enginePrinter(
      {
        lib: this.lib,
        ctx: this.ctx,
        released: () => family.released,
        adopt: (handle) => family.regions.push(handle),
        run: (operation) => this.call(operation),
      },
      this.newPrinter(),
      false,
    );
  }

  /** Whether the engine stamped this test case for capture. */
  shouldCapture(): boolean {
    return this.call(() => this.lib.shouldCapture(this.ctx, this.tc));
  }

  /**
   * The test case's printed draws and notes, one line per element: its whole
   * document, with every clone's and block's region spliced in. Reading
   * seals the document.
   */
  render(): string[] {
    const handle = this.newPrinter();
    this.family.released = true;
    return this.call(() => {
      if (this.family.derived.length > 0 || anyLive(this.lib, this.ctx, this.family.regions)) {
        this.lib.printerResolve(this.ctx, handle);
      }
      return splitLines(this.lib.printerValue(this.ctx, handle));
    });
  }

  assertHealthy(): void {
    if (this.family.fault !== undefined) throw this.family.fault;
  }

  private call<T>(operation: () => T): T {
    try {
      return operation();
    } catch (error) {
      if (error instanceof AssumeError || error instanceof StopTestError) throw error;
      const fault =
        error instanceof EngineError ? error : new EngineError(String(error), { cause: error });
      this.family.fault ??= fault;
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
    const family = this.family;
    family.released = true;
    release(family.regions, (printer) => this.lib.freePrinter(printer));
    release(family.printers, (printer) => this.lib.freePrinter(printer));
    release(family.derived, (tc) => this.lib.freeTestCase(tc));
    release(family.collections, (collection) => this.lib.freeCollection(collection));
    release(family.pools, (pool) => this.lib.freePool(pool));
    release(family.recursions, (recursion) => this.lib.freeRecursion(recursion));
    release(family.machines, (machine) => this.lib.freeStateMachine(machine));
    try {
      if (family.ownsCache) family.cache.dispose();
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

function classifyResult(e: unknown): TestCaseResult {
  if (e instanceof AssumeError) return { status: "invalid" };
  if (e instanceof StopTestError) return { status: "overrun" };
  if (e instanceof LoopDone) return { status: "valid" };
  if (e instanceof EngineError) throw e;
  return { status: "interesting", error: e, origin: extractOrigin(e) };
}

function finishTestCase(dataSource: DataSource, outcome: () => TestCaseResult): TestCaseResult {
  let result: TestCaseResult;
  try {
    result = outcome();
  } catch (fault) {
    try {
      dataSource.markComplete(Status.INVALID, null);
    } catch (completionError) {
      throw new AggregateError([fault, completionError], "Engine failure during case completion", {
        cause: completionError,
      });
    }
    throw fault;
  }
  dataSource.markComplete(
    RESULT_TO_STATUS[result.status],
    result.status === "interesting" ? result.origin : null,
  );
  return result;
}

/**
 * Run the test body once against `dataSource`, classify how it ended, and
 * report that to the engine. `emit` is whether its draws and notes are
 * printed; `explicit` serves an explicit example's values to its draws.
 */
export async function runTestCaseAsync(
  dataSource: DataSource,
  testFn: (tc: TestCase) => void | Promise<void>,
  emit = false,
  explicit: ExplicitValues | null = null,
): Promise<TestCaseResult> {
  const tc = new TestCase(dataSource, emit, explicit);
  let thrown: { error: unknown } | null = null;
  try {
    await testFn(tc);
  } catch (error: unknown) {
    thrown = { error };
  }
  return finishTestCase(dataSource, () => {
    dataSource.assertHealthy?.();
    return thrown === null ? { status: "valid" } : classifyResult(thrown.error);
  });
}

/** {@link runTestCaseAsync} for a synchronous test body. */
export function runTestCase(
  dataSource: DataSource,
  testFn: (tc: TestCase) => void,
  emit = false,
  explicit: ExplicitValues | null = null,
): TestCaseResult {
  const tc = new TestCase(dataSource, emit, explicit);
  let thrown: { error: unknown } | null = null;
  try {
    testFn(tc);
  } catch (error: unknown) {
    thrown = { error };
  }
  return finishTestCase(dataSource, () => {
    dataSource.assertHealthy?.();
    return thrown === null ? { status: "valid" } : classifyResult(thrown.error);
  });
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
  throw new AggregateError(
    errors,
    `Property-based test failed with ${errors.length} distinct failures.`,
  );
}

/** Read what the run reports about one failure, then free it. */
function readFailure(
  lib: Engine,
  failure: FailureHandle,
): { origin: string; blob: string | null; caveat: string | null } {
  const errors: unknown[] = [];
  try {
    return {
      origin: lib.failureOrigin(failure),
      blob: lib.reproductionBlob(failure),
      caveat: lib.failureCaveat(failure),
    };
  } catch (error) {
    errors.push(error);
    throw error;
  } finally {
    finishCleanup(errors, () => lib.freeFailure(failure));
  }
}

/** The values of an explicit example, as given to {@link Hegel.example} or {@link Hegel.namedExample}. */
type ExplicitExample =
  | { kind: "positional"; values: readonly unknown[] }
  | { kind: "named"; values: Readonly<Record<string, unknown>> };

/** How an explicit example is described in its report and errors: `example(1, "a")`. */
function describeExample(lib: Engine, ctx: ContextHandle, example: ExplicitExample): string {
  return printedLine(lib, ctx, (printer) => {
    if (example.kind === "named") {
      printer.text("namedExample(");
      printer.value(example.values);
    } else {
      printer.text("example(");
      example.values.forEach((value, index) => {
        if (index > 0) printer.text(", ");
        printer.value(value);
      });
    }
    printer.text(")");
  });
}

/** One test case the run loop hands its driver to execute. */
interface Step {
  ds: DataSource;
  emit: boolean;
  explicit: ExplicitValues | null;
}

/**
 * One failure origin's report material, captured as the test cases ran: the
 * printed lines of the case, whether the engine stamped it, and the error to
 * throw. A capture replaces an earlier one of the same origin unless that one
 * ranks higher, so a stamped execution's report survives later unstamped
 * shrink probes while, at equal rank, the newest wins.
 */
interface Captured {
  lines: string[];
  stamped: boolean;
  error: unknown;
}

function rank(capture: Captured): number {
  if (capture.stamped) return 2;
  return capture.lines.length > 0 ? 1 : 0;
}

/** What one engine run concluded. */
interface RunOutcome {
  /** Whether any failure was reported. */
  failed: boolean;
  errors: unknown[];
}

const DID_NOT_REPRODUCE =
  "reproduceFailure: the supplied failure blob did not reproduce a failure. The failure may " +
  "have been fixed, or, for a nondeterministic blob, it may not have recurred within the " +
  "replay budget (a bug failing 10% of the time escapes it about 5% of the time). Re-run to " +
  "retry, or remove the reproduceFailure call once the failure is fixed.";

let installedRuntime: RuntimeServices | null = null;

/**
 * Set the runtime a {@link Hegel} built without one runs on: what the Node and
 * browser entry points do when loaded.
 *
 * @internal
 */
export function installRuntime(runtime: RuntimeServices): void {
  installedRuntime = runtime;
  installDocumentEngine(() => runtime.getEngine());
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
  private readonly _examples: ExplicitExample[] = [];
  private readonly _blobs: string[] = [];

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
   * place of the one derived from its location: its file, relative to the
   * working directory, plus its name (`"tests/a.test.ts::property"`). A test
   * function's name is its own, or for an anonymous one the name Vitest or
   * Jest gives the running test, else the enclosing function's, else its
   * line. When several `hegel.test` calls in one runner test or enclosing
   * function borrow its name, they are numbered in the order their lines
   * first run in the process, the first keeping the plain key and each later
   * one carrying its number (`"tests/a.test.ts::describe > test (2)"`). Such
   * a key does not depend on which other runner tests ran, but skipping an
   * earlier call in the same runner test, adding one above it, or moving its
   * line in a process that keeps Hegel loaded renumbers it; name the test
   * function or set a key here to keep its saved failures. Where the runtime
   * has no `Error.captureStackTrace` (Firefox) there is no call site to
   * derive a location from: the key is the test function's name, and an
   * anonymous test has none unless set here.
   */
  databaseKey(key: string): this {
    this._databaseKey = key;
    return this;
  }

  /**
   * Add an explicit example: a test case run with the given values before
   * any generated ones, each value served, in order, to one of the test's
   * draws in place of a generated value. Explicit examples run in the
   * `Phase.Explicit` phase. One that fails fails the test like a generated
   * failing example does: it is reported, but not shrunk.
   *
   * Misusing an explicit example throws instead of being reported as a
   * failing one: rejecting it with `tc.assume` or `tc.reject` (explicit test
   * cases must not be rejected), leaving a value undrawn, making a draw it
   * has no value for, or calling `tc.drawSilent` in it. `tc.repeat` runs its
   * body once in an explicit example.
   *
   * @example
   * ```ts
   * new hegel.Hegel((tc) => {
   *   const xs = tc.draw(gs.arrays(gs.integers()));
   *   const n = tc.draw(gs.integers());
   *   // ...
   * })
   *   .example([], 0)
   *   .example([1, 2, 3], -1)
   *   .runSync();
   * ```
   */
  example(...values: unknown[]): this {
    this._examples.push({ kind: "positional", values });
    return this;
  }

  /**
   * Add an explicit example whose values are served to draws by name: each
   * key is the name a draw is reported under (`"draw_1"` for the test's first
   * unnamed draw, or the `name` passed to {@link TestCase.draw}). See
   * {@link example}.
   *
   * @example
   * ```ts
   * new hegel.Hegel((tc) => {
   *   const n = tc.draw(gs.integers(), "n");
   *   // ...
   * })
   *   .namedExample({ n: 0 })
   *   .runSync();
   * ```
   */
  namedExample(values: Readonly<Record<string, unknown>>): this {
    this._examples.push({ kind: "named", values });
    return this;
  }

  /**
   * Replay the failure that `blob` encodes instead of running the test
   * normally: the blob a failure report prints, with the `printBlob` setting
   * on, as how to reproduce it. Call it once for each failure to replay. A
   * run that replays blobs generates nothing else, and throws a
   * {@link DidNotReproduceError} when none of them fails any more.
   */
  reproduceFailure(blob: string): this {
    this._blobs.push(blob);
    return this;
  }

  private location(): TestLocation | null {
    if (this._testLocation !== null) return this._testLocation;
    return this.site === null ? null : locationOf(this.site, this.testFn);
  }

  private output(line: string): void {
    this.runtime.output(line);
  }

  /**
   * Generator that drives a test, yielding a {@link Step} for each test case
   * the driver ({@link run} or {@link runSync}) should execute the body
   * against: first the explicit examples, then each case of the engine run.
   * The driver runs the body and resumes the generator with the
   * {@link TestCaseResult}. This factoring lets the sync and async drivers
   * share one loop implementation.
   */
  private *runSteps(): Generator<Step, void, TestCaseResult> {
    this.runtime.validateSettings(this._settings);
    const lib = this.runtime.getEngine();
    const ctx = lib.newContext();
    const contextErrors: unknown[] = [];
    try {
      const settings = resolveSettings(lib, ctx, this._settings);
      const cache = new StringGeneratorCache(lib, this.runtime.readGrammarFile);
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
          (location !== null
            ? databaseKeyOf(
                location,
                this.runtime.projectRoot,
                this._testLocation === null && !this.testFn.name,
              )
            : this.testFn.name || null);
        if (key !== null) lib.setDatabaseKey(ctx, settings, key);
        yield* this.runAll(lib, ctx, settings, cache);
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

  private *runAll(
    lib: Engine,
    ctx: ContextHandle,
    settings: SettingsHandle,
    cache: StringGeneratorCache,
  ): Generator<Step, void, TestCaseResult> {
    const verbosity = lib.getVerbosity(settings);
    const quiet = verbosity === NativeVerbosity.QUIET;
    const verbose = verbosity === NativeVerbosity.VERBOSE || verbosity === NativeVerbosity.DEBUG;
    const printBlob = lib.getPrintBlob(settings);

    if (this._blobs.length > 0) {
      const errors: unknown[] = [];
      for (const blob of this._blobs) {
        const run = lib.runStartBlob(ctx, settings, blob);
        const outcome = yield* this.driveRun(lib, ctx, run, cache, quiet, verbose, false);
        errors.push(...outcome.errors);
      }
      if (errors.length === 0) throw new DidNotReproduceError(DID_NOT_REPRODUCE);
      throwFailures(errors);
    }

    const failures: unknown[] = [];
    if ((lib.getPhases(settings) & NativePhase.EXPLICIT) !== 0) {
      const multiple = lib.getReportMultipleFailures(settings);
      for (const given of this._examples) {
        const example = new ExplicitValues(
          describeExample(lib, ctx, given),
          given.kind === "positional" ? given.values : [],
          given.kind === "named" ? given.values : {},
        );
        const document = new EngineDocument(lib, ctx);
        const errors: unknown[] = [];
        try {
          const ds = new ExplicitDataSource(document);
          const outcome = yield { ds, emit: true, explicit: example };
          if (outcome.status === "invalid") {
            throw new ExplicitExampleError(
              `Explicit test cases must not be rejected, but ${example.description} was rejected by tc.assume or tc.reject`,
            );
          }
          if (outcome.status !== "interesting") {
            example.checkConsumed();
            continue;
          }
          if (outcome.error instanceof ExplicitExampleError) throw outcome.error;
          if (!quiet) {
            for (const line of document.lines()) this.output(line);
          }
          if (!multiple) throw outcome.error;
          failures.push(outcome.error);
        } catch (error) {
          errors.push(error);
        } finally {
          finishCleanup(errors, () => document.dispose());
        }
      }
    }

    let outcome: RunOutcome;
    try {
      const run = lib.runStart(ctx, settings);
      outcome = yield* this.driveRun(lib, ctx, run, cache, quiet, verbose, printBlob);
    } catch (error) {
      if (failures.length === 0) throw error;
      throwFailures([...failures, error]);
    }
    if (failures.length > 0 || outcome.failed) throwFailures([...failures, ...outcome.errors]);
  }

  /**
   * Pull every test case off `run`, capturing each failure origin's report
   * material, then act on the run's verdict: return when it passed, throw a
   * {@link RunError} when it errored, and print each failure's report when it
   * failed, returning the errors to throw. The engine owns the whole
   * exploration, the final replay of each reported failure included: it
   * stamps the executions a report can be built from, and those print their
   * draws and notes.
   */
  private *driveRun(
    lib: Engine,
    ctx: ContextHandle,
    run: RunHandle,
    cache: StringGeneratorCache,
    quiet: boolean,
    verbose: boolean,
    printBlob: boolean,
  ): Generator<Step, RunOutcome, TestCaseResult> {
    const runErrors: unknown[] = [];
    try {
      const captured = new Map<string, Captured>();
      for (;;) {
        const tc = lib.nextTestCase(ctx, run);
        if (tc === null) break;
        const ds = new EngineDataSource(lib, ctx, tc, cache, false);
        try {
          const stamped = ds.shouldCapture();
          const emit = (stamped && !quiet) || verbose;
          const outcome = yield { ds, emit, explicit: null };
          const lines = emit ? ds.render() : [];
          if (verbose) for (const line of lines) this.output(line);
          if (outcome.status === "interesting") {
            const capture: Captured = { lines, stamped, error: outcome.error };
            const stored = captured.get(outcome.origin);
            if (stored === undefined || rank(capture) >= rank(stored)) {
              captured.set(outcome.origin, capture);
            }
          }
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
        if (status === RunStatus.PASSED) return { failed: false, errors: [] };
        if (status === RunStatus.ERROR) {
          throw runError(lib.runError(result), lib.runErrorKind(result));
        }
        if (status !== RunStatus.FAILED) {
          throw new EngineError(`Unknown Hegel run status: ${status}`);
        }
        const count = lib.failureCount(result);
        if (count > 1 && !quiet) {
          this.output(`Property-based test failed with ${count} distinct failures.`);
        }
        const errors: unknown[] = [];
        for (let i = 0; i < count; i++) {
          const { origin, blob, caveat } = readFailure(lib, lib.failure(result, i));
          const report = captured.get(origin);
          if (report === undefined) {
            throw new EngineError(`Hegel failure ${origin} was never seen by the test`);
          }
          if (count > 1 && !quiet) this.output("");
          for (const line of report.lines) this.output(line);
          if (caveat !== null && !quiet) this.output(`note: ${caveat}`);
          if (blob !== null && printBlob) {
            this.output("");
            this.output("To reproduce this failure, replay it with the Hegel builder:");
            this.output(`    .reproduceFailure(${JSON.stringify(blob)})`);
          }
          errors.push(report.error);
        }
        return { failed: true, errors };
      } catch (error) {
        resultErrors.push(error);
        throw error;
      } finally {
        finishCleanup(resultErrors.splice(0), () => lib.freeRunResult(result));
      }
    } catch (error) {
      runErrors.push(error);
      throw error;
    } finally {
      finishCleanup(runErrors.splice(0), () => lib.freeRun(run));
    }
  }

  /** Run the test, awaiting an asynchronous test body. */
  async run(): Promise<void> {
    const gen = this.runSteps();
    const errors: unknown[] = [];
    try {
      let next = gen.next();
      while (!next.done) {
        const { ds, emit, explicit } = next.value;
        const result = await runTestCaseAsync(ds, this.testFn, emit, explicit);
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
        const { ds, emit, explicit } = next.value;
        const result = runTestCase(ds, this.testFn as (tc: TestCase) => void, emit, explicit);
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
