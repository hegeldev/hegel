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

  test("rejects bounds a 32-bit float cannot hold, and other widths", () => {
    expect(() => gs.floats({ width: 32, minValue: 0.1 })).toThrow(
      "minValue 0.1 is not exactly representable as a 32-bit float",
    );
    expect(() => gs.floats({ width: 32, maxValue: 1e300 })).toThrow(
      "maxValue 1e+300 is not exactly representable as a 32-bit float",
    );
    expect(() => gs.floats({ width: 16 as 32 })).toThrow("width must be 32 or 64, got 16");
    expect(gs.floats({ minValue: 0.1 }).asBasic().schema["width"]).toBe(64);
  });
});
