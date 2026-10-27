/**
 * TestCase: per-test-case state passed explicitly to test functions.
 *
 * Provides draw(), assume(), note(), target(), event(), repeat(), clone(),
 * span management, and the Collection class for engine-managed collection
 * sizing.
 *
 * @packageDocumentation
 */

import { EngineError } from "./engine.js";
import { PrettyPrinter, type Printer } from "./pretty.js";
import type { LabelSpec } from "./labels.js";
// Type-only import to avoid a runtime cycle: Generator depends on TestCase.
import type { Generator, PrintableGenerator } from "./generators/core.js";
import type { GrammarSpec } from "./generate.js";

export class StopTestError extends Error {
  constructor() {
    super("Hegel ran out of data (StopTest)");
    this.name = "StopTestError";
  }
}

export class AssumeError extends Error {
  constructor() {
    super("Assumption rejected");
    this.name = "AssumeError";
  }
}

/**
 * Thrown by {@link TestCase.repeat} once the engine has decided how many
 * repetitions to run: it ends the test case, which then passes.
 *
 * @internal
 */
export class LoopDone extends Error {
  constructor() {
    super("TestCase.repeat finished");
    this.name = "LoopDone";
  }
}

/**
 * A misused explicit example: one that is rejected, leaves a value undrawn,
 * has no value for a draw, or does what an explicit example cannot. It fails
 * the run as it is, rather than being reported as a failing example.
 *
 * @internal
 */
export class ExplicitExampleError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ExplicitExampleError";
  }
}

/** The registration for a state machine, see {@link DataSource.newStateMachine}. */
export interface StateMachineSpec {
  ruleNames: readonly string[];
  invariantNames: readonly string[];
  /** Parallel to `invariantNames`: whether the invariant is exempt from sampling. */
  invariantAlwaysCheck: readonly boolean[];
  stepCount: number;
}

/**
 * Abstraction over the data backend for a test case.
 *
 * The default implementation (`EngineDataSource`) drives a libhegel test case
 * via its C ABI; explicit examples use one that serves no draws. Custom
 * implementations can be used in tests to inject specific behaviors without
 * the engine.
 *
 * `status` passed to {@link DataSource.markComplete} is a `hegel_status_t`
 * value.
 *
 * Collections, pools and state machines are identified by numeric ids handed
 * out by their `new*` method; the data source owns the underlying handles for
 * the life of the test case, shared with every data source derived from it by
 * {@link clone} or {@link block}.
 */
export interface DataSource {
  /** The same object for a data source and every data source derived from it. */
  readonly family: object;
  assertHealthy?(): void;
  generate(schema: Record<string, unknown>): unknown;
  /** The span label for `spec`. */
  label(spec: LabelSpec): bigint;
  startSpan(label: bigint): void;
  stopSpan(discard: boolean): void;
  newCollection(minSize: number, maxSize?: number): number;
  collectionMore(collectionId: number): boolean;
  collectionReject(collectionId: number, why?: string): void;
  /** Start a repeat loop (see {@link TestCase.repeat}). */
  newRepeat(): number;
  /** Whether the engine wants another iteration of the loop. */
  repeatMore(repeatId: number): boolean;
  /** Report that the iteration just run failed, before the failure is rethrown. */
  repeatFailed(repeatId: number): void;
  markComplete(status: number, origin: string | null): void;

  /** Open a variable pool (see `stateful.Pool`). */
  newPool(): number;
  /** Register a new variable in the pool, returning its engine-assigned id. */
  poolAdd(poolId: number): bigint;
  /**
   * Draw the id of a variable in the pool, removing it when `consume` is set.
   * Throws {@link AssumeError} when the pool is empty.
   */
  poolGenerate(poolId: number, consume: boolean): bigint;
  /** Put back a variable that a consuming {@link poolGenerate} removed. */
  poolRestore(poolId: number, variableId: bigint): void;

  /** Open a recursive generation scope (see `generators.recursive`). */
  newRecursion(maxDepth: number, maxLeaves: number): number;
  /** Whether the sub-value at `depth` is a branch (`true`) or a leaf. */
  recursionBranch(recursionId: number, depth: number): boolean;
  /** Count a leaf; `false` when the attempt outgrew its budget and must be retried. */
  recursionLeaf(recursionId: number): boolean;
  /** Discard an attempt that outgrew its budget; throws {@link AssumeError} when out of attempts. */
  recursionRetry(recursionId: number): void;
  /** Finish the value; `false` when the engine discarded it and it must be drawn again. */
  recursionFinish(recursionId: number): boolean;

  /**
   * Draw a string of the grammar `spec` describes (see `generators.fromLark`),
   * calling `terminal` for the value of each occurrence of an explicit
   * terminal, by its index in `spec.explicitNames`.
   */
  drawGrammar(spec: GrammarSpec, terminal: (index: number) => string): string;

  /** Register a sequential state machine (single group, concurrency 1). */
  newStateMachine(spec: StateMachineSpec): number;
  /** Start the next round; `false` once the machine is finished. */
  stateMachineNextRound(machineId: number): boolean;
  /** The index of the next rule to run this round, or `null` at the join point. */
  stateMachineNextRule(machineId: number): number | null;
  /** Report the rule last returned by {@link stateMachineNextRule} as rejected. */
  stateMachineRuleRejected(machineId: number): void;
  /** Whether invariant `invariantIndex` should run at the current join point. */
  stateMachineShouldCheckInvariant(machineId: number, invariantIndex: number): boolean;

  /** Append a note to this data source's region of the output. */
  note(message: string): void;
  target(score: number, label: string): void;
  event(label: string): void;
  eventValue(label: string, value: number): void;
  /** A data source drawing from an independent choice stream of the same test case. */
  clone(): DataSource;
  /** A data source on the same choice stream whose output is indented `indent` further. */
  block(indent: number): DataSource;
  /** Attribute this data source's output to concurrent worker `index`. */
  setWorker(index: number): void;
  /** The printer for this data source's region of the output. */
  printer(): Printer;
}

function isPrintable<T>(generator: Generator<T>): generator is PrintableGenerator<T> {
  return typeof (generator as { doDrawAndPrint?: unknown }).doDrawAndPrint === "function";
}

function unprintable(generator: Generator<unknown>): TypeError {
  return new TypeError(
    `${generator.constructor.name} cannot print the values it draws. Make it printable with ` +
      ".printWith((value, printer) => …) or .printAsValue(); declare helpers as returning " +
      "PrintableGenerator<T> rather than Generator<T>; or draw it with tc.drawSilent(…).",
  );
}

/** A name a draw is reported under; see {@link TestCase.draw}. */
interface DrawState {
  readonly counts: Map<string, number>;
  readonly allocated: Set<string>;
}

/**
 * The values of an explicit example, served to the draws of a test body in
 * place of generated ones: positional values in draw order, named values to
 * the draw reported under that name.
 *
 * @internal
 */
export class ExplicitValues {
  private readonly positional: unknown[];
  private readonly named: Map<string, unknown>;
  private readonly used = new Set<string>();
  private consumed = 0;

  constructor(
    readonly description: string,
    positional: readonly unknown[],
    named: Readonly<Record<string, unknown>>,
  ) {
    this.positional = [...positional];
    this.named = new Map(Object.entries(named));
  }

  /** The value for the draw reported as `name`. */
  take(name: string): unknown {
    if (this.named.has(name) && !this.used.has(name)) {
      this.used.add(name);
      return this.named.get(name);
    }
    if (this.consumed < this.positional.length) {
      return this.positional[this.consumed++];
    }
    throw new ExplicitExampleError(
      `Explicit example ${this.description} has no value for draw ${JSON.stringify(name)}: pass it by name, or add a positional value`,
    );
  }

  /** Throw unless every value was drawn. */
  checkConsumed(): void {
    const unused = [...this.named.keys()].filter((name) => !this.used.has(name));
    const leftover = this.positional.length - this.consumed;
    const problems: string[] = [];
    if (unused.length > 0) {
      problems.push(`named values not drawn: ${unused.map((n) => JSON.stringify(n)).join(", ")}`);
    }
    if (leftover > 0) problems.push(`${leftover} positional value(s) not drawn`);
    if (problems.length > 0) {
      throw new ExplicitExampleError(
        `Explicit example ${this.description} was not fully used: ${problems.join("; ")}`,
      );
    }
  }
}

/** What every {@link TestCase} derived from one test body's shares. */
interface Shared {
  readonly emit: boolean;
  readonly explicit: ExplicitValues | null;
}

/**
 * A handle to the current test case, passed to the test function: draw values
 * from it, make assumptions, and record notes, targets and events.
 */
export class TestCase {
  private _dataSource: DataSource;
  private readonly shared: Shared;
  private readonly drawState: DrawState;
  private _printer: PrettyPrinter | null = null;
  private spanDepth = 0;
  private printingDepth = 0;

  /**
   * `emit` is whether this test case's draws and notes are printed: the
   * engine stamped it for capture, or the run is verbose.
   *
   * @internal
   */
  constructor(
    dataSource: DataSource,
    emit = false,
    explicit: ExplicitValues | null = null,
    drawState?: DrawState,
  ) {
    this._dataSource = dataSource;
    this.shared = { emit: emit || explicit !== null, explicit };
    this.drawState = drawState ?? { counts: new Map(), allocated: new Set() };
  }

  private derive(dataSource: DataSource, drawState?: DrawState): TestCase {
    return new TestCase(dataSource, this.shared.emit, this.shared.explicit, drawState);
  }

  /** @internal */
  dataSource(): DataSource {
    return this._dataSource;
  }

  private printer(): PrettyPrinter {
    this._printer ??= new PrettyPrinter(this._dataSource.printer());
    return this._printer;
  }

  /**
   * Draw a value from a generator.
   *
   * When this test case is reported (the failing example, or every test case
   * of a verbose run), the generator prints the value as it draws it, as
   * `const draw_1 = <value>;`, numbered in the order of the test's draws.
   * Pass `name` to report it as `const <name> = <value>;` instead (a name
   * drawn again is numbered: `<name>_2`, `<name>_3`, …). Draws made while
   * another is in progress — inside a {@link generators.composite | composite}
   * generator, say — are part of the outer value and are not printed
   * separately.
   *
   * The generator must be a {@link generators.PrintableGenerator}; a
   * generator that cannot print throws a `TypeError`. Use
   * {@link drawSilent} to draw from one without printing it.
   */
  draw<T>(generator: PrintableGenerator<T>, name?: string): T {
    if (!isPrintable(generator)) throw unprintable(generator);
    if (this.shared.explicit === null && (this.spanDepth > 0 || this.printingDepth > 0)) {
      return generator.doDraw(this);
    }
    const display = this.allocateDisplayName(name ?? "draw", name === undefined);
    if (display === null) return this.drawSilent(generator);
    this.printingDepth++;
    const printer = this.printer();
    const speculation = printer.speculate();
    try {
      printer.text(`const ${display} = `);
      let value: T;
      if (this.shared.explicit === null) {
        value = generator.doDrawAndPrint(this, printer);
      } else {
        value = this.shared.explicit.take(display) as T;
        printer.value(value);
      }
      printer.text(";");
      printer.hardBreak();
      speculation.commit();
      return value;
    } finally {
      speculation.abort();
      this.printingDepth--;
    }
  }

  /**
   * Draw a value from a generator without printing it in the report of a
   * failing example. Use it for values that would only clutter the report,
   * or from a generator that cannot print. It cannot be used in an explicit
   * example, whose values are matched to draws by the names they are printed
   * under.
   */
  drawSilent<T>(generator: Generator<T>): T {
    if (this.shared.explicit !== null) {
      throw new ExplicitExampleError("drawSilent is not supported in explicit examples");
    }
    this.printingDepth++;
    try {
      return generator.doDraw(this);
    } finally {
      this.printingDepth--;
    }
  }

  /**
   * Draw a value from `generator`, printing it to `printer`: how a
   * {@link generators.PrintableGenerator} draws the generators it is built
   * from, so that each prints in place inside its own output. With
   * {@link PrettyPrinter.noop} it draws silently, from any generator.
   */
  drawAndPrint<T>(generator: PrintableGenerator<T>, printer: PrettyPrinter): T {
    if (this.shared.explicit !== null && this.printingDepth === 0) {
      throw new ExplicitExampleError("drawAndPrint is not supported in explicit examples");
    }
    if (isPrintable(generator)) return generator.doDrawAndPrint(this, printer);
    if (printer.region === null) return (generator as Generator<T>).doDraw(this);
    throw unprintable(generator);
  }

  private allocateDisplayName(name: string, repeatable: boolean): string | null {
    const { counts, allocated } = this.drawState;
    const count = (counts.get(name) ?? 0) + 1;
    counts.set(name, count);
    if (!this.shared.emit) return null;
    let candidate = name;
    if (repeatable || count > 1 || allocated.has(name)) {
      let suffix = count;
      candidate = `${name}_${suffix}`;
      while (allocated.has(candidate)) {
        suffix++;
        candidate = `${name}_${suffix}`;
      }
    }
    allocated.add(candidate);
    return candidate;
  }

  /**
   * Reject the current test case if the condition is false. A rejected test
   * case is neither a pass nor a failure: it is abandoned, and does not count
   * toward `testCases`.
   */
  assume(condition: boolean): void {
    if (!condition) {
      this.reject();
    }
  }

  /** Reject the current test case unconditionally, as `assume(false)` does. */
  reject(): never {
    throw new AssumeError();
  }

  /**
   * Add a message to the output of this test case, between its drawn values
   * in the order it was recorded. The output is shown for the failing example
   * Hegel reports, and for every test case of a verbose run.
   */
  note(message: string): void {
    if (!this.shared.emit) return;
    this._dataSource.note(String(message));
  }

  /**
   * Guide generation toward test cases that maximise `score`. `label` names
   * the quantity being maximised, so that one test can target several; each
   * label can be targeted at most once per test case. `score` must be finite.
   * Has no effect in an explicit example.
   */
  target(score: number, label = ""): void {
    if (this.shared.explicit !== null) return;
    this._dataSource.target(score, label);
  }

  /**
   * Record that `label` happened in this test case. With the `showStatistics`
   * setting on, the end of the run reports the fraction of test cases
   * recording each event. Has no effect in an explicit example.
   */
  event(label: string): void {
    if (this.shared.explicit !== null) return;
    this._dataSource.event(label);
  }

  /**
   * Record a numeric observation under `label`. With the `showStatistics`
   * setting on, the end of the run reports the count, minimum, median, mean,
   * 90th percentile and maximum of the values recorded under each label.
   * `value` must be finite. Has no effect in an explicit example.
   */
  eventValue(label: string, value: number): void {
    if (this.shared.explicit !== null) return;
    this._dataSource.eventValue(label, value);
  }

  private beginIteration(iteration: number): DataSource {
    this.note(`// Repetition #${iteration}`);
    return this.switchTo(this._dataSource.block(2));
  }

  private failIteration(outer: DataSource, repeatId: number, error: unknown): void {
    this.switchTo(outer);
    if (error instanceof AssumeError) return;
    if (
      error instanceof StopTestError ||
      error instanceof EngineError ||
      error instanceof LoopDone
    ) {
      throw error;
    }
    this._dataSource.repeatFailed(repeatId);
    throw error;
  }

  /**
   * Run `body` an engine-chosen number of times, then end the test case
   * successfully: for workload-style tests that perform operations until the
   * engine decides to stop. A repetition that rejects (with {@link assume}) is
   * skipped and the next one runs. A failure in any repetition fails the test
   * case, and shrinking reduces the number of repetitions along with the
   * values each drew. Each repetition is reported under a `// Repetition #N`
   * heading, its draws and notes indented beneath it.
   *
   * `repeat` never returns: code after it does not run. For an asynchronous
   * body, use {@link repeatAsync}. In an explicit example, which has no
   * engine to choose the number of repetitions, `body` runs once.
   */
  repeat(body: () => void): never {
    if (this.shared.explicit !== null) {
      const outer = this.beginIteration(1);
      try {
        body();
      } finally {
        this.switchTo(outer);
      }
      throw new LoopDone();
    }
    const repeatId = this._dataSource.newRepeat();
    let iteration = 0;
    while (this._dataSource.repeatMore(repeatId)) {
      const outer = this.beginIteration(++iteration);
      try {
        body();
      } catch (error) {
        this.failIteration(outer, repeatId, error);
      } finally {
        this.switchTo(outer);
      }
    }
    throw new LoopDone();
  }

  /** {@link repeat} with an asynchronous body, each repetition awaited before the next. */
  async repeatAsync(body: () => void | Promise<void>): Promise<never> {
    if (this.shared.explicit !== null) {
      const outer = this.beginIteration(1);
      try {
        await body();
      } finally {
        this.switchTo(outer);
      }
      throw new LoopDone();
    }
    const repeatId = this._dataSource.newRepeat();
    let iteration = 0;
    while (this._dataSource.repeatMore(repeatId)) {
      const outer = this.beginIteration(++iteration);
      try {
        await body();
      } catch (error) {
        this.failIteration(outer, repeatId, error);
      } finally {
        this.switchTo(outer);
      }
    }
    throw new LoopDone();
  }

  /**
   * A handle onto the same test case drawing from its own, independent choice
   * stream: give each concurrent task of a test its own clone, so that what
   * one draws does not depend on how the tasks interleave. A clone's draws
   * and notes are printed where the clone was made; use {@link setWorker} to
   * label them with the task they came from. Clones share the test case's
   * outcome, so a clone must not be used once the test body has returned:
   * drawing from it then throws.
   */
  clone(): TestCase {
    return this.derive(this._dataSource.clone(), this.drawState);
  }

  /**
   * Attribute everything this handle prints from now on to concurrent worker
   * `index` (a non-negative integer): each line is prefixed with
   * `[worker N +X.XXXms]`, the time since the test case started.
   */
  setWorker(index: number): void {
    if (!Number.isSafeInteger(index) || index < 0) {
      throw new RangeError(`worker index must be a non-negative integer, got ${String(index)}`);
    }
    this._dataSource.setWorker(index);
  }

  /**
   * A handle for an indented section of this one's output: it draws from the
   * same choice stream, and everything it prints is indented `indent`
   * columns further, with draws named afresh.
   *
   * @internal
   */
  child(indent: number): TestCase {
    return this.derive(this._dataSource.block(indent));
  }

  private switchTo(dataSource: DataSource): DataSource {
    const previous = this._dataSource;
    this._dataSource = dataSource;
    this._printer = null;
    return previous;
  }

  /**
   * Start a shrinking span labelled for the generator `spec` describes.
   * Draws made inside it are part of the enclosing value and not printed.
   *
   * @internal
   */
  startSpan(spec: LabelSpec): void {
    this.spanDepth++;
    try {
      this._dataSource.startSpan(this._dataSource.label(spec));
    } catch (e) {
      this.spanDepth--;
      throw e;
    }
  }

  /**
   * The number of spans this handle has open.
   *
   * @internal
   */
  get openSpanDepth(): number {
    return this.spanDepth;
  }

  /**
   * Forget the spans opened above `depth`, which the engine has already
   * closed (as a discarded recursive attempt's are).
   *
   * @internal
   */
  resetOpenSpansTo(depth: number): void {
    this.spanDepth = depth;
  }

  /**
   * Stop the current shrinking span.
   * @internal
   */
  stopSpan(discard = false): void {
    this.spanDepth--;
    try {
      this._dataSource.stopSpan(discard);
    } catch (error) {
      if (error instanceof EngineError) throw error;
      // Preserve custom data-source behavior; engine faults are never suppressed.
    }
  }
}

/**
 * Send a generate command to the data source and return the raw result.
 * Throws StopTestError if the data source runs out of data.
 * @internal
 */
export function generateRaw(tc: TestCase, schema: Record<string, unknown>): unknown {
  return tc.dataSource().generate(schema);
}

/**
 * Engine-managed collection sizing.
 *
 * The engine determines how many elements to generate based on
 * min_size, max_size, and shrinking state.
 */
export class Collection {
  private dataSource: DataSource;
  private minSize: number;
  private maxSize: number | undefined;
  private collectionId: number | null = null;
  private finished = false;

  constructor(tc: TestCase, minSize: number, maxSize?: number) {
    this.dataSource = tc.dataSource();
    this.minSize = minSize;
    this.maxSize = maxSize;
  }

  private ensureInitialized(): number {
    if (this.collectionId === null) {
      this.collectionId = this.dataSource.newCollection(this.minSize, this.maxSize);
    }
    return this.collectionId;
  }

  /**
   * Ask the engine whether to produce another element.
   */
  more(): boolean {
    if (this.finished) {
      return false;
    }
    const collectionId = this.ensureInitialized();
    let result: boolean;
    try {
      result = this.dataSource.collectionMore(collectionId);
    } catch (e) {
      this.finished = true;
      throw e;
    }
    if (!result) {
      this.finished = true;
    }
    return result;
  }

  /**
   * Reject the last element (don't count towards size budget).
   */
  reject(why?: string): void {
    if (this.finished) return;
    const collectionId = this.ensureInitialized();
    this.dataSource.collectionReject(collectionId, why);
  }
}
