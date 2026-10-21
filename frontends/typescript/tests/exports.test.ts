import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

describe("public export shape", () => {
  test("hegel.generators is the same namespace as @hegeldev/hegel/generators", () => {
    // The root-namespace re-export and the subpath export must resolve to
    // the same module so users can mix and match without surprise.
    expect(hegel.generators).toBeDefined();
    for (const key of Object.keys(gs)) {
      expect(hegel.generators[key as keyof typeof hegel.generators]).toBe(
        gs[key as keyof typeof gs],
      );
    }
  });

  test("a property test runs via hegel.generators.* without the subpath import", () =>
    // This is the path the namespace re-export unlocks: users only need to
    // import `@hegeldev/hegel` and can reach generators through `hegel.generators`.
    hegel.test(
      (tc) => {
        const n = tc.draw(hegel.generators.integers({ minValue: 0, maxValue: 100 }));
        expect(n).toBeGreaterThanOrEqual(0);
        expect(n).toBeLessThanOrEqual(100);
      },
      { testCases: 20 },
    ));
});
