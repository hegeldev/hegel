import { describe, it, expect, vi } from "vitest";
import { EngineError, RunStatus, Status } from "../src/engine.js";
import {
  createRunner,
  EngineDataSource,
  runTestCase,
  runTestCaseAsync,
} from "../src/runnerCore.js";
import { AssumeError, StopTestError } from "../src/testCase.js";
import * as gs from "../src/generators/index.js";
import { RunError } from "../src/errors.js";
import { fakeEngine, fakeRuntime, context, settings, testCase, failure } from "./fakeEngine.js";
import { labelSpec } from "../src/labels.js";

describe("runnerCore engine fault classification and cleanup", () => {
  for (const async of [false, true]) {
    it(`closes the ${async ? "async" : "sync"} generator on an adapter exception`, async () => {
      const engine = fakeEngine();
      const trap = new Error("adapter trap");
      engine.generateBoolean.mockImplementation(() => {
        throw trap;
      });
      const runner = createRunner(fakeRuntime(engine));
      const body = (tc: import("../src/testCase.js").TestCase) => {
        tc.draw(gs.booleans());
      };
      if (async) await expect(runner.testAsync(body)).rejects.toThrow("adapter trap");
      else expect(() => runner.test(body)).toThrow("adapter trap");
      expect(engine.nextTestCase).toHaveBeenCalledTimes(1);
      expect(engine.markComplete).toHaveBeenCalledExactlyOnceWith(
        context,
        testCase,
        Status.INVALID,
        null,
      );
      expect(engine.freeTestCase).toHaveBeenCalledExactlyOnceWith(testCase);
      expect(engine.freeRun).toHaveBeenCalledTimes(1);
      expect(engine.freeSettings).toHaveBeenCalledTimes(1);
      expect(engine.freeContext).toHaveBeenCalledTimes(1);
      expect(engine.runResult).not.toHaveBeenCalled();
    });

    it.each(["draw", "completion"])(
      `retains %s and outer-free faults in the ${async ? "async" : "sync"} driver`,
      async (stage) => {
        const engine = fakeEngine();
        const cause = new Error("original cause");
        const fault = new EngineError(`${stage} broke`, { cause });
        const freeFault = new EngineError("context free broke");
        engine[stage === "draw" ? "generateBoolean" : "markComplete"].mockImplementation(() => {
          throw fault;
        });
        engine.freeContext.mockImplementation(() => {
          throw freeFault;
        });
        const runner = createRunner(fakeRuntime(engine));
        const body = (tc: import("../src/testCase.js").TestCase) => tc.draw(gs.booleans());
        let error: unknown;
        try {
          if (async) await runner.testAsync(body);
          else runner.test(body);
        } catch (caught) {
          error = caught;
        }
        expect(error).toBeInstanceOf(AggregateError);
        expect((error as AggregateError).errors).toEqual([fault, freeFault]);
        expect((error as AggregateError).errors[0].cause).toBe(cause);
        expect(engine.markComplete).toHaveBeenCalledTimes(1);
        expect(engine.freeTestCase).toHaveBeenCalledTimes(1);
        expect(engine.freeRun).toHaveBeenCalledTimes(1);
        expect(engine.freeSettings).toHaveBeenCalledTimes(1);
        expect(engine.freeContext).toHaveBeenCalledTimes(1);
        expect(engine.runResult).not.toHaveBeenCalled();
      },
    );

    it(`does not retry failed completion in the ${async ? "async" : "sync"} driver`, async () => {
      const engine = fakeEngine();
      engine.markComplete.mockImplementation(() => {
        throw new EngineError("completion broke");
      });
      const runner = createRunner(fakeRuntime(engine));
      if (async) await expect(runner.testAsync(() => {})).rejects.toThrow("completion broke");
      else expect(() => runner.test(() => {})).toThrow("completion broke");
      expect(engine.markComplete).toHaveBeenCalledTimes(1);
      expect(engine.freeTestCase).toHaveBeenCalledTimes(1);
      expect(engine.freeRun).toHaveBeenCalledTimes(1);
      expect(engine.freeContext).toHaveBeenCalledTimes(1);
    });

    it(`retains draw and completion faults in the ${async ? "async" : "sync"} driver`, async () => {
      const engine = fakeEngine();
      engine.generateBoolean.mockImplementation(() => {
        throw new EngineError("draw broke");
      });
      engine.markComplete.mockImplementation(() => {
        throw new EngineError("completion broke");
      });
      const runner = createRunner(fakeRuntime(engine));
      const body = (tc: import("../src/testCase.js").TestCase) => tc.draw(gs.booleans());
      let error: unknown;
      try {
        if (async) await runner.testAsync(body);
        else runner.test(body);
      } catch (caught) {
        error = caught;
      }
      expect(error).toBeInstanceOf(AggregateError);
      expect((error as AggregateError).errors.map((e: Error) => e.message)).toEqual([
        "draw broke",
        "completion broke",
      ]);
      expect(engine.markComplete).toHaveBeenCalledTimes(1);
      expect(engine.freeTestCase).toHaveBeenCalledTimes(1);
    });
  }

  it("does not let user code swallow an adapter fault", () => {
    const engine = fakeEngine();
    engine.generateBoolean.mockImplementation(() => {
      throw new EngineError("persistent fault");
    });
    expect(() =>
      createRunner(fakeRuntime(engine)).test((tc) => {
        try {
          tc.draw(gs.booleans());
        } catch {
          /* Deliberately catch user-visible error. */
        }
      }),
    ).toThrow("persistent fault");
    expect(engine.markComplete).toHaveBeenCalledWith(context, testCase, Status.INVALID, null);
  });

  it.each([new AssumeError(), new StopTestError()])(
    "keeps ordinary draw control flow out of failures",
    (error) => {
      const engine = fakeEngine();
      engine.generateBoolean.mockImplementation(() => {
        throw error;
      });
      createRunner(fakeRuntime(engine)).test((tc) => tc.draw(gs.booleans()));
      expect(engine.markComplete).toHaveBeenCalledWith(
        context,
        testCase,
        error instanceof AssumeError ? Status.INVALID : Status.OVERRUN,
        null,
      );
    },
  );

  it("does not treat an engine fault thrown by the test itself as a failure", () => {
    const engine = fakeEngine();
    const fault = new EngineError("thrown by the test");
    expect(() =>
      createRunner(fakeRuntime(engine)).test(() => {
        throw fault;
      }),
    ).toThrow(fault);
    expect(engine.markComplete).toHaveBeenCalledWith(context, testCase, Status.INVALID, null);
  });

  it("rejects duplicate completion attempts before calling the engine", () => {
    const engine = fakeEngine();
    const ds = new EngineDataSource(engine, context, testCase);
    ds.markComplete(Status.VALID, null);
    expect(() => ds.markComplete(Status.VALID, null)).toThrow("already attempted");
    expect(engine.markComplete).toHaveBeenCalledTimes(1);
    ds.dispose();
  });

  it("releases context when settings construction fails", () => {
    const engine = fakeEngine();
    engine.newSettings.mockImplementation(() => {
      throw new EngineError("settings allocation");
    });
    expect(() => createRunner(fakeRuntime(engine)).test(() => {})).toThrow("settings allocation");
    expect(engine.freeContext).toHaveBeenCalledTimes(1);
    expect(engine.freeSettings).not.toHaveBeenCalled();
  });

  it("validates runtime settings before acquiring the engine", () => {
    const runtime = fakeRuntime(fakeEngine());
    runtime.getEngine = vi.fn(runtime.getEngine);
    runtime.validateSettings = () => {
      throw new Error("unsupported database");
    };
    expect(() => createRunner(runtime).test(() => {})).toThrow("unsupported database");
    expect(runtime.getEngine).not.toHaveBeenCalled();
  });

  it("releases all collections, case, cache and outer handles when one collection free fails", () => {
    const engine = fakeEngine();
    engine.freeCollection.mockImplementationOnce(() => {
      throw new EngineError("collection free");
    });
    expect(() =>
      createRunner(fakeRuntime(engine)).test((tc) => {
        tc.draw(gs.text());
        tc.dataSource().newCollection(0, 1);
        tc.dataSource().newCollection(0, 1);
      }),
    ).toThrow("collection free");
    expect(engine.freeCollection).toHaveBeenCalledTimes(2);
    expect(engine.freeStringGenerator).toHaveBeenCalledTimes(1);
    expect(engine.freeTestCase).toHaveBeenCalledTimes(1);
    expect(engine.freeRun).toHaveBeenCalledTimes(1);
    expect(engine.freeSettings).toHaveBeenCalledTimes(1);
    expect(engine.freeContext).toHaveBeenCalledTimes(1);
  });

  it("keeps stop-span engine faults visible", () => {
    const engine = fakeEngine();
    engine.stopSpan.mockImplementation(() => {
      throw new EngineError("stop span");
    });
    expect(() =>
      createRunner(fakeRuntime(engine)).test((tc) => {
        tc.startSpan(labelSpec("span"));
        tc.stopSpan();
      }),
    ).toThrow("stop span");
    expect(engine.markComplete).toHaveBeenCalledWith(context, testCase, Status.INVALID, null);
  });
});

describe("runnerCore run results", () => {
  // 3 is the retired FAILED_NONDETERMINISTIC value: like any other unknown
  // status it is now rejected rather than special-cased.
  it.each([3, 99, RunStatus.ERROR])("reports status %s without reading failures", (status) => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(status);
    expect(() => createRunner(fakeRuntime(engine)).test(() => {})).toThrow(
      status === RunStatus.ERROR ? /backend error/ : /Unknown Hegel/,
    );
    expect(engine.failureCount).not.toHaveBeenCalled();
    expect(engine.freeRunResult).toHaveBeenCalledTimes(1);
    expect(engine.freeRun).toHaveBeenCalledTimes(1);
  });

  it("rejects a failure the test never reported", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    expect(() => createRunner(fakeRuntime(engine)).test(() => {})).toThrow(
      "Hegel failure property.ts:1 was never seen by the test",
    );
    expect(engine.freeFailure).toHaveBeenCalledWith(failure);
    expect(engine.freeRunResult).toHaveBeenCalledTimes(1);
  });

  it("throws the captured error of the reported failure without replaying it", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    engine.failureOrigin.mockReturnValue("<unknown>");
    engine.shouldCapture.mockReturnValue(true);
    const runtime = fakeRuntime(engine);
    const thrown = "not an error";
    let caught: unknown;
    try {
      createRunner(runtime).test((tc) => {
        tc.draw(gs.booleans());
        throw thrown;
      });
    } catch (error) {
      caught = error;
    }
    expect(caught).toBe(thrown);
    expect(engine.nextTestCase).toHaveBeenCalledTimes(2);
    expect(vi.mocked(runtime.output).mock.calls).toEqual([["const draw_1 = true;"]]);
  });

  it("passes an explicit test location to the engine and keys the database by it", () => {
    const engine = fakeEngine();
    const location = { function: "property", file: "test.ts", class: "tests", beginLine: 1 };
    new (createRunner(fakeRuntime(engine)).Hegel)(() => {}).testLocation(location).runSync();
    expect(engine.setTestLocation).toHaveBeenCalledWith(context, settings, {
      file: "test.ts",
      beginLine: 1,
      className: "tests",
      function: "property",
    });
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(context, settings, "test.ts::property");
  });
});

describe("runnerCore origin formats", () => {
  it.each([
    [
      "Error: failure\n    at dep (file:///node_modules/dep.js:1:1)\n    at property (file:///test.ts:4:5)",
      "at property (file:///test.ts:4:5)",
    ],
    [
      "dep@https://host/node_modules/dep.js:1:1\nproperty@https://host/test.ts:4:5",
      "property@https://host/test.ts:4:5",
    ],
    ["@https://host/test.ts:8:9", "@https://host/test.ts:8:9"],
    ["Error: no frames", "<unknown>"],
  ])("extracts an origin without assuming V8", (stack, origin) => {
    const engine = fakeEngine();
    const ds = new EngineDataSource(engine, context, testCase);
    const error = new Error("failure");
    error.stack = stack;
    const result = runTestCase(
      ds,
      () => {
        throw error;
      },
      false,
    );
    expect(result.status).toBe("interesting");
    expect(engine.markComplete).toHaveBeenCalledWith(context, testCase, Status.INTERESTING, origin);
    ds.dispose();
  });
});

it("EngineDataSource disposes its own cache after collection cleanup fails", () => {
  const engine = fakeEngine();
  const ds = new EngineDataSource(engine, context, testCase);
  ds.newCollection(0);
  ds.generate({ type: "email" });
  engine.freeCollection.mockImplementation(() => {
    throw new EngineError("collection free");
  });
  engine.freeStringGenerator.mockImplementation(() => {
    throw new EngineError("generator free");
  });
  expect(() => ds.dispose()).toThrow(AggregateError);
  expect(engine.freeCollection).toHaveBeenCalledTimes(1);
  expect(engine.freeStringGenerator).toHaveBeenCalledTimes(1);
  ds.dispose();
});

it("runTestCaseAsync reports an interesting case with its origin", async () => {
  const engine = fakeEngine();
  const ds = new EngineDataSource(engine, context, testCase);
  const result = await runTestCaseAsync(ds, () => {
    throw new Error("user failure");
  });
  expect(result.status).toBe("interesting");
  expect(engine.markComplete).toHaveBeenCalledWith(
    context,
    testCase,
    Status.INTERESTING,
    expect.any(String),
  );
  ds.dispose();
});

it("retains a run error through multiple failing outer destructors", () => {
  const engine = fakeEngine();
  engine.runStatus.mockReturnValue(RunStatus.ERROR);
  const resultFault = new EngineError("result free");
  const runFault = new EngineError("run free");
  const cacheFault = new EngineError("cache free");
  const settingsFault = new EngineError("settings free");
  const contextFault = new EngineError("context free");
  engine.freeRunResult.mockImplementation(() => {
    throw resultFault;
  });
  engine.freeRun.mockImplementation(() => {
    throw runFault;
  });
  engine.freeStringGenerator.mockImplementation(() => {
    throw cacheFault;
  });
  engine.freeSettings.mockImplementation(() => {
    throw settingsFault;
  });
  engine.freeContext.mockImplementation(() => {
    throw contextFault;
  });
  let error: unknown;
  try {
    createRunner(fakeRuntime(engine)).test((tc) => tc.draw(gs.text()));
  } catch (caught) {
    error = caught;
  }
  const flatten = (error: unknown): unknown[] =>
    error instanceof AggregateError ? error.errors.flatMap(flatten) : [error];
  const errors = flatten(error);
  expect(errors[0]).toBeInstanceOf(RunError);
  expect((errors[0] as Error).message).toBe("backend error");
  expect(errors.slice(1)).toEqual([resultFault, runFault, cacheFault, settingsFault, contextFault]);
  for (const free of [
    engine.freeRunResult,
    engine.freeRun,
    engine.freeStringGenerator,
    engine.freeSettings,
    engine.freeContext,
  ])
    expect(free).toHaveBeenCalledTimes(1);
});

it("retains a failure-snapshot read error if its destructor also fails", () => {
  const engine = fakeEngine();
  engine.runStatus.mockReturnValue(RunStatus.FAILED);
  const readFault = new EngineError("blob read");
  const freeFault = new EngineError("failure free");
  engine.reproductionBlob.mockImplementation(() => {
    throw readFault;
  });
  engine.freeFailure.mockImplementation(() => {
    throw freeFault;
  });
  let error: unknown;
  try {
    createRunner(fakeRuntime(engine)).test(() => {});
  } catch (caught) {
    error = caught;
  }
  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([readFault, freeFault]);
  expect(engine.freeRunResult).toHaveBeenCalledTimes(1);
  expect(engine.freeContext).toHaveBeenCalledTimes(1);
});
