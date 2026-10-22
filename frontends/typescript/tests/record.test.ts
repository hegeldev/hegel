import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.record()", () => {
  test("exposes a schema via asBasic when all fields are basic", () => {
    const gen = gs.record({
      name: gs.text({ minSize: 1, maxSize: 10 }),
      age: gs.integers({ minValue: 0, maxValue: 120 }),
    });
    expect(gen.asBasic()).not.toBeNull();
  });

  test("asBasic returns null when any field is non-basic", () => {
    const gen = gs.record({
      value: gs.integers({ minValue: 0, maxValue: 100 }).filter(() => true),
    });
    expect(gen.asBasic()).toBeNull();
  });

  test("generates plain objects with correct field types", () =>
    hegel.test(
      (tc) => {
        const gen = gs.record({
          name: gs.text({ minSize: 1, maxSize: 10 }),
          age: gs.integers({ minValue: 0, maxValue: 120 }),
          active: gs.booleans(),
        });
        const obj = tc.draw(gen);
        expect(typeof obj.name).toBe("string");
        expect(typeof obj.age).toBe("number");
        expect(typeof obj.active).toBe("boolean");
      },
      { testCases: 20 },
    ));

  test("works with non-basic field generators (composite path)", () =>
    hegel.test(
      (tc) => {
        const gen = gs.record({
          value: gs.integers({ minValue: 0, maxValue: 100 }).filter((x) => x > 10),
        });
        const obj = tc.draw(gen);
        expect(obj.value).toBeGreaterThan(10);
      },
      { testCases: 20 },
    ));

  test("works with gs.just() for constant fields", () =>
    hegel.test(
      (tc) => {
        const gen = gs.record({
          type: gs.just("user" as const),
          id: gs.integers({ minValue: 1, maxValue: 1000 }),
        });
        const obj = tc.draw(gen);
        expect(obj.type).toBe("user");
        expect(obj.id).toBeGreaterThanOrEqual(1);
      },
      { testCases: 20 },
    ));
});
