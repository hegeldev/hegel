import { describe, test, expect, expectTypeOf } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.tuples()", () => {
  test("generates 2-tuples with correct types", () =>
    hegel.test(
      (tc) => {
        const [n, b] = tc.draw(
          gs.tuples(gs.integers({ minValue: 0, maxValue: 10 }), gs.booleans()),
        );
        expect(typeof n).toBe("number");
        expect(n).toBeGreaterThanOrEqual(0);
        expect(n).toBeLessThanOrEqual(10);
        expect(typeof b).toBe("boolean");
      },
      { testCases: 50 },
    ));

  test("all basic with transforms: transforms applied per-position", () =>
    hegel.test(
      (tc) => {
        const g1 = gs.integers({ minValue: 0, maxValue: 10 }).map((x) => x * 2);
        const g2 = gs.integers({ minValue: 0, maxValue: 5 });
        const [a, b] = tc.draw(gs.tuples(g1, g2));
        expect(a % 2).toBe(0);
        expect(a).toBeGreaterThanOrEqual(0);
        expect(a).toBeLessThanOrEqual(20);
        expect(b).toBeGreaterThanOrEqual(0);
        expect(b).toBeLessThanOrEqual(5);
      },
      { testCases: 30 },
    ));

  test("non-basic: filtered elements use composite tuple path", () =>
    hegel.test(
      (tc) => {
        const filtered = gs.integers({ minValue: 0, maxValue: 10 }).filter((x) => x > 5);
        const [n, b] = tc.draw(gs.tuples(filtered, gs.booleans()));
        expect(n).toBeGreaterThan(5);
        expect(n).toBeLessThanOrEqual(10);
        expect(typeof b).toBe("boolean");
      },
      { testCases: 50 },
    ));
});

describe("gs.tuples() 3-tuples", () => {
  test("generates 3-tuples with correct types", () =>
    hegel.test(
      (tc) => {
        const [s, n, f] = tc.draw(
          gs.tuples(
            gs.text({ maxSize: 5 }),
            gs.integers({ minValue: 0, maxValue: 5 }),
            gs.floats({ minValue: 0, maxValue: 1 }),
          ),
        );
        expect(typeof s).toBe("string");
        expect(typeof n).toBe("number");
        expect(Number.isInteger(n)).toBe(true);
        expect(n).toBeGreaterThanOrEqual(0);
        expect(n).toBeLessThanOrEqual(5);
        expect(typeof f).toBe("number");
        expect(f).toBeGreaterThanOrEqual(0);
        expect(f).toBeLessThanOrEqual(1);
      },
      { testCases: 50 },
    ));
});

describe("gs.tuples() 4-tuples", () => {
  test("generates 4-tuples", () =>
    hegel.test(
      (tc) => {
        const [n, b, s, f] = tc.draw(
          gs.tuples(
            gs.integers({ minValue: 0, maxValue: 10 }),
            gs.booleans(),
            gs.text({ maxSize: 5 }),
            gs.floats({ minValue: 0, maxValue: 1 }),
          ),
        );
        expect(typeof n).toBe("number");
        expect(typeof b).toBe("boolean");
        expect(typeof s).toBe("string");
        expect(typeof f).toBe("number");
      },
      { testCases: 30 },
    ));
});

describe("gs.tuples() inferred types", () => {
  test("empty tuple", () => {
    const g = gs.tuples();
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[]>>();
  });

  test("1-tuple", () => {
    const g = gs.tuples(gs.integers());
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[number]>>();
  });

  test("2-tuple with mixed element types", () => {
    const g = gs.tuples(gs.integers(), gs.booleans());
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[number, boolean]>>();
  });

  test("3-tuple preserves per-position types", () => {
    const g = gs.tuples(gs.text(), gs.integers(), gs.floats());
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[string, number, number]>>();
    // Not gs.Generator<(string | number)[]>
    expectTypeOf(g).not.toEqualTypeOf<gs.Generator<(string | number)[]>>();
  });

  test("tuple type survives map()", () => {
    const g = gs.tuples(gs.integers(), gs.booleans()).map(([n, b]) => ({ n, b }));
    expectTypeOf(g).toEqualTypeOf<gs.Generator<{ n: number; b: boolean }>>();
  });

  test("nested tuples infer nested tuple types", () => {
    const g = gs.tuples(gs.tuples(gs.integers(), gs.booleans()), gs.text());
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[[number, boolean], string]>>();
  });

  test("map callback parameter is a tuple, not an array", () => {
    gs.tuples(gs.integers(), gs.booleans()).map((pair) => {
      expectTypeOf(pair).toEqualTypeOf<[number, boolean]>();
      return pair;
    });
  });

  test("high-arity tuple (5 elements)", () => {
    const g = gs.tuples(gs.integers(), gs.booleans(), gs.text(), gs.floats(), gs.integers());
    expectTypeOf(g).toEqualTypeOf<gs.Generator<[number, boolean, string, number, number]>>();
  });
});
