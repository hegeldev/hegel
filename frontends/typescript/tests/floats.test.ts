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
