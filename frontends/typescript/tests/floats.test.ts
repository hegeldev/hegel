import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.floats()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.floats().asBasic()).not.toBeNull();
  });

  test("generates numbers in range", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValue: 0, maxValue: 1 }));
        expect(typeof v).toBe("number");
        expect(v).toBeGreaterThanOrEqual(0);
        expect(v).toBeLessThanOrEqual(1);
      },
      { testCases: 20 },
    ));

  test("generates floats without bounds", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats());
        expect(typeof v).toBe("number");
      },
      { testCases: 10 },
    ));

  test("generates floats with only minValue", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValue: 0 }));
        expect(typeof v).toBe("number");
      },
      { testCases: 10 },
    ));

  test("exclusive bounds exclude the bound itself", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValueExclusive: 0, maxValueExclusive: 1 }));
        expect(v).toBeGreaterThan(0);
        expect(v).toBeLessThan(1);
        const above = tc.draw(gs.floats({ minValueExclusive: -1, maxValue: 0 }));
        expect(above).toBeGreaterThan(-1);
        expect(above).toBeLessThanOrEqual(0);
        const below = tc.draw(gs.floats({ minValue: 0, maxValueExclusive: 1 }));
        expect(below).toBeGreaterThanOrEqual(0);
        expect(below).toBeLessThan(1);
      },
      { testCases: 200 },
    ));

  test("both forms of one bound is an error", () => {
    expect(() => gs.floats({ minValue: 0, minValueExclusive: 0 })).toThrow(
      "Cannot give both minValue and minValueExclusive",
    );
    expect(() => gs.floats({ maxValue: 1, maxValueExclusive: 1 })).toThrow(
      "Cannot give both maxValue and maxValueExclusive",
    );
  });

  test("allowSubnormal: false keeps subnormal values out", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ minValue: -1, maxValue: 1, allowSubnormal: false }));
        expect(v === 0 || Math.abs(v) >= 2 ** -1022).toBe(true);
        const f = tc.draw(gs.floats({ width: 32, minValueExclusive: 0, allowSubnormal: false }));
        expect(f).toBeGreaterThanOrEqual(2 ** -126);
        const any = tc.draw(gs.floats({ allowSubnormal: false }));
        expect(any === 0 || Number.isNaN(any) || Math.abs(any) >= 2 ** -1022).toBe(true);
      },
      { testCases: 300 },
    ));

  test("subnormals are allowed by default exactly when the bounds admit any", () =>
    hegel.test(
      (tc) => {
        const tiny = tc.draw(gs.floats({ minValue: 1e-310, maxValue: 1e-310 }));
        expect(tiny).toBe(1e-310);
        const one = tc.draw(gs.floats({ minValue: 1, maxValue: 1 }));
        expect(one).toBe(1);
        expect(tc.draw(gs.floats({ minValue: -1e-310, maxValue: 1 }))).toBeGreaterThanOrEqual(
          -1e-310,
        );
        expect(tc.draw(gs.floats({ minValue: 1 }))).toBeGreaterThanOrEqual(1);
        expect(tc.draw(gs.floats({ maxValue: -1 }))).toBeLessThanOrEqual(-1);
      },
      { testCases: 20 },
    ));

  test("allowSubnormal is checked against the bounds", () => {
    expect(() => gs.floats({ minValue: 1, allowSubnormal: true })).toThrow(
      "allowSubnormal is true, but minValue excludes all values below the smallest positive normal",
    );
    expect(() => gs.floats({ minValueExclusive: 1, allowSubnormal: true })).toThrow(
      "allowSubnormal is true, but minValueExclusive excludes all values below",
    );
    expect(() => gs.floats({ maxValue: -1, allowSubnormal: true })).toThrow(
      "allowSubnormal is true, but maxValue excludes all values above the smallest negative normal",
    );
    expect(() => gs.floats({ maxValueExclusive: -1, allowSubnormal: true })).toThrow(
      "allowSubnormal is true, but maxValueExclusive excludes all values above",
    );
    expect(() => gs.floats({ minValue: 1e-310, maxValue: 2e-310, allowSubnormal: false })).toThrow(
      "allowSubnormal is false, but that leaves no 64-bit floating-point values in [1e-310, 2e-310]",
    );
    expect(() =>
      gs.floats({ minValue: -1e-310, maxValue: 0, allowSubnormal: false }),
    ).not.toThrow();
    expect(() => gs.floats({ minValue: 1e-310, maxValue: 1, allowSubnormal: false })).not.toThrow();
  });
});

describe("gs.floats({ width: 32 })", () => {
  test("generates values exactly representable as 32-bit floats", () =>
    hegel.test(
      (tc) => {
        const v = tc.draw(gs.floats({ width: 32 }));
        expect(Object.is(Math.fround(v), v)).toBe(true);
        const bounded = tc.draw(gs.floats({ width: 32, minValue: 0.5, maxValue: 1.5 }));
        expect(Math.fround(bounded)).toBe(bounded);
        expect(bounded).toBeGreaterThanOrEqual(0.5);
        expect(bounded).toBeLessThanOrEqual(1.5);
      },
      { testCases: 100 },
    ));

  test.each([
    [
      { width: 32, minValue: 0.1 },
      "min_value (0.1) cannot be exactly represented as a float of width 32",
    ],
    [
      { width: 32, maxValue: 1.1 },
      "max_value (1.1) cannot be exactly represented as a float of width 32",
    ],
    [{ width: 16 as 32 }, "unsupported float width: 16"],
  ] as const)("the engine rejects %o", (options, message) => {
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(gs.floats(options));
        },
        { testCases: 1, database: hegel.Database.disabled },
      ),
    ).toThrow(message);
  });

  test("defaults to width 64", () => {
    expect(gs.floats({ minValue: 0.1 }).asBasic().schema["width"]).toBe(64);
  });
});
