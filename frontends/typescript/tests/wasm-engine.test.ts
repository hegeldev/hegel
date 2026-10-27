import { describe, it, expect, vi } from "vitest";
import { wasmFixture } from "./wasmFixture.js";
import {
  createRunner,
  Database,
  HealthCheck,
  Verbosity,
  Phase,
  Backend,
  NondeterminismStrictness,
} from "../src/runnerCore.js";
import { FailedHealthCheckError } from "../src/errors.js";
import { browserRuntime } from "../src/browser/runtime.js";
import { WasmArena } from "../src/browser/arena.js";
import {
  EngineError,
  NativeVerbosity,
  RunStatus,
  Status,
  type Engine,
  type TextGeneratorOptions,
} from "../src/engine.js";
import { AssumeError, StopTestError } from "../src/testCase.js";
import * as gs from "../src/generators/index.js";
import * as stateful from "../src/stateful.js";
import { Libhegel } from "../src/libhegel.js";
import { testLibPath } from "./libPath.js";
import { fakeRuntime } from "./fakeEngine.js";
import { prettyPrint, type PrettyPrinter } from "../src/pretty.js";

function active<T>(
  engine: Engine,
  body: (
    ctx: ReturnType<Engine["newContext"]>,
    tc: NonNullable<ReturnType<Engine["nextTestCase"]>>,
  ) => T,
): T {
  const ctx = engine.newContext();
  try {
    const settings = engine.newSettings(ctx);
    try {
      engine.setSeed(settings, 42n);
      engine.setDatabase(ctx, settings, "");
      const run = engine.runStart(ctx, settings);
      try {
        const tc = engine.nextTestCase(ctx, run)!;
        try {
          return body(ctx, tc);
        } finally {
          engine.freeTestCase(tc);
        }
      } finally {
        engine.freeRun(run);
      }
    } finally {
      engine.freeSettings(settings);
    }
  } finally {
    engine.freeContext(ctx);
  }
}

const text: TextGeneratorOptions = {
  minSize: 3,
  maxSize: 3n,
  codec: null,
  minCodepoint: 0,
  maxCodepoint: 0,
  categories: null,
  excludeCategories: null,
  includeCharacters: null,
  excludeCharacters: null,
};

describe("WasmEngine pinned raw ABI", () => {
  it("matches native fixed temporal and signed integer draws", () => {
    const wasm = wasmFixture().engine;
    const native = Libhegel.load(testLibPath());
    const draw = (engine: Engine) =>
      active(engine, (ctx, tc) => {
        const date = { year: -12345, month: 6, day: 17 };
        const values: unknown[] = [engine.generateDate(ctx, tc, date, date)];
        for (const nanosecond of [0, 1, 123456789, 999999999]) {
          const time = { hour: 12, minute: 34, second: 56, nanosecond };
          values.push(engine.generateTime(ctx, tc, time, time));
          values.push(engine.generateDatetime(ctx, tc, { date, time }, { date, time }));
          expect(values.at(-2)).toEqual(time);
          expect(values.at(-1)).toEqual({ date, time });
        }
        for (const value of [-1n << 127n, 1n << 127n, -129n, 0n, 128n]) {
          expect(engine.generateIntegerBig(ctx, tc, value, value)).toBe(value);
        }
        expect(engine.generateInteger(ctx, tc, -12n, -12n)).toBe(-12n);
        const uuid = engine.generateUuid(ctx, tc, 4);
        expect(uuid).toHaveLength(16);
        expect(uuid[6] >> 4).toBe(4);
        expect(uuid[8] >> 6).toBe(2);
        values.push(Array.from(uuid), Array.from(engine.generateUuid(ctx, tc)));
        return values;
      });
    expect(draw(wasm)).toEqual(draw(native));
  });

  it("copies empty buffers, NUL, WTF-8 surrogates and astral text, preserving null vs empty lists", () => {
    const { engine } = wasmFixture();
    active(engine, (ctx, tc) => {
      expect(engine.generateBytes(ctx, tc, 0, 0n)).toEqual(new Uint8Array());
      expect(engine.generateBytes(ctx, tc, 7, 7n)).toHaveLength(7);
      for (const codepoint of [0, 0x1f642]) {
        const generator = engine.stringGeneratorText(ctx, {
          ...text,
          minCodepoint: codepoint,
          maxCodepoint: codepoint,
        });
        try {
          expect(engine.generateString(ctx, tc, generator)).toBe(
            String.fromCodePoint(codepoint).repeat(3),
          );
        } finally {
          engine.freeStringGenerator(generator);
        }
      }
      expect(() =>
        engine.stringGeneratorText(ctx, { ...text, minCodepoint: 0xd800, maxCodepoint: 0xd800 }),
      ).toThrow(/No valid characters/);
      const generator = engine.stringGeneratorText(ctx, {
        ...text,
        categories: [],
        excludeCategories: [],
        includeCharacters: new Uint8Array([0]),
        excludeCharacters: new Uint8Array(),
      });
      try {
        expect(engine.generateString(ctx, tc, generator)).toBe("\0\0\0");
      } finally {
        engine.freeStringGenerator(generator);
      }
      expect(() => engine.stringGeneratorText(ctx, { ...text, categories: [] })).toThrow(
        EngineError,
      );
    });
  });

  it("survives growth between allocations and after every engine call", () => {
    const { engine, abi } = wasmFixture();
    const alloc = abi.alloc.bind(abi),
      call = abi.call.bind(abi);
    vi.spyOn(abi, "alloc").mockImplementation((size, align) => {
      const ptr = alloc(size, align);
      abi.memory.grow(1);
      return ptr;
    });
    vi.spyOn(abi, "call").mockImplementation((op, ...args) => {
      const result = call(op, ...args);
      abi.memory.grow(1);
      return result;
    });
    active(engine, (ctx, tc) => {
      const generator = engine.stringGeneratorText(ctx, {
        ...text,
        categories: ["Cc"],
        codec: "utf-8",
      });
      try {
        expect(engine.generateString(ctx, tc, generator)).toBe("\0\0\0");
      } finally {
        engine.freeStringGenerator(generator);
      }
      const bytes = engine.generateBytes(ctx, tc, 20, 20n);
      const copy = bytes.slice();
      abi.memory.grow(2);
      expect(bytes).toEqual(copy);
      expect(
        engine.generateIntegerBig(ctx, tc, -100000000000000000000n, -100000000000000000000n),
      ).toBe(-100000000000000000000n);
    });
  });

  it("drives pools and state machines like the native adapter", () => {
    const wasm = wasmFixture().engine;
    const native = Libhegel.load(testLibPath());
    const options = {
      ruleNames: ["push", "pop"],
      ruleGroups: [0, 0],
      invariantNames: ["sorted", "small"],
      invariantAlwaysCheck: [true, false],
      minConcurrency: 1,
      maxConcurrency: 1,
      stepCount: 5,
    };
    const drive = (engine: Engine) =>
      active(engine, (ctx, tc) => {
        const trace: unknown[] = [];
        const pool = engine.newPool(ctx, tc);
        const machine = engine.newStateMachine(ctx, tc, options);
        try {
          expect(() => engine.poolGenerate(ctx, tc, pool, false)).toThrow(AssumeError);
          const first = engine.poolAdd(ctx, tc, pool);
          const second = engine.poolAdd(ctx, tc, pool);
          expect(first).not.toBe(second);
          expect([first, second]).toContain(engine.poolGenerate(ctx, tc, pool, false));
          expect([first, second]).toContain(engine.poolGenerate(ctx, tc, pool, true));
          trace.push(engine.poolGenerate(ctx, tc, pool, true));
          expect(() => engine.poolGenerate(ctx, tc, pool, true)).toThrow(AssumeError);
          let rounds = 0;
          while (engine.stateMachineNextGroup(ctx, tc, machine) !== null) {
            rounds++;
            for (;;) {
              const rule = engine.stateMachineNextRule(ctx, tc, machine, 0);
              if (rule === null) break;
              trace.push(rule);
              if (rule === 1) engine.stateMachineRuleRejected(ctx, tc, machine, 0);
            }
            expect(engine.stateMachineShouldCheckInvariant(ctx, tc, machine, 0)).toBe(true);
            trace.push(engine.stateMachineShouldCheckInvariant(ctx, tc, machine, 1));
          }
          expect(rounds).toBeGreaterThan(0);
          expect(rounds).toBeLessThanOrEqual(1000);
          return trace;
        } finally {
          engine.freeStateMachine(machine);
          engine.freePool(pool);
        }
      });
    expect(drive(wasm)).toEqual(drive(native));
  });

  it("drives recursions, pool restores and the Unicode version like the native adapter", () => {
    const wasm = wasmFixture().engine;
    const native = Libhegel.load(testLibPath());
    const drive = (engine: Engine) =>
      active(engine, (ctx, tc) => {
        const trace: unknown[] = [];
        engine.setUnicodeVersion(ctx, "16.0");
        expect(() => engine.setUnicodeVersion(ctx, "not a version")).toThrow(/version/);
        engine.setUnicodeVersion(ctx, process.versions["unicode"] ?? "");
        const recursion = engine.newRecursion(ctx, tc, 5, 3);
        const pool = engine.newPool(ctx, tc);
        try {
          const subtree = (depth: number): boolean => {
            const branch = engine.recursionBranch(ctx, tc, recursion, depth);
            trace.push(branch);
            if (!branch) return engine.recursionLeaf(ctx, tc, recursion);
            for (let i = 0; i < 3; i++) if (!subtree(depth + 1)) return false;
            return true;
          };
          for (let attempt = 0; attempt < 3; attempt++) {
            if (!subtree(0)) {
              trace.push("retry");
              engine.recursionRetry(ctx, tc, recursion);
              continue;
            }
            const finished = engine.recursionFinish(ctx, tc, recursion);
            trace.push(finished);
            if (finished) break;
          }
          engine.poolAdd(ctx, tc, pool);
          const id = engine.poolGenerate(ctx, tc, pool, true);
          expect(() => engine.poolGenerate(ctx, tc, pool, true)).toThrow(AssumeError);
          engine.poolRestore(ctx, tc, pool, id);
          expect(engine.poolGenerate(ctx, tc, pool, true)).toBe(id);
          expect(() => engine.poolRestore(ctx, tc, pool, id + 1n)).toThrow(EngineError);
          return trace;
        } finally {
          engine.freePool(pool);
          engine.freeRecursion(recursion);
        }
      });
    expect(drive(wasm)).toEqual(drive(native));
  });

  it("maps RETRY from the Wasm recursion protocol to false and checks other codes", () => {
    const { engine, abi } = wasmFixture();
    active(engine, (ctx, tc) => {
      const recursion = engine.newRecursion(ctx, tc, 5, 3);
      const call = abi.call.bind(abi);
      const spy = vi.spyOn(abi, "call").mockImplementation((op, ...args) => {
        if (op === "recursion_leaf") return -10;
        if (op === "recursion_finish") return -1;
        return call(op, ...args);
      });
      try {
        expect(engine.recursionLeaf(ctx, tc, recursion)).toBe(false);
        expect(() => engine.recursionFinish(ctx, tc, recursion)).toThrow(StopTestError);
      } finally {
        spy.mockRestore();
        engine.freeRecursion(recursion);
      }
    });
  });

  it("draws recursive values and filtered pool values in the browser runner", () => {
    const runner = createRunner(browserRuntime(wasmFixture().engine));
    type Tree = number | Tree[];
    const trees = gs.recursive<Tree>(gs.integers({ minValue: 0, maxValue: 3 }), (children) =>
      gs.arrays(children, { minSize: 1, maxSize: 3 }),
    );
    runner.test(
      (tc) => {
        tc.draw(trees);
        const pool = new stateful.Pool<number>(tc);
        for (let i = 0; i < 4; i++) pool.add(i);
        expect(tc.draw(pool.valuesConsumed().filter((x) => x > 0))).toBeGreaterThan(0);
        expect(pool.size).toBe(3);
      },
      { testCases: 50, database: Database.disabled },
    );
  });

  it("runs a stateful test with a pool in the browser runner", () => {
    const runner = createRunner(browserRuntime(wasmFixture().engine));
    let steps = 0;
    expect(() =>
      runner.test(
        (tc) => {
          const handles = new stateful.Pool<number>(tc);
          stateful.run(
            tc,
            {
              rules: {
                alloc: (_tc, state: { live: Set<number>; next: number }) => {
                  const handle = state.next++;
                  handles.add(handle);
                  state.live.add(handle);
                  steps++;
                },
                free: (tc, state) => {
                  const handle = tc.draw(handles.valuesConsumed());
                  state.live.delete(handle);
                  steps++;
                  if (state.next > 3) throw new Error(`freed ${handle} late`);
                },
              },
              invariants: {
                liveMatchesPool: (_tc, state) => {
                  expect(state.live.size).toBe(handles.size);
                },
              },
            },
            { live: new Set<number>(), next: 0 },
          );
        },
        { seed: 42, testCases: 30 },
      ),
    ).toThrow(/freed \d+ late/);
    expect(steps).toBeGreaterThan(0);
  });

  it("draws NaN and infinities and composes the portable generator helpers", () => {
    const runner = createRunner(browserRuntime(wasmFixture().engine));
    let nan = false,
      infinite = false;
    runner.test(
      (tc) => {
        const value = tc.draw(gs.floats());
        nan ||= Number.isNaN(value);
        infinite ||= value === Infinity || value === -Infinity;
      },
      { testCases: 200, seed: 42 },
    );
    expect(nan).toBe(true);
    expect(infinite).toBe(true);
  });

  it("runs every schema category through the single interpreter", () => {
    const { engine } = wasmFixture();
    const runner = createRunner(browserRuntime(engine));
    runner.test(
      (tc) => {
        tc.draw(gs.booleans());
        tc.draw(gs.integers());
        tc.draw(gs.bigIntegers());
        tc.draw(gs.floats());
        tc.draw(gs.binary({ minSize: 1, maxSize: 10 }));
        tc.draw(gs.text({ maxSize: 10 }));
        tc.draw(gs.fromRegex("a[bc]+", { fullmatch: true }));
        expect(tc.draw(gs.fromRegex("a[bc]+", { alphabet: "ab" }))).toMatch(/^ab+$/);
        tc.draw(gs.emails());
        tc.draw(gs.urls());
        tc.draw(gs.domains());
        tc.draw(gs.dates());
        tc.draw(gs.times());
        tc.draw(gs.datetimes());
        tc.draw(gs.uuids());
        tc.draw(gs.uuids({ version: 4 }));
        tc.draw(gs.ipAddresses({ version: 4 }));
        tc.draw(gs.ipAddresses({ version: 6 }));
        tc.draw(gs.arrays(gs.integers(), { maxSize: 5 }));
        tc.draw(gs.maps(gs.integers(), gs.booleans(), { maxSize: 5 }));
        tc.draw(gs.tuples(gs.integers(), gs.text({ maxSize: 2 })));
        tc.draw(gs.oneOf(gs.just(null), gs.integers()));
      },
      {
        testCases: 15,
        seed: 42,
        database: Database.default,
        suppressHealthCheck: [HealthCheck.TooSlow],
      },
    );
    active(engine, (ctx, tc) => {
      expect(
        engine.generateFloat(ctx, tc, {
          width: 64,
          minValue: Infinity,
          maxValue: Infinity,
          allowNan: false,
          allowInfinity: true,
          excludeMin: false,
          excludeMax: false,
          smallestNonzeroMagnitude: 5e-324,
        }),
      ).toBe(Infinity);
    });
  });
});

describe("WasmEngine real run lifecycle", () => {
  it("passes sync and async bodies with assumptions and notes", async () => {
    const { engine } = wasmFixture();
    const runtime = browserRuntime(engine),
      runner = createRunner(runtime);
    const output = vi.spyOn(runtime, "output");
    runner.test(
      (tc) => {
        tc.assume(tc.draw(gs.integers({ minValue: 0, maxValue: 10 })) > 0);
        tc.note("passing");
      },
      { testCases: 10, seed: 42 },
    );
    await runner.testAsync(
      async (tc) => {
        await Promise.resolve();
        expect(tc.draw(gs.booleans())).toBeTypeOf("boolean");
      },
      { testCases: 5, seed: 42 },
    );
    expect(output).not.toHaveBeenCalled();
  });

  it("shrinks and reports a failure with notes and a blob native replays, in both drivers", async () => {
    for (const async of [false, true]) {
      const { engine } = wasmFixture();
      const runtime = browserRuntime(engine);
      const lines: string[] = [];
      runtime.output = (line) => lines.push(line);
      const runner = createRunner(runtime);
      const body = (tc: import("../src/testCase.js").TestCase) => {
        const n = tc.draw(gs.integers({ minValue: 0, maxValue: 1000 }));
        tc.note("replay note");
        if (n >= 50) throw new Error(`bad ${n}`);
      };
      const settings = { seed: 42, printBlob: true };
      if (async) await expect(runner.testAsync(body, settings)).rejects.toThrow("bad 50");
      else expect(() => runner.test(body, settings)).toThrow("bad 50");
      expect(lines.slice(0, 4)).toEqual([
        "const draw_1 = 50;",
        "replay note",
        "",
        "To reproduce this failure, replay it with the Hegel builder:",
      ]);
      const blob = JSON.parse(lines[4].trim().replace(/^\.reproduceFailure\((.*)\)$/, "$1"));
      const native = createRunner(fakeRuntime(Libhegel.load(testLibPath())));
      expect(() => new native.Hegel(body).reproduceFailure(blob).runSync()).toThrow("bad 50");
    }
  });

  it("prints the same report as the native engine for every kind of output", () => {
    const body = (tc: import("../src/testCase.js").TestCase) => {
      const worker = tc.clone();
      worker.setWorker(2);
      tc.note("first line\nsecond line");
      tc.target(1, "t");
      tc.event("e");
      tc.eventValue("v", 1);
      const values = tc.draw(gs.arrays(gs.text({ maxSize: 3 }), { minSize: 1 }), "values");
      worker.draw(gs.booleans());
      tc.repeat(() => {
        if (tc.draw(gs.integers({ minValue: 0, maxValue: 10 })) > 2 && values.length > 0) {
          throw new Error("failed");
        }
      });
    };
    const reports: string[][] = [];
    for (const runtime of [
      browserRuntime(wasmFixture().engine),
      fakeRuntime(Libhegel.load(testLibPath())),
    ]) {
      const lines: string[] = [];
      runtime.output = (line) => lines.push(line);
      expect(() =>
        createRunner(runtime).test(body, {
          database: Database.disabled,
          derandomize: true,
          showStatistics: true,
        }),
      ).toThrow("failed");
      reports.push(lines.map((line) => line.replace(/\+\d+\.\d+ms/, "+TIME")));
    }
    expect(reports[0]).toEqual(reports[1]);
    expect(reports[0].slice(0, 6)).toEqual([
      "[worker 2 +TIME] const draw_1 = false;",
      "first line",
      "second line",
      'const values = [""];',
      "// Repetition #1",
      "  const draw_2 = 3;",
    ]);
  });

  it("lays out structured values the same as the native engine", () => {
    class Interval {
      constructor(
        readonly lo: number,
        readonly hi: number,
      ) {}
      [prettyPrint](p: PrettyPrinter): void {
        p.group(9, "Interval(", ")", () => {
          p.value(this.lo);
          p.text(",");
          p.breakable();
          p.value(this.hi);
          p.ifBreak(",");
          p.shiftIndent(-9);
          p.breakable("");
          p.shiftIndent(9);
        });
        p.comment("half-open");
      }
    }
    class Span {
      constructor(
        readonly start: unknown,
        readonly end: unknown,
      ) {}
    }
    const body = (tc: import("../src/testCase.js").TestCase) => {
      tc.draw(gs.just(new RangeError("bad")), "error");
      tc.draw(gs.just(new Interval(1, 2)), "interval");
      tc.draw(gs.just(new Interval(10 ** 20, -(10 ** 20)).hi), "plain");
      tc.draw(gs.just(Array.from({ length: 20 }, (_, i) => i * 1000)), "long");
      tc.draw(
        gs.just({ name: "x".repeat(60), tags: new Set(["a"]), at: new Map([[1, [2]]]) }),
        "record",
      );
      tc.draw(gs.just(new Span("y".repeat(50), new Span(1, 2))), "span");
      throw new Error("failed");
    };
    const reports: string[][] = [];
    for (const runtime of [
      browserRuntime(wasmFixture().engine),
      fakeRuntime(Libhegel.load(testLibPath())),
    ]) {
      const lines: string[] = [];
      runtime.output = (line) => lines.push(line);
      expect(() =>
        createRunner(runtime).test(body, { database: Database.disabled, printBlob: false }),
      ).toThrow("failed");
      reports.push(lines);
    }
    expect(reports[0]).toEqual(reports[1]);
    expect(reports[0]).toEqual([
      'const error = new RangeError("bad");',
      "const interval = Interval(1, 2);  // half-open",
      "const plain = -100000000000000000000;",
      "const long = [",
      ...Array.from({ length: 20 }, (_, i) => `    ${i * 1000},`),
      "];",
      "const record = {",
      `    name: "${"x".repeat(60)}",`,
      '    tags: new Set(["a"]),',
      "    at: new Map([[1, [2]]]),",
      "};",
      "const span = Object.assign(Object.create(Span.prototype), {",
      `    start: "${"y".repeat(50)}",`,
      "    end: Object.assign(Object.create(Span.prototype), { start: 1, end: 2 }),",
      "});",
    ]);
  });

  it.each([false, true])(
    "reports a real flaky run's own error and caveat without frontend replay, async=%s",
    async (async) => {
      const { engine } = wasmFixture();
      const status = vi.spyOn(engine, "runStatus");
      const failureCount = vi.spyOn(engine, "failureCount");
      const failure = vi.spyOn(engine, "failure");
      const free = vi.spyOn(engine, "freeContext");
      const runtime = browserRuntime(engine);
      runtime.output = vi.fn();
      const runner = createRunner(runtime);
      const flaky = new Error("flaky failure");
      let seen = false;
      const body = (tc: import("../src/testCase.js").TestCase) => {
        const n = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        if (n > 0 && !seen) {
          seen = true;
          throw flaky;
        }
      };
      let thrown: unknown;
      try {
        if (async) await runner.testAsync(body, { seed: 42 });
        else runner.test(body, { seed: 42 });
      } catch (error) {
        thrown = error;
      }
      expect(thrown).toBe(flaky);
      expect(runtime.output).toHaveBeenCalledTimes(1);
      expect(seen).toBe(true);
      expect(status).toHaveReturnedWith(RunStatus.FAILED);
      expect(failureCount).toHaveBeenCalledTimes(1);
      expect(failure).toHaveBeenCalledTimes(1);
      expect(free).toHaveBeenCalledTimes(1);
    },
  );

  it("reports multiple independent origins", () => {
    const { engine } = wasmFixture();
    const runtime = browserRuntime(engine);
    const lines: string[] = [];
    runtime.output = (line) => lines.push(line);
    let thrown: unknown;
    try {
      createRunner(runtime).test(
        (tc) => {
          const n = tc.draw(gs.integers({ minValue: 0, maxValue: 10 }));
          if (n < 3) throw new Error("low");
          if (n > 7) throw new Error("high");
        },
        { seed: 42, reportMultipleFailures: true },
      );
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(AggregateError);
    expect((thrown as AggregateError).message).toBe(
      "Property-based test failed with 2 distinct failures.",
    );
    expect((thrown as AggregateError).errors.map((e: Error) => e.message).sort()).toEqual([
      "high",
      "low",
    ]);
    expect(lines[0]).toBe("Property-based test failed with 2 distinct failures.");
    expect(lines.filter((line) => line.startsWith("const draw_1 = ")).length).toBe(2);
  });

  it("reports real health errors without replay and rejects filesystem settings", () => {
    const { engine } = wasmFixture();
    const runner = createRunner(browserRuntime(engine));
    expect(() => runner.test((tc) => tc.assume(false), { seed: 42 })).toThrow(/Unsatisfiable/i);
    expect(() => runner.test(() => {}, { database: Database.fromPath("") })).toThrow(/Filesystem/);
  });

  it("handles engine overrun and explicit completion, snapshots and blob replay", () => {
    const { engine } = wasmFixture();
    for (const adapter of [engine, Libhegel.load(testLibPath())])
      active(adapter, (ctx, tc) => {
        expect(() => {
          // The engine concludes a case as overrun once it has made 2^20 choices.
          for (let i = 0; i <= 1 << 20; i++) adapter.generateBoolean(ctx, tc, 0.5);
        }).toThrow(StopTestError);
        expect(() => adapter.newCollection(ctx, tc, 0, 5)).toThrow(StopTestError);
        adapter.markComplete(ctx, tc, Status.OVERRUN, null);
      });
    const ctx = engine.newContext(),
      settings = engine.newSettings(ctx);
    engine.setDatabase(ctx, settings, null);
    engine.setTestCases(settings, 1);
    // Every health check except TestCasesTooLarge (bit 2): suppressing that
    // one also lifts the choice limit the overrun above relies on.
    engine.setSuppressHealthCheck(settings, 1 | 2 | 8);
    engine.setDerandomize(settings, true);
    engine.setVerbosity(settings, NativeVerbosity.QUIET);
    engine.setDatabaseKey(ctx, settings, "Unicode 🙂");
    const run = engine.runStart(ctx, settings);
    let tc;
    while ((tc = engine.nextTestCase(ctx, run)) !== null) {
      try {
        engine.generateInteger(ctx, tc, 42n, 42n);
        engine.markComplete(ctx, tc, Status.INTERESTING, "origin");
      } catch (error) {
        if (!(error instanceof StopTestError)) throw error;
        engine.markComplete(ctx, tc, Status.OVERRUN, null);
      } finally {
        engine.freeTestCase(tc);
      }
    }
    const result = engine.runResult(ctx, run);
    expect(engine.runStatus(result)).toBe(RunStatus.FAILED);
    expect(engine.runError(result)).toBe(null);
    expect(engine.failureCount(result)).toBe(1);
    const failure = engine.failure(result, 0);
    const blob = engine.reproductionBlob(failure)!;
    expect(engine.failureOrigin(failure)).toBe("origin");
    engine.freeFailure(failure);
    engine.freeRunResult(result);
    engine.freeRun(run);
    const replay = engine.runStartBlob(ctx, settings, blob);
    let replayed = 0;
    while ((tc = engine.nextTestCase(ctx, replay)) !== null) {
      expect(engine.generateInteger(ctx, tc, 42n, 42n)).toBe(42n);
      engine.markComplete(ctx, tc, Status.INTERESTING, "origin");
      engine.freeTestCase(tc);
      replayed++;
    }
    expect(replayed).toBeGreaterThan(0);
    const replayResult = engine.runResult(ctx, replay);
    expect(engine.runStatus(replayResult)).toBe(RunStatus.FAILED);
    engine.freeRunResult(replayResult);
    engine.freeRun(replay);
    engine.freeSettings(settings);
    engine.freeContext(ctx);
  });
});

describe("WasmEngine error and ownership boundaries", () => {
  it("copies diagnostics before cleanup, checks settings and rejects stale/foreign handles", () => {
    const { engine } = wasmFixture();
    const ctx = engine.newContext(),
      s = engine.newSettings(ctx);
    expect(engine.lastError(ctx)).toBe("");
    engine.setDatabaseKey(ctx, s, "key\0with NUL");
    expect(() => engine.setDatabase(ctx, s, "path")).toThrow(/Filesystem/);
    expect(() => engine.runStartBlob(ctx, s, "bad blob")).toThrow(/failed/);
    expect(() => engine.setVerbosity(s, 99)).toThrow(/failed/);
    expect(() => wasmFixture().engine.freeSettings(s)).toThrow(/Foreign/);
    engine.freeSettings(s);
    expect(() => engine.freeSettings(s)).toThrow(/freed/);
    engine.freeContext(ctx);
  });

  it.each([-1, -2, -3, -4, -5, -6, -7, -8, -9, -10])(
    "maps draw code %s without interesting counterexamples",
    (code) => {
      const { engine, abi } = wasmFixture();
      active(engine, (ctx, tc) => {
        const call = abi.call.bind(abi);
        vi.spyOn(abi, "call").mockImplementation((op, ...args) =>
          op === "generate_boolean" ? code : call(op, ...args),
        );
        expect(() => engine.generateBoolean(ctx, tc, 0.5)).toThrow(
          code === -1 ? StopTestError : code === -2 ? AssumeError : EngineError,
        );
        vi.spyOn(abi, "call").mockImplementation((op, ...args) =>
          op === "new_collection" ? code : call(op, ...args),
        );
        expect(() => engine.newCollection(ctx, tc, 0, 5)).toThrow(
          code === -1 ? StopTestError : code === -2 ? AssumeError : EngineError,
        );
      });
    },
  );

  it("runs exact result destructor after corrupted buffer output and releases the slot", () => {
    const { engine, abi } = wasmFixture();
    active(engine, (ctx, tc) => {
      const call = abi.call.bind(abi);
      let data = 0,
        result = 0;
      const frees = vi.spyOn(abi, "dealloc");
      const spy = vi.spyOn(abi, "call").mockImplementation((op, ...args) => {
        if (op === "generate_bytes") {
          const code = call(op, ...args);
          result = Number(args.at(-1));
          const view = new DataView(abi.memory.buffer);
          data = view.getUint32(result, true);
          view.setUint32(result, 0xffffffff, true);
          return code;
        }
        if (op === "generate_bytes_result_free")
          new DataView(abi.memory.buffer).setUint32(result, data, true);
        return call(op, ...args);
      });
      expect(() => engine.generateBytes(ctx, tc, 2, 2n)).toThrow(/range/);
      expect(spy).toHaveBeenCalledWith("generate_bytes_result_free", 0, result);
      expect(frees).toHaveBeenCalledWith(result, 8, 4);
    });
  });

  it("does not swallow a trapped draw and attempts INVALID completion and handle cleanup", () => {
    const { engine, abi } = wasmFixture();
    const original = abi.call.bind(abi);
    vi.spyOn(abi, "call").mockImplementation((op, ...args) => {
      if (op === "generate_boolean") throw new WebAssembly.RuntimeError("unreachable");
      return original(op, ...args);
    });
    const complete = vi.spyOn(engine, "markComplete"),
      free = vi.spyOn(engine, "freeContext");
    expect(() =>
      createRunner(browserRuntime(engine)).test((tc) => {
        try {
          tc.draw(gs.booleans());
        } catch {
          /* The runner remembers adapter faults. */
        }
      }),
    ).toThrow(/unreachable/);
    expect(complete).toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      Status.INVALID,
      null,
    );
    expect(free).toHaveBeenCalledTimes(1);
  });

  it("preserves allocation size/alignment for direct scoped use", () => {
    const { abi } = wasmFixture();
    const free = vi.spyOn(abi, "dealloc");
    let pointer = 0;
    expect(() =>
      WasmArena.scoped(abi, (a) => {
        pointer = a.alloc(16, 8);
        throw new Error("body");
      }),
    ).toThrow("body");
    expect(free).toHaveBeenCalledExactlyOnceWith(pointer, 16, 8);
  });
});

describe("WasmEngine settings, profiles and run errors", () => {
  it("applies every setting, resolves profiles and classifies run errors like Node", () => {
    const { engine } = wasmFixture();
    const runner = createRunner(browserRuntime(engine));
    let count = 0;
    const body = (tc: import("../src/testCase.js").TestCase) => {
      tc.draw(gs.integers());
      count++;
    };
    runner.test(body, {
      testCases: 3,
      seed: 2n ** 64n - 1n,
      derandomize: true,
      verbosity: Verbosity.Quiet,
      suppressHealthCheck: [],
      reportMultipleFailures: true,
      stopOnFirstFailure: false,
      phases: [Phase.Generate, Phase.Shrink],
      printBlob: false,
      showStatistics: false,
      unboundedChoices: true,
      nondeterminismStrictness: NondeterminismStrictness.Warn,
      backend: Backend.Default,
    });
    expect(count).toBe(3);
    runner.registerProfile("wasm-tiny", { testCases: 2, seed: null });
    count = 0;
    runner.test(body, { profile: "wasm-tiny" });
    expect(count).toBe(2);
    runner.setDefaultProfile("wasm-tiny");
    count = 0;
    try {
      runner.test(body);
    } finally {
      runner.setDefaultProfile(null);
    }
    expect(count).toBe(2);
    expect(() => runner.test(body, { profile: "no-such-wasm-profile" })).toThrow(
      /no-such-wasm-profile/,
    );
    expect(() => runner.registerProfile("base")).toThrow(/base/);
    expect(() => runner.setDefaultProfile("default")).toThrow(/default/);
    let error: unknown;
    try {
      runner.test((tc) => tc.assume(tc.draw(gs.integers({ minValue: 0, maxValue: 1000 })) === 5));
    } catch (caught) {
      error = caught;
    }
    expect(error).toBeInstanceOf(FailedHealthCheckError);
    expect((error as FailedHealthCheckError).healthCheck).toBe(HealthCheck.FilterTooMuch);
  });

  it("passes the test location and reads failure caveats", () => {
    const { engine } = wasmFixture();
    const setTestLocation = vi.spyOn(engine, "setTestLocation");
    const caveat = vi.spyOn(engine, "failureCaveat");
    const runtime = browserRuntime(engine);
    runtime.output = vi.fn();
    expect(() =>
      createRunner(runtime).test(function located(tc) {
        if (tc.draw(gs.integers({ minValue: 0, maxValue: 10 })) > 3) throw new Error("big");
      }),
    ).toThrow("big");
    expect(setTestLocation.mock.calls[0][2].function).toBe("located");
    expect(caveat).toHaveReturnedWith(null);
  });
});

describe.each([
  ["Wasm", () => wasmFixture().engine],
  ["native", () => Libhegel.load(testLibPath())],
])("deferred printer regions on the %s engine", (_name, load) => {
  it("splice in where they were opened, and die when the document is resolved", () => {
    const engine = load();
    const ctx = engine.newContext();
    const printer = engine.newPrinter(ctx, null);
    try {
      engine.printerText(ctx, printer, "f(");
      const slot = engine.printerDeferred(ctx, printer);
      engine.printerText(ctx, printer, ")");
      expect(engine.printerIsLive(ctx, printer)).toBe(true);
      expect(engine.printerIsLive(ctx, slot)).toBe(true);
      const inner = engine.printerDeferred(ctx, slot);
      engine.printerText(ctx, inner, "1");
      engine.printerResolve(ctx, printer);
      expect(engine.printerIsLive(ctx, slot)).toBe(false);
      expect(engine.printerValue(ctx, printer)).toBe("f(1)");
      expect(() => engine.printerDeferred(ctx, slot)).toThrow(/hegel_printer_deferred/);
      engine.freePrinter(inner);
      engine.freePrinter(slot);
    } finally {
      engine.freePrinter(printer);
      engine.freeContext(ctx);
    }
  });
});
