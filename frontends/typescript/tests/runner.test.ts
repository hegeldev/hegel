/**
 * Tests for the test runner, error classes, and test lifecycle.
 */

import { describe, test, it, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { AssumeError, StopTestError, TestCase } from "../src/testCase.js";

describe("StopTestError", () => {
  it("has correct name and message", () => {
    const e = new StopTestError();
    expect(e.name).toBe("StopTestError");
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("Hegel ran out of data (StopTest)");
  });
});

describe("AssumeError", () => {
  it("has correct name and message", () => {
    const e = new AssumeError();
    expect(e.name).toBe("AssumeError");
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("Assumption rejected");
  });
});

// ---------------------------------------------------------------------------
// assume() behavior
// ---------------------------------------------------------------------------

describe("assume", () => {
  test("assume(true) is a no-op", () =>
    hegel.test((tc) => {
      tc.assume(true);
    }));

  test("assume(false) rejects the test case", () =>
    hegel.test((tc) => {
      const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
      tc.assume(x > 10);
      expect(x).toBeGreaterThan(10);
    }));
});

// ---------------------------------------------------------------------------
// note() behavior
// ---------------------------------------------------------------------------

describe("note", () => {
  test("note does not throw during exploration", () =>
    hegel.test((tc) => {
      tc.draw(gs.booleans());
      tc.note("should not throw");
    }));
});

// ---------------------------------------------------------------------------
// Failing test detection
// ---------------------------------------------------------------------------

describe("failing test detection", () => {
  test("hegel.test() detects a property failure", () => {
    expect(() =>
      hegel.test((tc) => {
        const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        if (x > 0) {
          throw new Error("Found positive number");
        }
      }),
    ).toThrow("Found positive number");
  });

  test("non-Error thrown value is reported", () => {
    expect(() =>
      hegel.test((tc) => {
        tc.draw(gs.booleans());
        throw new Error("custom failure");
      }),
    ).toThrow("custom failure");
  });
});

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

describe("settings", () => {
  test("hegel.test() with settings override", () =>
    hegel.test(
      (tc) => {
        const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        expect(x).toBeGreaterThanOrEqual(0);
      },
      { testCases: 10 },
    ));
});

// ---------------------------------------------------------------------------
// Async test bodies
// ---------------------------------------------------------------------------

describe("async test bodies", () => {
  test("hegel.testAsync() awaits an async test body", () =>
    hegel.testAsync(
      async (tc) => {
        const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
        // Yield to the event loop, then resume and assert.
        await Promise.resolve();
        expect(x).toBeGreaterThanOrEqual(0);
      },
      { testCases: 10 },
    ));

  test("async test body throwing fails the run", async () => {
    await expect(
      hegel.testAsync(
        async (tc) => {
          const x = tc.draw(gs.integers({ minValue: 0, maxValue: 100 }));
          await Promise.resolve();
          if (x > 0) {
            throw new Error("async failure");
          }
        },
        { testCases: 50 },
      ),
    ).rejects.toThrow("async failure");
  });

  test("hegel.testAsync() awaits async bodies before resolving", async () => {
    let observed = false;
    await hegel.testAsync(
      async (tc) => {
        tc.draw(gs.booleans());
        await Promise.resolve();
        observed = true;
      },
      { testCases: 3 },
    );
    expect(observed).toBe(true);
  });

  test("hegel.test() rejects async test bodies up front", () => {
    expect(() =>
      hegel.test((async (tc) => {
        tc.draw(gs.booleans());
      }) as (tc: TestCase) => void),
    ).toThrow(/hegel\.testAsync/);
  });

  test("dropping the testAsync() promise does not throw synchronously", async () => {
    let caughtSync: Error | null = null;
    let promise!: Promise<void>;
    try {
      promise = hegel.testAsync(async (tc) => {
        tc.draw(gs.integers());
        await Promise.resolve();
        throw new Error("boom");
      });
    } catch (e) {
      caughtSync = e as Error;
    }
    expect(caughtSync).toBeNull();
    await expect(promise).rejects.toThrow(/boom/);
  });
});

test("rethrows a thrown non-Error value as it is", () => {
  let thrown: unknown;
  try {
    hegel.test((tc) => {
      const n = tc.draw(gs.integers());
      if (n >= 12345) {
        throw n;
      }
    });
  } catch (error) {
    thrown = error;
  }
  expect(thrown).toBe(12345);
});
