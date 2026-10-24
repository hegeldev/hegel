import { describe, it, expect, vi } from "vitest";
import * as hegel from "@hegeldev/hegel";
import { createRunner } from "../src/runnerCore.js";
import { callSite, parseFrame, projectPath, runnerTestName } from "../src/callSite.js";
import { fakeEngine, fakeRuntime, context, settings } from "./fakeEngine.js";

const THIS_FILE = new URL(import.meta.url).pathname;

const MATCHERS = Symbol.for("$$jest-matchers-object");

function withMatcherState<T>(matchers: unknown, body: () => T): T {
  const original = Object.getOwnPropertyDescriptor(globalThis, MATCHERS)!;
  Object.defineProperty(globalThis, MATCHERS, { configurable: true, value: matchers });
  try {
    return body();
  } finally {
    Object.defineProperty(globalThis, MATCHERS, original);
  }
}

function withoutCaptureStackTrace(body: () => void): void {
  const original = Error.captureStackTrace;
  (Error as { captureStackTrace?: unknown }).captureStackTrace = undefined;
  try {
    body();
  } finally {
    Error.captureStackTrace = original;
  }
}

describe("stack frame parsing", () => {
  it.each([
    [
      "    at property (file:///a/b/test.ts:4:5)",
      { file: "/a/b/test.ts", line: 4, name: "property" },
    ],
    ["    at async run (/a/test.ts:7:1)", { file: "/a/test.ts", line: 7, name: "run" }],
    ["    at file:///a/test.ts:3:9", { file: "/a/test.ts", line: 3, name: null }],
    [
      "property@https://host/test.ts:4:5",
      { file: "https://host/test.ts", line: 4, name: "property" },
    ],
    ["@https://host/test.ts:8:9", { file: "https://host/test.ts", line: 8, name: null }],
    ["Error: not a frame", null],
  ])("parses %s", (line, frame) => {
    expect(parseFrame(line)).toEqual(frame);
  });

  it("finds no call site without Error.captureStackTrace", () => {
    withoutCaptureStackTrace(() => expect(callSite(callSite)).toBe(null));
  });

  it("finds no call site in a stack it cannot parse", () => {
    const original = Error.captureStackTrace;
    Error.captureStackTrace = (target: object) => {
      (target as { stack?: string }).stack = "Error\n    garbage";
    };
    try {
      expect(callSite(callSite)).toBe(null);
    } finally {
      Error.captureStackTrace = original;
    }
  });

  it("finds no call site in a holder left without a stack", () => {
    const original = Error.captureStackTrace;
    Error.captureStackTrace = () => {};
    try {
      expect(callSite(callSite)).toBe(null);
    } finally {
      Error.captureStackTrace = original;
    }
  });
});

describe("runner test names", () => {
  it.each([
    ["a", { state: { currentTestName: "suite > case" } }, "suite > case"],
    ["no", undefined, null],
    ["a stateless", {}, null],
    ["an empty", { state: { currentTestName: "" } }, null],
    ["a non-string", { state: { currentTestName: 7 } }, null],
  ])("reads %s test name", (_kind, matchers, name) => {
    expect(withMatcherState(matchers, runnerTestName)).toBe(name);
  });
});

describe("project paths", () => {
  it.each([
    ["/proj/tests/a.test.ts", "/proj", "tests/a.test.ts"],
    ["/proj/tests/a.test.ts", "/proj/", "tests/a.test.ts"],
    ["/project/a.test.ts", "/proj", "/project/a.test.ts"],
    ["/proj/a.test.ts", "", "/proj/a.test.ts"],
    ["/C:/proj/tests/a.test.ts", "C:\\proj", "tests/a.test.ts"],
    ["C:\\proj\\tests\\a.test.ts", "C:\\proj", "tests/a.test.ts"],
    ["http://localhost:5173/src/a.test.ts?t=1", "", "/src/a.test.ts"],
  ])("takes %s relative to %s", (file, root, path) => {
    expect(projectPath(file, root)).toBe(path);
  });
});

describe("test identity", () => {
  it("derives the location and database key from where hegel.test is called", () => {
    const engine = fakeEngine();
    const runner = createRunner(fakeRuntime(engine));
    function namedProperty(): void {}
    const line = Number(new Error().stack!.split("\n")[1].match(/:(\d+):\d+\)?$/)![1]) + 1;
    runner.test(namedProperty);
    expect(engine.setTestLocation).toHaveBeenCalledWith(context, settings, {
      file: THIS_FILE,
      beginLine: line,
      className: "identity.test",
      function: "namedProperty",
    });
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(
      context,
      settings,
      `${THIS_FILE}::namedProperty`,
    );
  });

  it("names an anonymous test after the runner's running test", async () => {
    const engine = fakeEngine();
    const runner = createRunner(fakeRuntime(engine));
    await runner.testAsync(() => {});
    const name = "test identity > names an anonymous test after the runner's running test";
    expect(engine.setTestLocation.mock.calls[0][2].function).toBe(name);
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(context, settings, `${THIS_FILE}::${name}`);
  });

  it("names an anonymous test by its line outside a known test runner", () => {
    const engine = fakeEngine();
    const runner = createRunner(fakeRuntime(engine));
    withMatcherState(undefined, () => runner.test(() => {}));
    const location = engine.setTestLocation.mock.calls[0][2];
    expect(location.function).toBe(`<anonymous at line ${location.beginLine}>`);
    expect(location.file).toBe(THIS_FILE);
  });

  it("keys a test by its path relative to the project root", () => {
    const engine = fakeEngine();
    const runtime = {
      ...fakeRuntime(engine),
      projectRoot: THIS_FILE.replace(/\/tests\/[^/]*$/, ""),
    };
    createRunner(runtime).test(function relative() {});
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(
      context,
      settings,
      "tests/identity.test.ts::relative",
    );
  });

  it("derives the call site of a Hegel built directly", () => {
    const engine = fakeEngine();
    const runner = createRunner(fakeRuntime(engine));
    new runner.Hegel(function direct() {}).runSync();
    expect(engine.setTestLocation.mock.calls[0][2].file).toBe(THIS_FILE);
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(context, settings, `${THIS_FILE}::direct`);
  });

  it("uses an explicit database key", () => {
    const engine = fakeEngine();
    new (createRunner(fakeRuntime(engine)).Hegel)(() => {}).databaseKey("my key").runSync();
    expect(engine.setDatabaseKey).toHaveBeenCalledWith(context, settings, "my key");
  });

  it("falls back to the function's name without a call site, and to no key without one", () => {
    withoutCaptureStackTrace(() => {
      const engine = fakeEngine();
      const runner = createRunner(fakeRuntime(engine));
      runner.test(function named() {});
      runner.test(() => {});
      expect(engine.setTestLocation).not.toHaveBeenCalled();
      expect(engine.setDatabaseKey).toHaveBeenCalledExactlyOnceWith(context, settings, "named");
    });
  });

  it("runs a Hegel built from the package entry point on the installed runtime", () => {
    let count = 0;
    new hegel.Hegel((tc) => {
      tc.draw(hegel.generators.integers());
      count++;
    })
      .settings({ testCases: 3 })
      .settings({ database: hegel.Database.disabled })
      .runSync();
    expect(count).toBe(3);
  });

  it("refuses to build a Hegel before any runtime is installed", async () => {
    vi.resetModules();
    const { Hegel } = await import("../src/runnerCore.js");
    expect(() => new Hegel(() => {})).toThrow(/no runtime/);
  });
});
