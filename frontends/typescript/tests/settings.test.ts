import { describe, it, expect, vi } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { createRunner } from "../src/runnerCore.js";
import {
  NativeBackend,
  NativeHealthCheck,
  NativeNondeterminismStrictness,
  NativePhase,
  NativeVerbosity,
  type Engine,
  type SettingsHandle,
} from "../src/engine.js";
import { applySettings, resolveSettings } from "../src/settings.js";
import { getLibhegel } from "../src/session.js";
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
    const browser = createRunner(browserRuntime(wasmFixture().engine));
    for (const testCases of [-1, 1.5, NaN, Infinity, 2 ** 64, 5n as unknown as number]) {
      for (const runner of [hegel, browser]) {
        expect(() => runner.test(() => {}, { testCases })).toThrow(
          /^Invalid testCases .*: expected a non-negative safe integer$/,
        );
      }
    }
    expect(() => browser.test(() => {}, { seed: -1 })).toThrow(
      /^Invalid seed -1: expected an integer in \[0, 2\^64\)$/,
    );
    expect(() => hegel.test(() => {}, { verbosity: "loud" as unknown as hegel.Verbosity })).toThrow(
      /Invalid verbosity: loud/,
    );
    expect(() => hegel.test(() => {}, { phases: ["toString" as unknown as hegel.Phase] })).toThrow(
      /Invalid phase/,
    );
  });
});

function readBack(lib: Engine, handle: SettingsHandle) {
  return {
    testCases: lib.getTestCases(handle),
    seed: lib.getSeed(handle),
    verbosity: lib.getVerbosity(handle),
    derandomize: lib.getDerandomize(handle),
    database: lib.getDatabase(handle),
    suppressHealthCheck: lib.getSuppressHealthCheck(handle),
    reportMultipleFailures: lib.getReportMultipleFailures(handle),
    stopOnFirstFailure: lib.getStopOnFirstFailure(handle),
    phases: lib.getPhases(handle),
    printBlob: lib.getPrintBlob(handle),
    showStatistics: lib.getShowStatistics(handle),
    unboundedChoices: lib.getUnboundedChoices(handle),
    nondeterminismStrictness: lib.getNondeterminismStrictness(handle),
    backend: lib.getBackend(handle),
  };
}

function applied(lib: Engine, given: hegel.Settings): ReturnType<typeof readBack> {
  const ctx = lib.newContext();
  try {
    const handle = resolveSettings(lib, ctx, given);
    try {
      applySettings(lib, ctx, handle, given);
      return readBack(lib, handle);
    } finally {
      lib.freeSettings(handle);
    }
  } finally {
    lib.freeContext(ctx);
  }
}

function resolved(lib: Engine, profile?: string): ReturnType<typeof readBack> {
  const ctx = lib.newContext();
  try {
    const handle =
      profile === undefined ? lib.newSettings(ctx) : lib.newSettingsForProfile(ctx, profile);
    try {
      return readBack(lib, handle);
    } finally {
      lib.freeSettings(handle);
    }
  } finally {
    lib.freeContext(ctx);
  }
}

const engines: [string, () => Engine, string | null][] = [
  ["native", () => getLibhegel(), null],
  ["wasm", () => wasmFixture().engine, ""],
];

describe.each(engines)("settings applied to the %s engine", (_name, engine, defaultDatabase) => {
  it("sets only the fields given", () => {
    const lib = engine();
    expect(applied(lib, {})).toEqual(resolved(lib));
    expect(applied(lib, { profile: "ci" })).toEqual(resolved(lib, "ci"));
    expect(applied(lib, { profile: "base", testCases: 7 })).toEqual({
      ...resolved(lib, "base"),
      testCases: 7,
    });
  });

  it("maps every value to the engine's", () => {
    const lib = engine();
    expect(
      applied(lib, {
        profile: "base",
        testCases: 3,
        seed: 2n ** 64n - 1n,
        verbosity: hegel.Verbosity.Debug,
        derandomize: true,
        database: hegel.Database.disabled,
        suppressHealthCheck: [hegel.HealthCheck.TooSlow, hegel.HealthCheck.LargeInitialTestCase],
        reportMultipleFailures: true,
        stopOnFirstFailure: false,
        phases: [hegel.Phase.Target, hegel.Phase.Shrink],
        printBlob: false,
        showStatistics: true,
        unboundedChoices: true,
        nondeterminismStrictness: hegel.NondeterminismStrictness.Error,
        backend: hegel.Backend.Urandom,
      }),
    ).toEqual({
      testCases: 3,
      seed: 2n ** 64n - 1n,
      verbosity: NativeVerbosity.DEBUG,
      derandomize: true,
      database: "",
      suppressHealthCheck: NativeHealthCheck.TOO_SLOW | NativeHealthCheck.LARGE_INITIAL_TEST_CASE,
      reportMultipleFailures: true,
      stopOnFirstFailure: false,
      phases: NativePhase.TARGET | NativePhase.SHRINK,
      printBlob: false,
      showStatistics: true,
      unboundedChoices: true,
      nondeterminismStrictness: NativeNondeterminismStrictness.ERROR,
      backend: NativeBackend.URANDOM,
    });
    expect(applied(lib, { profile: "base", seed: 5, database: hegel.Database.default })).toEqual({
      ...resolved(lib, "base"),
      seed: 5n,
      database: defaultDatabase,
    });
    expect(applied(lib, { profile: "base", seed: 5 }).seed).toBe(5n);
    expect(applied(lib, { seed: null }).seed).toBeNull();
  });
});

it("maps a database path to the native engine's", () => {
  expect(applied(getLibhegel(), { database: hegel.Database.fromPath("/tmp/db") }).database).toBe(
    "/tmp/db",
  );
});

describe("settings applied through a runner", () => {
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
    const { engine } = wasmFixture();
    const freeSettings = vi.spyOn(engine, "freeSettings");
    const freeContext = vi.spyOn(engine, "freeContext");
    expect(() => createRunner(browserRuntime(engine)).registerProfile("base")).toThrow(/base/);
    expect(freeSettings).toHaveBeenCalledTimes(1);
    expect(freeContext).toHaveBeenCalledTimes(1);
  });

  it("clears the default profile with an empty name", () => {
    const { engine } = wasmFixture();
    const setDefaultProfile = vi.spyOn(engine, "setDefaultProfile");
    const runner = createRunner(browserRuntime(engine));
    runner.registerProfile("ts-wasm-tiny", { testCases: 1 });
    runner.setDefaultProfile("ts-wasm-tiny");
    runner.setDefaultProfile(null);
    expect(setDefaultProfile.mock.calls.map((call) => call[1])).toEqual(["ts-wasm-tiny", ""]);
    expect(resolved(engine).testCases).toBe(resolved(engine, "base").testCases);
  });
});
