import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("gs.sampledFrom()", () => {
  test("exposes a schema via asBasic", () => {
    expect(gs.sampledFrom([1, 2, 3]).asBasic()).not.toBeNull();
  });

  test("throws on empty list", () => {
    expect(() => gs.sampledFrom([])).toThrow("sampledFrom requires at least one element");
  });

  test("returns a value from the list", () =>
    hegel.test(
      (tc) => {
        const items = [10, 20, 30];
        const v = tc.draw(gs.sampledFrom(items));
        expect(items).toContain(v);
      },
      { testCases: 50 },
    ));

  test("returns non-primitive objects from the list", () =>
    hegel.test(
      (tc) => {
        class Custom {
          constructor(public readonly x: number) {}
        }
        const items = [new Custom(1), new Custom(2), new Custom(3)];
        const v = tc.draw(gs.sampledFrom(items));
        expect(v).toBeInstanceOf(Custom);
        expect(items).toContain(v);
      },
      { testCases: 10 },
    ));

  test("covers all values across many runs", async () => {
    const items = ["red", "green", "blue"];
    const seen = new Set<string>();
    await hegel.test(
      (tc) => {
        const v = tc.draw(gs.sampledFrom(items));
        seen.add(v);
      },
      { testCases: 100 },
    );
    for (const item of items) {
      expect(seen.has(item)).toBe(true);
    }
  });
});
