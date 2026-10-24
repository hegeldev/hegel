import { describe, it, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { createRunner } from "../src/runnerCore.js";
import { RunErrorKind, RunStatus } from "../src/engine.js";
import { runError } from "../src/errors.js";
import { fakeEngine, fakeRuntime } from "./fakeEngine.js";

function thrownBy(body: () => void): unknown {
  try {
    body();
  } catch (error) {
    return error;
  }
  throw new Error("expected the run to throw");
}

describe("run errors from the real engine", () => {
  it("throws FailedHealthCheckError naming the check", () => {
    const error = thrownBy(() =>
      hegel.test(
        (tc) => {
          tc.assume(tc.draw(gs.integers({ minValue: 0, maxValue: 1000 })) === 500);
        },
        { database: hegel.Database.disabled },
      ),
    );
    expect(error).toBeInstanceOf(hegel.FailedHealthCheckError);
    expect(error).toBeInstanceOf(hegel.RunError);
    expect((error as hegel.FailedHealthCheckError).healthCheck).toBe(
      hegel.HealthCheck.FilterTooMuch,
    );
    expect((error as Error).name).toBe("FailedHealthCheckError");
  });

  it("throws UnsatisfiableError when no test case is valid", () => {
    const error = thrownBy(() =>
      hegel.test((tc) => tc.assume(false), {
        database: hegel.Database.disabled,
        suppressHealthCheck: [hegel.HealthCheck.FilterTooMuch],
      }),
    );
    expect(error).toBeInstanceOf(hegel.UnsatisfiableError);
    expect((error as Error).name).toBe("UnsatisfiableError");
  });

  it("throws FlakyError for a nondeterministic test under the error strictness", () => {
    let calls = 0;
    const error = thrownBy(() =>
      hegel.test(
        (tc) => {
          tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          calls++;
          if (calls % 2 === 0) throw new Error("sometimes");
        },
        {
          database: hegel.Database.disabled,
          nondeterminismStrictness: hegel.NondeterminismStrictness.Error,
        },
      ),
    );
    expect(error).toBeInstanceOf(hegel.FlakyError);
    expect((error as Error).name).toBe("FlakyError");
  });
});

describe("run error classification", () => {
  it.each([
    [RunErrorKind.NONDETERMINISTIC, hegel.FlakyError],
    [RunErrorKind.USAGE, hegel.RunError],
    [RunErrorKind.INTERNAL, hegel.RunError],
  ])("maps kind %s", (kind, type) => {
    const error = runError("message", { kind, healthCheck: 0 });
    expect(error).toBeInstanceOf(type);
    expect(error.message).toBe("message");
    expect(error.name).toBe(type.name);
  });

  it("does not invent a health check or a message the engine did not give", () => {
    const error = runError(null, { kind: RunErrorKind.HEALTH_CHECK, healthCheck: 1 << 9 });
    expect(error).toBeInstanceOf(hegel.FailedHealthCheckError);
    expect((error as hegel.FailedHealthCheckError).healthCheck).toBe(null);
    expect(error.message).toBe("The run failed with an unknown error");
  });
});

describe("failures", () => {
  it("rethrows the test's own error, keeping its class and cause", () => {
    class CustomError extends Error {
      constructor(
        message: string,
        readonly value: number,
      ) {
        super(message, { cause: "the cause" });
      }
    }
    const error = thrownBy(() =>
      hegel.test(
        (tc) => {
          const n = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          if (n >= 10) throw new CustomError("too big", n);
        },
        { database: hegel.Database.disabled },
      ),
    );
    expect(error).toBeInstanceOf(CustomError);
    expect((error as CustomError).value).toBe(10);
    expect((error as CustomError).cause).toBe("the cause");
    expect((error as CustomError).message).toBe("too big");
  });

  it("keeps an assertion error's expected and actual values", () => {
    const error = thrownBy(() =>
      hegel.test(
        (tc) => {
          expect(tc.draw(gs.integers({ minValue: 0, maxValue: 100 }))).toBeLessThan(10);
        },
        { database: hegel.Database.disabled },
      ),
    );
    expect(error).toMatchObject({ actual: 10, expected: 10 });
  });

  it("prints a failure's caveat", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    engine.failureCaveat.mockReturnValue("confirmed 3 times out of 4");
    const runtime = fakeRuntime(engine);
    const failure = new Error("failure");
    expect(() =>
      createRunner(runtime).test(() => {
        throw failure;
      }),
    ).toThrow(failure);
    expect(runtime.note).toHaveBeenCalledWith("confirmed 3 times out of 4");
  });

  it("reports a caveated failure seen while exploring when it does not recur on replay", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    engine.failureCaveat.mockReturnValue("caveat");
    engine.failureOrigin.mockImplementation(() => engine.markComplete.mock.calls[0][3]!);
    const runtime = fakeRuntime(engine);
    const failure = new Error("failure");
    let thrown: unknown;
    try {
      createRunner(runtime).test((tc) => {
        if (!tc.isLastRun) throw failure;
      });
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBe(failure);
  });

  it("reports a caveated failure without a blob that exploration never saw", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    engine.failureCaveat.mockReturnValue("caveat");
    engine.reproductionBlob.mockReturnValue(null);
    expect(() => createRunner(fakeRuntime(engine)).test(() => {})).toThrow("no reproduction blob");
  });

  it("reports a caveated failure that neither recurs nor was seen as not reproducing", () => {
    const engine = fakeEngine();
    engine.runStatus.mockReturnValue(RunStatus.FAILED);
    engine.failureCaveat.mockReturnValue("caveat");
    expect(() => createRunner(fakeRuntime(engine)).test(() => {})).toThrow(
      "did not reproduce during final replay",
    );
  });
});
