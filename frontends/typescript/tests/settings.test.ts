import { describe, it, expect, vi } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { createRunner } from "../src/runnerCore.js";
import { NativeBackend, NativeNondeterminismStrictness, NativePhase } from "../src/engine.js";
import { fakeEngine, fakeRuntime, context, settings } from "./fakeEngine.js";
import { browserRuntime } from "../src/browser/runtime.js";
import { wasmFixture } from "./wasmFixture.js";

function countCases(options?: hegel.Settings): number {
  let count = 0;
  hegel.test((tc) => {
    tc.draw(gs.integers());
    count++;
  }, options);
  return count;
}

describe("settings resolution against the real engine", () => {
  it("keeps the engine's resolved settings for every field left out", () => {
    vi.stubEnv("HEGEL_TEST_CASES", "7");
    try {
      expect(countCases({ database: hegel.Database.disabled })).toBe(7);
      expect(countCases({ database: hegel.Database.disabled, testCases: 3 })).toBe(3);
    } finally {
      vi.unstubAllEnvs();
    }
  });

  it("starts from a named profile", () => {
    expect(countCases({ profile: "base", testCases: 4, database: hegel.Database.disabled })).toBe(
      4,
    );
    expect(() => countCases({ profile: "no-such-profile-ts" })).toThrow(/no-such-profile-ts/);
  });

  it("registers profiles and selects the default one", () => {
    hegel.registerProfile("ts-tiny", { testCases: 2, database: hegel.Database.disabled });
    hegel.registerProfile("ts-defaults");
    expect(countCases({ profile: "ts-tiny" })).toBe(2);
    expect(countCases({ profile: "ts-tiny", testCases: 5 })).toBe(5);
    hegel.setDefaultProfile("ts-tiny");
    try {
      expect(countCases()).toBe(2);
    } finally {
      hegel.setDefaultProfile(null);
    }
    expect(() => hegel.registerProfile("base", {})).toThrow(/base/);
    expect(() => hegel.registerProfile("ts-bad", { profile: "no-such-profile-ts" })).toThrow(
      /no-such-profile-ts/,
    );
    expect(() => hegel.registerProfile("ts-bad", { seed: -1 })).toThrow(RangeError);
    expect(() => hegel.setDefaultProfile("default")).toThrow(/default/);
  });

  it("passes every setting through to the engine", () => {
    hegel.test(
      (tc) => {
        tc.draw(gs.integers());
      },
      {
        testCases: 3,
        seed: 2n ** 64n - 1n,
        derandomize: true,
        database: hegel.Database.disabled,
        verbosity: hegel.Verbosity.Quiet,
        suppressHealthCheck: [],
        reportMultipleFailures: true,
        stopOnFirstFailure: false,
        phases: [hegel.Phase.Explicit, hegel.Phase.Reuse, hegel.Phase.Generate],
        printBlob: false,
        showStatistics: true,
        unboundedChoices: true,
        nondeterminismStrictness: hegel.NondeterminismStrictness.Warn,
        backend: hegel.Backend.Default,
      },
    );
    hegel.test(() => {}, {
      testCases: 1,
      seed: null,
      database: hegel.Database.disabled,
      nondeterminismStrictness: hegel.NondeterminismStrictness.Quiet,
      backend: hegel.Backend.Urandom,
    });
  });

  it("rejects invalid seeds and enum values before running", () => {
    for (const seed of [-1, 1.5, 2n ** 64n, -1n, Number.MAX_SAFE_INTEGER + 2]) {
      expect(() => hegel.test(() => {}, { seed })).toThrow(RangeError);
    }
    expect(() => hegel.test(() => {}, { verbosity: "loud" as unknown as hegel.Verbosity })).toThrow(
      /Invalid verbosity: loud/,
    );
    expect(() => hegel.test(() => {}, { phases: ["toString" as unknown as hegel.Phase] })).toThrow(
      /Invalid phase/,
    );
  });
});

describe("settings applied through the Engine interface", () => {
  it("sets only the fields given", () => {
    const engine = fakeEngine();
    createRunner(fakeRuntime(engine)).test(() => {});
    for (const setter of [
      engine.setTestCases,
      engine.setVerbosity,
      engine.setSeed,
      engine.setDerandomize,
      engine.setDatabase,
      engine.setSuppressHealthCheck,
      engine.setReportMultipleFailures,
      engine.setStopOnFirstFailure,
      engine.setPhases,
      engine.setPrintBlob,
      engine.setShowStatistics,
      engine.setUnboundedChoices,
      engine.setNondeterminismStrictness,
      engine.setBackend,
    ])
      expect(setter).not.toHaveBeenCalled();
    expect(engine.newSettings).toHaveBeenCalledTimes(1);
    expect(engine.newSettingsForProfile).not.toHaveBeenCalled();
  });

  it("maps every value to the engine's", () => {
    const engine = fakeEngine();
    createRunner(fakeRuntime(engine)).test(() => {}, {
      profile: "ci",
      seed: 5,
      database: hegel.Database.fromPath("/tmp/db"),
      suppressHealthCheck: [hegel.HealthCheck.TooSlow, hegel.HealthCheck.LargeInitialTestCase],
      phases: [hegel.Phase.Target, hegel.Phase.Shrink],
      nondeterminismStrictness: hegel.NondeterminismStrictness.Error,
      backend: hegel.Backend.Urandom,
    });
    expect(engine.newSettingsForProfile).toHaveBeenCalledWith(context, "ci");
    expect(engine.setSeed).toHaveBeenCalledWith(settings, 5n);
    expect(engine.setDatabase).toHaveBeenCalledWith(context, settings, "/tmp/db");
    expect(engine.setSuppressHealthCheck).toHaveBeenCalledWith(settings, 2 | 8);
    expect(engine.setPhases).toHaveBeenCalledWith(
      settings,
      NativePhase.TARGET | NativePhase.SHRINK,
    );
    expect(engine.setNondeterminismStrictness).toHaveBeenCalledWith(
      settings,
      NativeNondeterminismStrictness.ERROR,
    );
    expect(engine.setBackend).toHaveBeenCalledWith(settings, NativeBackend.URANDOM);
  });

  it("maps the default database to the engine's NULL", () => {
    const engine = fakeEngine();
    createRunner(fakeRuntime(engine)).test(() => {}, { database: hegel.Database.default });
    expect(engine.setDatabase).toHaveBeenCalledWith(context, settings, null);
  });

  it("always disables the database where the host cannot keep one", () => {
    const { engine } = wasmFixture();
    const setDatabase = vi.spyOn(engine, "setDatabase");
    const runner = createRunner(browserRuntime(engine));
    runner.test(() => {}, { testCases: 1 });
    runner.test(() => {}, { testCases: 1, database: hegel.Database.default });
    expect(setDatabase.mock.calls.map((call) => call[2])).toEqual(["", null, ""]);
    expect(() => runner.test(() => {}, { database: hegel.Database.fromPath("db") })).toThrow(
      /Filesystem/,
    );
  });

  it("releases the context when registering a profile fails", () => {
    const engine = fakeEngine();
    engine.registerProfile.mockImplementation(() => {
      throw new Error("register");
    });
    expect(() => createRunner(fakeRuntime(engine)).registerProfile("p")).toThrow("register");
    expect(engine.freeSettings).toHaveBeenCalledTimes(1);
    expect(engine.freeContext).toHaveBeenCalledTimes(1);
  });

  it("clears the default profile with an empty name", () => {
    const engine = fakeEngine();
    createRunner(fakeRuntime(engine)).setDefaultProfile(null);
    expect(engine.setDefaultProfile).toHaveBeenCalledWith(context, "");
  });
});
