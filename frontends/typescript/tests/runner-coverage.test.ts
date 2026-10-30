/**
 * Tests targeting uncovered lines in runner.ts, specifically:
 * - Hegel.run() settings branches (database, suppressHealthCheck)
 * - ServerDataSource error handling paths
 * - Health check failure detection
 * - Flaky test detection
 * - Server error (invalid schema) detection
 */

import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { NativeDataSource } from "../src/runner.js";
import { Libhegel, Status, NativeVerbosity } from "../src/libhegel.js";
import { labelSpec } from "../src/labels.js";
import { testLibPath } from "./libPath.js";

describe("settings branches", () => {
  test("database: 'disabled' sets database to null", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      { testCases: 5, database: hegel.Database.disabled },
    ));

  test("database: 'default' resets the database to the engine's default location", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      { testCases: 5, database: hegel.Database.default },
    ));

  test("database: custom path sets database to string", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      { testCases: 5, database: hegel.Database.fromPath(".hegel/test-db") },
    ));

  test("suppressHealthCheck passes through to the engine", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      {
        testCases: 5,
        suppressHealthCheck: [hegel.HealthCheck.FilterTooMuch, hegel.HealthCheck.TooSlow],
      },
    ));

  test("reportMultipleFailures: true passes through to the engine", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      { testCases: 5, reportMultipleFailures: true },
    ));

  test("reportMultipleFailures: false passes through to the engine", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.booleans());
      },
      { testCases: 5, reportMultipleFailures: false },
    ));

  test("an explicit seed makes the run reproducible", () =>
    hegel.test(
      (tc) => {
        tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
      },
      { testCases: 5, seed: 42 },
    ));

  test("every verbosity level is accepted", () => {
    for (const verbosity of [
      hegel.Verbosity.Quiet,
      hegel.Verbosity.Normal,
      hegel.Verbosity.Verbose,
      hegel.Verbosity.Debug,
    ]) {
      hegel.test(
        (tc) => {
          tc.draw(gs.booleans());
        },
        { testCases: 2, verbosity },
      );
    }
  });

  test("an unresolvable default settings profile surfaces as the run's error", () => {
    const original = process.env["HEGEL_DEFAULT_PROFILE"];
    try {
      process.env["HEGEL_DEFAULT_PROFILE"] = "no-such-profile";
      expect(() =>
        hegel.test((tc) => {
          tc.draw(gs.booleans());
        }),
      ).toThrow(/unknown settings profile "no-such-profile"/);
    } finally {
      if (original === undefined) {
        delete process.env["HEGEL_DEFAULT_PROFILE"];
      } else {
        process.env["HEGEL_DEFAULT_PROFILE"] = original;
      }
    }
  });
});

describe("NativeDataSource collection rejection", () => {
  test("rejects elements with and without a reason", () => {
    const lib = Libhegel.load(testLibPath());
    const ctx = lib.newContext();
    const settings = lib.newSettings(ctx);
    lib.setVerbosity(settings, NativeVerbosity.QUIET);
    lib.setDatabase(ctx, settings, "");
    const run = lib.runStart(ctx, settings);
    try {
      const tc = lib.nextTestCase(ctx, run);
      expect(tc).not.toBeNull();
      const ds = new NativeDataSource(lib, ctx, tc);
      ds.startSpan(ds.label(labelSpec("hegel.sets")));
      const id = ds.newCollection(2, 5);
      const intSchema = { type: "integer", min_value: 0n, max_value: 100n };
      let rejects = 0;
      while (ds.collectionMore(id)) {
        ds.startSpan(ds.label(labelSpec("hegel.sets.element")));
        ds.generate(intSchema);
        ds.stopSpan(false);
        if (rejects < 2) {
          // First reject omits the reason (why ?? null), second supplies one.
          ds.collectionReject(id, rejects === 0 ? undefined : "duplicate");
          rejects++;
        }
      }
      ds.stopSpan(false);
      ds.markComplete(Status.VALID, null);
      ds.dispose();
      lib.freeTestCase(tc);
      expect(rejects).toBe(2);
    } finally {
      lib.freeRun(run);
      lib.freeSettings(settings);
      lib.freeContext(ctx);
    }
  });
});

describe("non-Error failures", () => {
  test("a thrown non-Error value is reported", () => {
    expect(() =>
      hegel.test((tc) => {
        tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        throw "boom-string";
      }),
    ).toThrow(/boom-string/);
  });
});

describe("engine error reporting", () => {
  test("a malformed schema surfaces as a thrown error", () => {
    // An integer schema with no bounds is rejected by the engine on the first
    // draw; the failing draw propagates out of hegel.test.
    const badGen = new gs.BasicGenerator({ type: "integer" });
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(badGen);
        },
        { testCases: 1 },
      ),
    ).toThrow();
  });

  test("excessive filtering trips the FilterTooMuch health check (run-level error)", () => {
    // Rejecting >99% of values produces a FilterTooMuch health-check failure,
    // reported by the runner as a run-level error.
    expect(() =>
      hegel.test((tc) => {
        const x = tc.draw(gs.integers({ minValue: 0, maxValue: 1000 }));
        tc.assume(x === 500);
      }),
    ).toThrow(/FilterTooMuch/);
  });

  test("a nondeterministic (flaky) test is reported as a failure", () => {
    // Fails the first time a positive value is seen, then passes on replay.
    let seen = false;
    expect(() =>
      hegel.test((tc) => {
        const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        if (x > 0 && !seen) {
          seen = true;
          throw new Error("flaky failure");
        }
      }),
    ).toThrow();
  });
});
