/** The deprecated ISO 8601 string generators over the temporal ones. */

import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";

function minimal<T>(generator: gs.Generator<T>, failing: (value: T) => boolean): T {
  let final!: T;
  expect(() =>
    hegel.test(
      (tc) => {
        final = tc.draw(generator);
        if (failing(final)) throw new Error("falsified");
      },
      { seed: 7, database: hegel.Database.disabled },
    ),
  ).toThrow("falsified");
  return final;
}

function check<T>(generator: gs.Generator<T>, body: (value: T) => void): void {
  hegel.test((tc) => body(tc.draw(generator)), {
    testCases: 100,
    database: hegel.Database.disabled,
  });
}

const TIME = String.raw`\d{2}:\d{2}:\d{2}(\.\d{6}|\.\d{9})?`;

describe("the deprecated string generators", () => {
  test("dates() generates YYYY-MM-DD within years 1 to 9999", () => {
    check(gs.dates(), (value) => expect(value).toMatch(/^\d{4}-\d{2}-\d{2}$/));
    expect(minimal(gs.dates(), () => true)).toBe("2000-01-01");
    expect(minimal(gs.dates(), (value) => value < "0001-01-02")).toBe("0001-01-01");
    expect(minimal(gs.dates(), (value) => value > "9999-12-30")).toBe("9999-12-31");
  });

  test("times() writes whole seconds, microseconds or nanoseconds", () => {
    check(gs.times(), (value) => expect(value).toMatch(new RegExp(`^${TIME}$`)));
    expect(minimal(gs.times(), () => true)).toBe("00:00:00");
    expect(minimal(gs.times(), (value) => value.length === 15)).toBe("00:00:00.000001");
    expect(minimal(gs.times(), (value) => value.length === 18)).toBe("00:00:00.000000001");
  });

  test("datetimes() joins a date and a time with T", () => {
    check(gs.datetimes(), (value) =>
      expect(value).toMatch(new RegExp(`^\\d{4}-\\d{2}-\\d{2}T${TIME}$`)),
    );
    expect(minimal(gs.datetimes(), () => true)).toBe("2000-01-01T00:00:00");
    expect(minimal(gs.datetimes(), (value) => value < "0001-01-02")).toBe("0001-01-01T00:00:00");
  });
});
