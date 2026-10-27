import { describe, expect, it } from "vitest";
import * as gs from "../src/generators/index.js";
import { createRunner, Phase, Verbosity, type Settings } from "../src/runnerCore.js";
import { nodeRuntime } from "../src/nodeRuntime.js";
import { DidNotReproduceError } from "../src/errors.js";
import { EngineError } from "../src/engine.js";
import { ExplicitExampleError, type TestCase } from "../src/testCase.js";
import { browserRuntime } from "../src/browser/runtime.js";
import { wasmFixture } from "./wasmFixture.js";

const deterministic: Settings = { database: { kind: "disabled" }, derandomize: true };

function capturing() {
  const lines: string[] = [];
  const runner = createRunner({ ...nodeRuntime, output: (line) => lines.push(line) });
  return { lines, runner };
}

function thrownBy(body: () => void): unknown {
  try {
    body();
  } catch (error) {
    return error;
  }
  throw new Error("expected the run to throw");
}

async function rejectionOf(body: () => Promise<void>): Promise<unknown> {
  try {
    await body();
  } catch (error) {
    return error;
  }
  throw new Error("expected the run to reject");
}

function blobOf(lines: string[]): string {
  const line = lines.find((l) => l.startsWith("    .reproduceFailure("))!;
  return JSON.parse(line.trim().slice(".reproduceFailure(".length, -1)) as string;
}

describe("the report of a failing example", () => {
  it("prints each draw, named or numbered, between the notes, and how to reproduce it", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      runner.test(
        (tc) => {
          const xs = tc.draw(gs.arrays(gs.integers({ minValue: 0, maxValue: 9 })));
          tc.note("after the list");
          tc.draw(gs.booleans(), "flag");
          tc.draw(gs.booleans(), "flag");
          tc.draw(gs.text({ maxSize: 0 }));
          tc.drawSilent(gs.integers());
          if (xs.length > 0) throw new Error("non-empty");
        },
        { ...deterministic, printBlob: true },
      ),
    );
    expect((error as Error).message).toBe("non-empty");
    expect(lines.slice(0, 7)).toEqual([
      "const draw_1 = [0];",
      "after the list",
      "const flag_1 = false;",
      "const flag_2 = false;",
      'const draw_2 = "";',
      "",
      "To reproduce this failure, replay it with the Hegel builder:",
    ]);
    expect(lines).toHaveLength(8);
  });

  it("numbers every name on its own, from its first use", () => {
    const { lines, runner } = capturing();
    thrownBy(() =>
      runner.test(
        (tc) => {
          tc.draw(gs.just(1), "draw_1");
          tc.draw(gs.just(2), "x_2");
          tc.draw(gs.just(3));
          tc.draw(gs.just(4), "x");
          tc.draw(gs.just(5), "x");
          throw new Error("fail");
        },
        { ...deterministic, printBlob: false },
      ),
    );
    expect(lines).toEqual([
      "const draw_1_1 = 1;",
      "const x_2_1 = 2;",
      "const draw_1 = 3;",
      "const x_1 = 4;",
      "const x_2 = 5;",
    ]);
  });

  it("prints nothing for a quiet run", () => {
    const { lines, runner } = capturing();
    thrownBy(() =>
      runner.test(
        (tc) => {
          tc.note("note");
          throw new Error(String(tc.draw(gs.booleans())));
        },
        { ...deterministic, verbosity: Verbosity.Quiet, printBlob: false },
      ),
    );
    expect(lines).toEqual([]);
  });

  it("prints every test case of a verbose run", () => {
    const { lines, runner } = capturing();
    runner.test(
      (tc) => {
        tc.draw(gs.just(7));
        tc.note("ran");
      },
      { ...deterministic, verbosity: Verbosity.Verbose, testCases: 3 },
    );
    expect(lines.filter((line) => line === "const draw_1 = 7;").length).toBeGreaterThan(0);
    expect(lines.filter((line) => line === "ran").length).toBe(
      lines.filter((line) => line === "const draw_1 = 7;").length,
    );
  });

  it("reports the failing example of a verbose run after printing every test case", () => {
    const { lines, runner } = capturing();
    thrownBy(() =>
      runner.test(
        (tc) => {
          if (tc.draw(gs.integers({ minValue: 0, maxValue: 100 })) > 0) throw new Error("positive");
        },
        { ...deterministic, verbosity: Verbosity.Verbose, printBlob: false },
      ),
    );
    expect(lines.at(-1)).toBe("const draw_1 = 1;");
    expect(lines.length).toBeGreaterThan(2);
  });

  it("reject abandons a test case like assume(false)", () => {
    const { runner } = capturing();
    let accepted = 0;
    runner.test(
      (tc) => {
        if (tc.draw(gs.integers({ minValue: 0, maxValue: 3 })) === 0) tc.reject();
        accepted++;
      },
      { ...deterministic, testCases: 20 },
    );
    expect(accepted).toBeGreaterThan(0);
  });
});

describe("targets and events", () => {
  it("records targets, events and event values", async () => {
    const { runner } = capturing();
    let maximum = 0;
    runner.test(
      (tc) => {
        const n = tc.draw(gs.integers({ minValue: 0, maxValue: 1000 }));
        tc.target(n);
        tc.target(-n, "smallness");
        tc.event(n % 2 === 0 ? "even" : "odd");
        tc.eventValue("n", n);
        maximum = Math.max(maximum, n);
      },
      { ...deterministic, testCases: 50, showStatistics: true },
    );
    expect(maximum).toBeGreaterThan(0);
    await runner.testAsync(
      async (tc) => {
        await Promise.resolve();
        tc.target(1, "async");
      },
      { ...deterministic, testCases: 2 },
    );
  });

  it("rejects a label targeted twice in one test case", () => {
    const { runner } = capturing();
    const error = thrownBy(() =>
      runner.test((tc) => {
        tc.target(1, "t");
        tc.target(2, "t");
      }, deterministic),
    );
    expect(error).toBeInstanceOf(EngineError);
    expect((error as Error).message).toMatch(/would overwrite/);
  });
});

describe("repeat", () => {
  it("reports each repetition under its heading", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      runner.test(
        (tc) => {
          let total = 0;
          tc.repeat(() => {
            const n = tc.draw(gs.integers({ minValue: 0, maxValue: 10 }));
            tc.assume(n !== 1);
            total += n;
            if (total > 15) throw new Error("too much");
          });
        },
        { ...deterministic, printBlob: false },
      ),
    );
    expect((error as Error).message).toBe("too much");
    expect(lines[0]).toBe("// Repetition #1");
    expect(lines.filter((line) => line.startsWith("// Repetition #")).length).toBe(2);
    expect(lines.filter((line) => line.startsWith("  const draw_")).length).toBe(2);
  });

  it("passes once the engine stops repeating", async () => {
    const { runner } = capturing();
    let repetitions = 0;
    runner.test(
      (tc) => {
        tc.repeat(() => {
          repetitions++;
          tc.assume(tc.draw(gs.booleans()));
        });
      },
      { ...deterministic, testCases: 2 },
    );
    expect(repetitions).toBeGreaterThan(0);
    await runner.testAsync(
      async (tc) => {
        await tc.repeatAsync(async () => {
          await Promise.resolve();
          tc.repeat(() => {
            tc.draw(gs.booleans());
          });
        });
      },
      { ...deterministic, testCases: 1 },
    );
  });

  it("fails on a failing asynchronous repetition", async () => {
    const { lines, runner } = capturing();
    const error = await rejectionOf(() =>
      runner.testAsync(
        async (tc) => {
          await tc.repeatAsync(async () => {
            await Promise.resolve();
            if (tc.draw(gs.integers({ minValue: 0, maxValue: 10 })) > 5) {
              throw new Error("async repetition");
            }
          });
        },
        { ...deterministic, printBlob: false },
      ),
    );
    expect((error as Error).message).toBe("async repetition");
    expect(lines).toEqual(["// Repetition #1", "  const draw_1 = 6;"]);
  });

  it("does not catch an engine fault in a repetition", () => {
    const { runner } = capturing();
    const error = thrownBy(() =>
      runner.test(
        (tc) => {
          tc.repeat(() => {
            tc.target(1, "t");
          });
        },
        { ...deterministic, testCases: 20 },
      ),
    );
    expect(error).toBeInstanceOf(EngineError);
  });
});

describe("clones and workers", () => {
  it("prints a clone's draws where it was made, attributed to its worker", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      runner.test(
        (tc) => {
          const worker = tc.clone();
          tc.note("main");
          worker.setWorker(1);
          const w = worker.draw(gs.integers({ minValue: 0, maxValue: 10 }), "w");
          if (w > 3) throw new Error("worker drew too much");
        },
        { ...deterministic, printBlob: false },
      ),
    );
    expect((error as Error).message).toBe("worker drew too much");
    expect(lines).toHaveLength(2);
    expect(lines[0]).toMatch(/^\[worker 1 \+\d+\.\d{3}ms\] const w_1 = 4;$/);
    expect(lines[1]).toBe("main");
  });

  it.each([
    ["native", () => capturing().runner],
    ["wasm", () => createRunner(browserRuntime(wasmFixture().engine))],
  ])("refuses a clone used after its test case on the %s engine", (_name, runner) => {
    const clones: TestCase[] = [];
    runner().test(
      (tc) => {
        clones.push(tc.clone());
        tc.draw(gs.integers());
      },
      { ...deterministic, testCases: 3 },
    );
    for (const clone of clones) {
      expect(() => clone.draw(gs.integers())).toThrow(
        new EngineError("Foreign, freed or invalid engine handle"),
      );
      expect(() => clone.setWorker(1)).toThrow(EngineError);
    }
  });

  it.each([-1, 1.5, Number.MAX_SAFE_INTEGER + 1])("rejects worker index %s", (index) => {
    const { runner } = capturing();
    expect(() => runner.test((tc) => tc.setWorker(index), deterministic)).toThrow(RangeError);
  });
});

describe("explicit examples", () => {
  const body = (tc: TestCase) => {
    const xs = tc.draw(gs.arrays(gs.integers()));
    const name = tc.draw(gs.text(), "name");
    tc.note(`checking ${name}`);
    tc.target(xs.length);
    tc.event("explicit?");
    tc.eventValue("length", xs.length);
    tc.clone().setWorker(0);
    if (xs.length > 2) throw new Error(`long ${name}`);
  };

  it("runs positional and named examples before generating, and reports a failing one", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      new runner.Hegel(body)
        .settings(deterministic)
        .example([], "a")
        .namedExample({ name_1: "c", draw_1: [] })
        .example([1, 2, 3], "b")
        .example([1, 2, 3, 4], "never run")
        .runSync(),
    );
    expect((error as Error).message).toBe("long b");
    expect(lines).toEqual(["const draw_1 = [1, 2, 3];", 'const name_1 = "b";', "checking b"]);
  });

  it("reports an explicit failure along with the generated ones", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      new runner.Hegel(body)
        .settings({ ...deterministic, reportMultipleFailures: true, printBlob: false })
        .example([1, 2, 3], "b")
        .runSync(),
    );
    expect(error).toBeInstanceOf(AggregateError);
    expect((error as AggregateError).errors.map((e: Error) => e.message)).toEqual([
      "long b",
      "long ",
    ]);
    expect(lines.slice(3)).toEqual([
      "const draw_1 = [0, 0, 0];",
      'const name_1 = "";',
      "checking ",
    ]);
  });

  it("reports an explicit failure along with a run error", () => {
    const { runner } = capturing();
    const error = thrownBy(() =>
      new runner.Hegel((tc) => {
        if (tc.draw(gs.integers()) === 0.5) throw new Error("explicit");
        tc.reject();
      })
        .settings({ ...deterministic, reportMultipleFailures: true, verbosity: Verbosity.Quiet })
        .example(0.5)
        .runSync(),
    );
    expect(error).toBeInstanceOf(AggregateError);
    const [explicit, health] = (error as AggregateError).errors as Error[];
    expect(explicit.message).toBe("explicit");
    expect(health.message).toMatch(/assume|filter/i);
  });

  it("skips explicit examples without the explicit phase", () => {
    const { runner } = capturing();
    new runner.Hegel((tc) => {
      if (tc.draw(gs.integers()) === 0.5) throw new Error("explicit");
    })
      .settings({ ...deterministic, phases: [Phase.Generate] })
      .example(0.5)
      .runSync();
  });

  it("throws when an example is rejected before any draw", () => {
    const { runner } = capturing();
    expect(() =>
      new runner.Hegel((tc) => {
        tc.reject();
        tc.draw(gs.integers());
      })
        .settings(deterministic)
        .example(1)
        .runSync(),
    ).toThrow(
      /^Explicit test cases must not be rejected, but example\(1\) was rejected by tc\.assume or tc\.reject$/,
    );
  });

  it("throws when an example is rejected after a draw", async () => {
    const { runner } = capturing();
    await expect(
      new runner.Hegel(async (tc) => {
        tc.assume(tc.draw(gs.integers(), "x") > 5);
      })
        .settings(deterministic)
        .namedExample({ x_1: 1 })
        .run(),
    ).rejects.toThrow(
      /^Explicit test cases must not be rejected, but namedExample\(\{ x_1: 1 \}\) was rejected by tc\.assume or tc\.reject$/,
    );
  });

  it("rejects an example with values left over or missing without reporting it", () => {
    const { lines, runner } = capturing();
    expect(() =>
      new runner.Hegel(body)
        .settings(deterministic)
        .namedExample({ draw_1: [], name_1: "", unused: 1 })
        .runSync(),
    ).toThrow(
      'Explicit example namedExample({ draw_1: [], name_1: "", unused: 1 }) was not fully used: named values not drawn: "unused"',
    );
    expect(() =>
      new runner.Hegel(body).settings(deterministic).example([], "", 1, 2).runSync(),
    ).toThrow("was not fully used: 2 positional value(s) not drawn");
    expect(() =>
      new runner.Hegel(body)
        .settings({ ...deterministic, reportMultipleFailures: true })
        .example([])
        .runSync(),
    ).toThrow(
      new ExplicitExampleError(
        'Explicit example example([]) has no value for draw "name_1": pass it by name, or add a positional value',
      ),
    );
    expect(lines).toEqual([]);
  });

  it("refuses a silent draw as a usage error, not a failing example", () => {
    const { lines, runner } = capturing();
    expect(() =>
      new runner.Hegel((tc) => tc.drawSilent(gs.integers()))
        .settings({ ...deterministic, reportMultipleFailures: true })
        .example(1)
        .runSync(),
    ).toThrow(new ExplicitExampleError("drawSilent is not supported in explicit examples"));
    expect(lines).toEqual([]);
  });

  it("runs a repeat's body once in an explicit example", async () => {
    const { lines, runner } = capturing();
    const explicitOnly = { ...deterministic, phases: [Phase.Explicit] };
    const seen: number[] = [];
    const body = (tc: TestCase) => () => {
      const n = tc.draw(gs.integers(), "n");
      seen.push(n);
      if (n === 5) throw new Error("five");
    };
    new runner.Hegel((tc) => tc.repeat(body(tc))).settings(explicitOnly).example(1).runSync();
    await new runner.Hegel(async (tc) => {
      await tc.repeatAsync(body(tc));
    })
      .settings(explicitOnly)
      .example(2)
      .run();
    expect(seen).toEqual([1, 2]);
    const error = await rejectionOf(() =>
      new runner.Hegel(async (tc) => {
        await tc.repeatAsync(body(tc));
      })
        .settings(explicitOnly)
        .example(5)
        .run(),
    );
    expect((error as Error).message).toBe("five");
    expect(lines).toEqual(["// Repetition #1", "  const n_1 = 5;"]);
    expect(() =>
      new runner.Hegel((tc) =>
        tc.repeat(() => {
          tc.reject();
        }),
      )
        .settings(explicitOnly)
        .example()
        .runSync(),
    ).toThrow(/^Explicit test cases must not be rejected, but example\(\) was rejected/);
  });

  it("lays out an explicit example's report as the engine lays out a generated one's", () => {
    const { lines, runner } = capturing();
    const values = Array.from({ length: 30 }, (_, i) => i * 1000);
    const error = thrownBy(() =>
      new runner.Hegel((tc) => {
        tc.draw(gs.arrays(gs.integers()), "xs");
        throw new Error("explicit");
      })
        .settings(deterministic)
        .example(values)
        .runSync(),
    );
    expect((error as Error).message).toBe("explicit");
    expect(lines).toEqual(["const xs_1 = [", ...values.map((value) => `    ${value},`), "];"]);
  });

  it("indents a block's draws and notes in an explicit example's report", () => {
    const { lines, runner } = capturing();
    const error = thrownBy(() =>
      new runner.Hegel((tc) => {
        tc.note("Step 1");
        const step = tc.child(2);
        const n = step.draw(gs.integers(), "n");
        step.note(`saw ${n}\nand more`);
        tc.draw(gs.integers(), "after");
        throw new Error("explicit");
      })
        .settings(deterministic)
        .example(1, 2)
        .runSync(),
    );
    expect((error as Error).message).toBe("explicit");
    expect(lines).toEqual([
      "Step 1",
      "  const n_1 = 1;",
      "  saw 1",
      "  and more",
      "const after_1 = 2;",
    ]);
  });
});

describe("reproduceFailure", () => {
  it("replays the failure a blob encodes, and every blob given", () => {
    const { lines, runner } = capturing();
    const failing = (tc: TestCase) => {
      const n = tc.draw(gs.integers({ minValue: 0, maxValue: 1000 }));
      if (n >= 10) throw new Error(`big ${n}`);
    };
    thrownBy(() => runner.test(failing, { ...deterministic, printBlob: true }));
    const blob = blobOf(lines);
    lines.length = 0;
    const error = thrownBy(() =>
      new runner.Hegel(failing).settings(deterministic).reproduceFailure(blob).runSync(),
    );
    expect((error as Error).message).toBe("big 10");
    expect(lines).toEqual(["const draw_1 = 10;"]);
    const both = thrownBy(() =>
      new runner.Hegel(failing)
        .settings(deterministic)
        .reproduceFailure(blob)
        .reproduceFailure(blob)
        .runSync(),
    );
    expect(both).toBeInstanceOf(AggregateError);
  });

  it("throws DidNotReproduceError for a blob that no longer fails", async () => {
    const { lines, runner } = capturing();
    thrownBy(() =>
      runner.test(
        (tc) => {
          throw new Error(String(tc.draw(gs.booleans())));
        },
        { ...deterministic, printBlob: true },
      ),
    );
    const blob = blobOf(lines);
    const error = await rejectionOf(() =>
      new runner.Hegel(async (tc) => {
        tc.draw(gs.booleans());
      })
        .reproduceFailure(blob)
        .run(),
    );
    expect(error).toBeInstanceOf(DidNotReproduceError);
    expect((error as Error).message).toMatch(/did not reproduce a failure/);
  });

  it("rejects a blob that is not one", () => {
    const { runner } = capturing();
    expect(() => new runner.Hegel(() => {}).reproduceFailure("not a blob").runSync()).toThrow(
      EngineError,
    );
  });
});
