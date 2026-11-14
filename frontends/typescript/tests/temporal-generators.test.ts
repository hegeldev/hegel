/**
 * The date and time generators against the real engine: argument validation
 * when a generator is built (the generators' own and the engine's), the
 * values they draw, their shrink targets and how a failing test's report
 * prints them.
 */

import { describe, test, expect, vi } from "vitest";
import { readFileSync } from "node:fs";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import {
  PlainDate,
  PlainTime,
  PlainDateTime,
  Instant,
  ZonedDateTime,
  Duration,
} from "../src/temporal.js";
import { NativePrecision, NativeSpanSign, NativeTimeZoneTag } from "../src/engine.js";
import { getLibhegel } from "../src/session.js";
import { TIMEZONE_NAMES } from "../src/timezone-names.js";
import { hostKnowsZone } from "../src/hostZones.js";

const { ZoneSource } = gs.temporal;

/** Run `body` as a property and return the shrunk failing draw of `generator`. */
function minimal<T>(generator: gs.PrintableGenerator<T>, failing: (value: T) => boolean): T {
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

function check<T>(
  generator: gs.PrintableGenerator<T>,
  body: (value: T) => void,
  testCases = 60,
): void {
  hegel.test((tc) => body(tc.draw(generator)), { testCases, database: hegel.Database.disabled });
}

/** Draw from `generator`, for the error a draw raises. */
function draw(generator: gs.PrintableGenerator<unknown>): () => void {
  return () =>
    hegel.test((tc) => void tc.draw(generator), {
      testCases: 5,
      database: hegel.Database.disabled,
    });
}

/** The lines a failing test of `generator` reports. */
function report(generator: gs.PrintableGenerator<unknown>): string {
  const lines: string[] = [];
  const output = vi.spyOn(console, "error").mockImplementation((line: string) => {
    lines.push(line);
  });
  try {
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(generator);
          throw new Error("falsified");
        },
        { seed: 7, database: hegel.Database.disabled },
      ),
    ).toThrow("falsified");
  } finally {
    output.mockRestore();
  }
  return lines.join("\n");
}

describe("gs.temporal.dates()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.dates({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("dates(): cannot have maxValue < minValue");
    expect(() => gs.temporal.dates({ weekdays: [0] })).toThrow(
      "weekdays must be integers between 1 and 7",
    );
    expect(() => gs.temporal.dates({ months: [13] })).toThrow(
      "months must be integers between 1 and 12",
    );
    expect(() => gs.temporal.dates({ minValue: { year: 2024, month: 2, day: 30 } })).toThrow(
      "day must be an integer between 1 and 29",
    );
  });

  test("says which bound is the default when one given alone lies beyond it", () => {
    expect(() => gs.temporal.dates({ minValue: { year: 300000, month: 1, day: 1 } })).toThrow(
      "dates(): minValue +300000-01-01 is after the default maxValue, +275760-09-13: pass maxValue as well",
    );
    expect(() => gs.temporal.dates({ maxValue: { year: -300000, month: 1, day: 1 } })).toThrow(
      "dates(): maxValue -300000-01-01 is before the default minValue, -271821-04-19: pass minValue as well",
    );
    const far = { year: 300000, month: 1, day: 1 };
    check(gs.temporal.dates({ minValue: far, maxValue: far }), (date) =>
      expect(date.toString()).toBe("+300000-01-01"),
    );
  });

  test("has the engine check the masks when it is built", () => {
    expect(() => gs.temporal.dates({ weekdays: [] })).toThrow(
      /hegel_date_options_set_weekdays failed.*at least one weekday/,
    );
    expect(() => gs.temporal.dates({ months: [] })).toThrow(
      /hegel_date_options_set_months failed.*at least one month/,
    );
  });

  test("has the engine report bounds no allowed date lies in when it is built", () => {
    const tuesday = { year: 2024, month: 3, day: 12 };
    expect(() =>
      gs.temporal.dates({ minValue: tuesday, maxValue: tuesday, weekdays: [1] }),
    ).toThrow(/hegel_validate_date failed.*falls on an allowed weekday and month/);
    expect(() =>
      gs.temporal.dates({
        minValue: tuesday,
        maxValue: { year: 2024, month: 3, day: 31 },
        weekdays: [2],
        months: [4],
      }),
    ).toThrow("falls on an allowed weekday and month");
  });

  test("draws within the bounds on the allowed weekdays and months", () => {
    const min = new PlainDate(2020, 1, 1);
    const max = new PlainDate(2030, 12, 31);
    check(
      gs.temporal.dates({ minValue: min, maxValue: max, weekdays: [6, 7], months: [2, 12] }),
      (date) => {
        expect(date).toBeInstanceOf(PlainDate);
        expect(PlainDate.compare(min, date)).toBeLessThanOrEqual(0);
        expect(PlainDate.compare(date, max)).toBeLessThanOrEqual(0);
        expect([6, 7]).toContain(date.dayOfWeek);
        expect([2, 12]).toContain(date.month);
      },
    );
    check(gs.temporal.dates({ months: [7] }), (date) => expect(date.month).toBe(7));
    check(gs.temporal.dates({ weekdays: [3] }), (date) => expect(date.dayOfWeek).toBe(3));
  });

  test("covers Temporal's range by default and shrinks to 2000-01-01", () => {
    check(gs.temporal.dates(), (date) => {
      expect(date.year).toBeGreaterThanOrEqual(-271821);
      expect(date.year).toBeLessThanOrEqual(275760);
    });
    expect(minimal(gs.temporal.dates(), () => true).toString()).toBe("2000-01-01");
    expect(minimal(gs.temporal.dates(), (date) => date.year > 2000).toString()).toBe("2001-01-01");
  });

  test("draws distinct dates for sets", () => {
    check(
      gs.sets(
        gs.temporal.dates({
          minValue: { year: 2024, month: 1, day: 1 },
          maxValue: { year: 2024, month: 1, day: 3 },
        }),
        { minSize: 3 },
      ),
      (dates) => {
        expect(new Set([...dates].map((d) => d.toString())).size).toBe(3);
      },
      10,
    );
  });
});

describe("gs.temporal.times()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.times({ minValue: { hour: 2 }, maxValue: { hour: 1 } })).toThrow(
      "times(): cannot have maxValue < minValue",
    );
    expect(() => gs.temporal.times({ precision: "fortnight" as "hour" })).toThrow(
      "precision must be one of",
    );
  });

  test("has the engine report bounds holding no value at the precision when it is built", () => {
    expect(() =>
      gs.temporal.times({
        minValue: { hour: 1, nanosecond: 1 },
        maxValue: { hour: 1, nanosecond: 999 },
        precision: "microsecond",
      }),
    ).toThrow(/hegel_validate_time failed.*no value at precision microseconds lies in the bounds/);
  });

  test("draws at the requested precision within the bounds", () => {
    check(
      gs.temporal.times({
        minValue: { hour: 9 },
        maxValue: { hour: 17, minute: 30 },
        precision: "minutes",
      }),
      (time) => {
        expect(time).toBeInstanceOf(PlainTime);
        expect(time.hour).toBeGreaterThanOrEqual(9);
        expect(PlainTime.compare(time, { hour: 17, minute: 30 })).toBeLessThanOrEqual(0);
        expect(time.second + time.millisecond + time.microsecond + time.nanosecond).toBe(0);
      },
    );
    check(gs.temporal.times({ precision: "hour" }), (time) => expect(time.minute).toBe(0));
    expect(minimal(gs.temporal.times(), () => true).toString()).toBe("00:00:00");
    expect(
      minimal(
        gs.temporal.times(),
        (time) => time.nanosecond > 0 && time.millisecond > 0,
      ).toString(),
    ).toBe("00:00:00.001000001");
  });
});

describe("gs.temporal.datetimes()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.datetimes({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("datetimes(): cannot have maxValue < minValue");
    expect(() => gs.temporal.datetimes({ minValue: { year: 280000, month: 1, day: 1 } })).toThrow(
      "datetimes(): minValue +280000-01-01T00:00:00 is after the default maxValue, +275760-09-13T23:59:59.999999999: pass maxValue as well",
    );
    expect(() =>
      gs.temporal.datetimes({
        minValue: { year: 2024, month: 1, day: 1, second: 1 },
        maxValue: { year: 2024, month: 1, day: 1, second: 59 },
        precision: "minute",
      }),
    ).toThrow(/hegel_validate_datetime failed.*no value at precision minutes/);
  });

  test("draws within the bounds at the precision and shrinks to the millennium", () => {
    const min = new PlainDateTime(1999, 12, 31, 23);
    const max = new PlainDateTime(2000, 1, 1, 1);
    check(
      gs.temporal.datetimes({ minValue: min, maxValue: max, precision: "seconds" }),
      (value) => {
        expect(value).toBeInstanceOf(PlainDateTime);
        expect(PlainDateTime.compare(min, value)).toBeLessThanOrEqual(0);
        expect(PlainDateTime.compare(value, max)).toBeLessThanOrEqual(0);
        expect(value.millisecond + value.microsecond + value.nanosecond).toBe(0);
      },
    );
    expect(minimal(gs.temporal.datetimes(), () => true).toString()).toBe("2000-01-01T00:00:00");
    check(
      gs.temporal.datetimes({
        minValue: { year: -5, month: 1, day: 1 },
        maxValue: { year: -5, month: 1, day: 1 },
      }),
      (value) => {
        expect(value.toString()).toBe("-000005-01-01T00:00:00");
      },
      5,
    );
  });
});

describe("gs.temporal.instants()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.instants({ minValue: 1n, maxValue: 0n })).toThrow(
      "instants(): cannot have maxValue < minValue",
    );
    expect(() => gs.temporal.instants({ maxValue: -9_000_000_000_000_000_000_000n })).toThrow(
      "instants(): maxValue -283229-05-10T08:00:00Z is before the default minValue, -271821-04-20T00:00:00Z: pass minValue as well",
    );
    expect(() =>
      gs.temporal.instants({ minValue: 1n, maxValue: 999n, precision: "microseconds" }),
    ).toThrow(/hegel_validate_timestamp failed.*no value at precision microseconds/);
  });

  test("draws within Temporal's range by default and shrinks to the epoch", () => {
    check(gs.temporal.instants(), (instant) => {
      expect(instant).toBeInstanceOf(Instant);
      expect(instant.epochNanoseconds >= -8_640_000_000_000_000_000_000n).toBe(true);
      expect(instant.epochNanoseconds <= 8_640_000_000_000_000_000_000n).toBe(true);
    });
    const min = Instant.fromEpochMilliseconds(-1500);
    check(
      gs.temporal.instants({
        minValue: min,
        maxValue: { epochNanoseconds: 1_500_000_000n },
        precision: "second",
      }),
      (instant) => {
        expect(instant.epochNanoseconds % 1_000_000_000n).toBe(0n);
        expect(instant.epochNanoseconds >= -1_000_000_000n).toBe(true);
        expect(instant.epochNanoseconds <= 1_000_000_000n).toBe(true);
      },
    );
    expect(minimal(gs.temporal.instants(), () => true).toString()).toBe("2000-01-01T00:00:00Z");
    expect(
      minimal(
        gs.temporal.instants({ maxValue: 0n }),
        (instant) => instant.epochNanoseconds % 1_000_000_000n !== 0n,
      ).toString(),
    ).toBe("1969-12-31T23:59:59.999999999Z");
  });
});

describe("gs.temporal.offsets()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.offsets({ granularity: "day" as "hour" })).toThrow(
      "granularity must be one of",
    );
    expect(() => gs.temporal.offsets({ minValue: "+01:00", maxValue: "-01:00" })).toThrow(
      "offsets(): cannot have maxValue < minValue",
    );
    expect(() => gs.temporal.offsets({ minValue: 1.5 })).toThrow("minValue must be an integer");
    expect(() => gs.temporal.offsets({ maxValue: "1h" })).toThrow(
      "UTC offsets must be formatted as ±HH:MM or ±HH:MM:SS, got 1h",
    );
    expect(() => gs.temporal.offsets({ minValue: "+24:30" })).toThrow(
      "offsets(): minValue +24:30 is after the default maxValue, +23:59: pass maxValue as well",
    );
    expect(() => gs.temporal.offsets({ maxValue: -90_000, granularity: "seconds" })).toThrow(
      "offsets(): maxValue -25:00 is before the default minValue, -23:59:59: pass minValue as well",
    );
  });

  test("has the engine report its range and the granularity when it is built", () => {
    expect(() => gs.temporal.offsets({ minValue: 0, maxValue: 93_600 })).toThrow(
      /hegel_validate_offset failed.*must be in \[-93599, 93599\], got 93600/,
    );
    expect(() =>
      gs.temporal.offsets({ minValue: "+00:01", maxValue: "+00:59", granularity: "hours" }),
    ).toThrow(/hegel_validate_offset failed.*no offset at granularity hours/);
  });

  test("draws offsets at the granularity, as Temporal identifiers, shrinking to UTC", () => {
    check(
      gs.temporal.offsets({ minValue: "-02:00", maxValue: 7200, granularity: "quarter-hours" }),
      (offset) => {
        expect(offset).toMatch(/^[+-]0[0-2]:(00|15|30|45)$/);
      },
    );
    check(gs.temporal.offsets({ granularity: "second" }), (offset) => {
      expect(offset).toMatch(/^[+-]\d{2}:\d{2}(:\d{2})?$/);
    });
    check(gs.temporal.offsets(), (offset) => expect(offset).toMatch(/^[+-]\d{2}:\d{2}$/));
    check(
      gs.temporal.offsets({ minValue: 93_599, maxValue: "+25:59:59", granularity: "second" }),
      (offset) => expect(offset).toBe("+25:59:59"),
    );
    expect(minimal(gs.temporal.offsets(), () => true)).toBe("+00:00");
  });
});

describe("gs.temporal.zones()", () => {
  test("validates explicit names against its source", () => {
    expect(() => gs.temporal.zones({ names: [] })).toThrow("names must not be empty");
    expect(() => gs.temporal.zones({ names: ["Mars/Olympus_Mons"] })).toThrow(
      "The time zone Mars/Olympus_Mons is not known to the zone source in use (this host's database): pass the zone source that has it as source",
    );
    expect(() => gs.temporal.zones({ names: [5 as unknown as string] })).toThrow(
      "The time zone 5 is not known to the zone source in use",
    );
    expect(gs.temporal.zones({ names: ["UTC", "UTC"] }).asBasic()?.schema).toMatchObject({
      names: ["UTC"],
    });
  });

  test("has the engine check that the source has a name to draw when it is built", () => {
    expect(() => gs.temporal.zones({ source: ZoneSource.custom() })).toThrow(
      /hegel_timezone_spec_validate failed.*has no allowed name/,
    );
  });

  test("draws only names the host and the engine both know, shrinking to UTC", () => {
    const seen = new Set<string>();
    check(
      gs.temporal.zones(),
      (zone) => {
        expect(TIMEZONE_NAMES).toContain(zone);
        expect(hostKnowsZone(zone)).toBe(true);
        seen.add(zone);
      },
      200,
    );
    expect(seen.size).toBeGreaterThan(20);
    expect(minimal(gs.temporal.zones(), () => true)).toBe("UTC");
    check(gs.temporal.zones({ names: ["America/New_York", "Asia/Kolkata"] }), (zone) => {
      expect(["America/New_York", "Asia/Kolkata"]).toContain(zone);
    });
  });

  test.each([
    ["the host's database", ZoneSource.host()],
    ["the bundled database", ZoneSource.bundled()],
  ])("filters links and legacy names of %s", (_name, source) => {
    const lib = getLibhegel();
    const ctx = lib.newContext();
    const info = (zone: string) => lib.timezoneInfo(ctx, zone);
    const isLegacy = (zone: string) => (info(zone).tags & NativeTimeZoneTag.LEGACY) !== 0;
    const isLink = (zone: string) => info(zone).isLink;
    const drawn = (options: gs.temporal.ZoneOptions): string[] => {
      const names = new Set<string>();
      check(gs.temporal.zones({ ...options, source }), (zone) => void names.add(zone), 400);
      for (const name of names) expect(hostKnowsZone(name)).toBe(true);
      return [...names];
    };
    try {
      const plain = drawn({});
      expect(plain.some(isLegacy)).toBe(false);
      expect(plain.some(isLink)).toBe(true);
      expect(drawn({ allowLinks: false }).some(isLink)).toBe(false);
      const legacy = drawn({ allowLegacy: true });
      expect(legacy.some(isLegacy)).toBe(true);
      expect(legacy.some(isLink)).toBe(true);
      const neither = drawn({ allowLinks: false, allowLegacy: true });
      expect(neither.some(isLegacy)).toBe(true);
      expect(neither.some(isLink)).toBe(false);
    } finally {
      lib.freeContext(ctx);
    }
  });

  test("gives one generator for every draw of a whole source", () => {
    expect(gs.temporal.zones()).toBe(gs.temporal.zones({ allowLinks: true }));
    expect(gs.temporal.zones()).not.toBe(gs.temporal.zones({ allowLegacy: true }));
    expect(gs.temporal.zones()).not.toBe(gs.temporal.zones({ source: ZoneSource.bundled() }));
    expect(gs.temporal.zones({ names: ["UTC"] })).not.toBe(gs.temporal.zones({ names: ["UTC"] }));
  });
});

const newYork = gs.temporal.zones({ names: ["America/New_York"] });

describe("gs.temporal.zonedDatetimes()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.zonedDatetimes({ minValue: { year: 2024, month: 1, day: 1 }, maxInstant: 0n }),
    ).toThrow(
      "zonedDatetimes() takes either wall-clock bounds (minValue, maxValue) or instant bounds (minInstant, maxInstant), not both",
    );
    expect(() => gs.temporal.zonedDatetimes({ ambiguous: "sometimes" as "both" })).toThrow(
      'ambiguous must be "both", "earlier" or "later"',
    );
    expect(() => gs.temporal.zonedDatetimes({ ambiguous: "toString" as "both" })).toThrow(
      "ambiguous must be",
    );
    expect(() => gs.temporal.zonedDatetimes({ minInstant: 1n, maxInstant: 0n })).toThrow(
      "zonedDatetimes(): cannot have maxInstant < minInstant",
    );
    expect(() =>
      gs.temporal.zonedDatetimes({ minInstant: 9_000_000_000_000_000_000_000n }),
    ).toThrow(
      "zonedDatetimes(): minInstant +287168-08-24T16:00:00Z is after the default maxInstant, +275760-09-13T00:00:00Z: pass maxInstant as well",
    );
    expect(() =>
      gs.temporal.zonedDatetimes({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("zonedDatetimes(): cannot have maxValue < minValue");
    expect(() => gs.temporal.zonedDatetimes({ source: "host" as never })).toThrow(
      "source must be a ZoneSource, got host",
    );
  });

  test("has the engine check the bounds against the precision when it is built", () => {
    expect(() =>
      gs.temporal.zonedDatetimes({
        minValue: { year: 2024, month: 1, day: 1, second: 1 },
        maxValue: { year: 2024, month: 1, day: 1, second: 2 },
        precision: "hour",
      }),
    ).toThrow(/hegel_zoned_spec_validate failed.*has no value at precision hours/);
  });

  test("does not take a PlainDateTime where it takes an instant", () => {
    expect(() =>
      gs.temporal.zonedDatetimes({ minInstant: new PlainDateTime(2024, 1, 1) as never }),
    ).toThrow("Instants must be within");
    expect(() =>
      gs.temporal.instants({ maxValue: new PlainDateTime(2024, 1, 1) as never }),
    ).toThrow("Instants must be within");
  });

  test("reports gaps with the forward shift and both sides of a fold", () => {
    const gap = new PlainDateTime(2024, 3, 10, 2, 30);
    check(
      gs.temporal.zonedDatetimes({ zones: newYork, minValue: gap, maxValue: gap }),
      (value) => {
        expect(value).toBeInstanceOf(ZonedDateTime);
        expect(value.toString()).toBe("2024-03-10T02:30:00-04:00[America/New_York][gap]");
        expect(value.offsetBefore).toBe("-05:00");
        expect(value.toInstant().toString()).toBe("2024-03-10T07:30:00Z");
      },
      5,
    );
    const fold = new PlainDateTime(2024, 11, 3, 1, 30);
    const seen = new Set<string>();
    check(
      gs.temporal.zonedDatetimes({ zones: newYork, minValue: fold, maxValue: fold }),
      (value) => {
        expect(value.kind).toBe("fold");
        expect(value.offsetBefore).toBe("-04:00");
        expect(value.offsetAfter).toBe("-05:00");
        expect(value.toInstant().toString()).toBe(
          value.fold ? "2024-11-03T06:30:00Z" : "2024-11-03T05:30:00Z",
        );
        seen.add(value.toString());
      },
      40,
    );
    expect([...seen].sort()).toEqual([
      "2024-11-03T01:30:00-04:00[America/New_York][fold=0]",
      "2024-11-03T01:30:00-05:00[America/New_York][fold=1]",
    ]);
    check(
      gs.temporal.zonedDatetimes({
        zones: newYork,
        minValue: fold,
        maxValue: fold,
        ambiguous: "earlier",
      }),
      (value) => expect(value.fold).toBe(false),
      10,
    );
    check(
      gs.temporal.zonedDatetimes({
        zones: newYork,
        minValue: fold,
        maxValue: fold,
        ambiguous: "later",
      }),
      (value) => expect(value.fold).toBe(true),
      10,
    );
  });

  test("avoids gaps on request and draws instants at the precision", () => {
    check(
      gs.temporal.zonedDatetimes({
        zones: newYork,
        minValue: { year: 2024, month: 3, day: 10, hour: 1 },
        maxValue: { year: 2024, month: 3, day: 10, hour: 4 },
        allowImaginary: false,
        precision: "minute",
      }),
      (value) => {
        expect(value.kind).not.toBe("gap");
        expect(value.second + value.millisecond + value.microsecond + value.nanosecond).toBe(0);
      },
      100,
    );
    check(
      gs.temporal.zonedDatetimes({
        zones: newYork,
        minInstant: Instant.fromEpochMilliseconds(1_710_052_200_000),
        maxInstant: 1_710_059_400_000_000_000n,
      }),
      (value) => {
        expect(value.epochMilliseconds).toBeGreaterThanOrEqual(1_710_052_200_000);
        expect(value.epochMilliseconds).toBeLessThanOrEqual(1_710_059_400_000);
      },
    );
    check(
      gs.temporal.zonedDatetimes({ maxInstant: 0n }),
      (value) => expect(value.epochNanoseconds <= 0n).toBe(true),
      20,
    );
    check(
      gs.temporal.zonedDatetimes({ minInstant: 0n }),
      (value) => expect(value.epochNanoseconds >= 0n).toBe(true),
      20,
    );
  });

  test("draws any host zone by default and shrinks to the millennium in UTC", () => {
    check(
      gs.temporal.zonedDatetimes(),
      (value) => {
        expect(hostKnowsZone(value.timeZone)).toBe(true);
        expect(
          PlainDateTime.compare({ year: -271821, month: 4, day: 22 }, value),
        ).toBeLessThanOrEqual(0);
        expect(
          PlainDateTime.compare(value, {
            year: 275760,
            month: 9,
            day: 10,
            hour: 23,
            minute: 59,
            second: 59,
            millisecond: 999,
            microsecond: 999,
            nanosecond: 999,
          }),
        ).toBeLessThanOrEqual(0);
      },
      100,
    );
    expect(minimal(gs.temporal.zonedDatetimes(), () => true).toString()).toBe(
      "2000-01-01T00:00:00+00:00[UTC]",
    );
    expect(gs.temporal.zonedDatetimes().asBasic()).not.toBeNull();
    expect(gs.temporal.zonedDatetimes({ zones: newYork }).asBasic()).not.toBeNull();
  });

  test("takes the zone from any generator of names, under one span", () => {
    const generator = gs.temporal.zonedDatetimes({
      zones: gs.sampledFrom(["Europe/London", "Asia/Tokyo"]),
      minValue: { year: 2024, month: 6, day: 1 },
      maxValue: { year: 2024, month: 6, day: 30 },
    });
    expect(generator.asBasic()).toBeNull();
    check(generator, (value) => {
      expect(["Europe/London", "Asia/Tokyo"]).toContain(value.timeZone);
      expect(value.offset).toBe(value.timeZone === "Asia/Tokyo" ? "+09:00" : "+01:00");
    });
    expect(draw(gs.temporal.zonedDatetimes({ zones: gs.just(5 as unknown as string) }))).toThrow(
      "The time zone 5 is not known to the zone source in use",
    );
    expect(
      minimal(
        gs.temporal.zonedDatetimes({ zones: gs.sampledFrom(["Asia/Tokyo", "Europe/London"]) }),
        () => true,
      ).toString(),
    ).toBe("2000-01-01T00:00:00+09:00[Asia/Tokyo]");
    expect(
      minimal(
        gs.temporal.zonedDatetimes({
          zones: gs.temporal.zones().filter((zone) => zone.startsWith("Asia/")),
        }),
        (value) => value.year > 2000,
      ).toString(),
    ).toMatch(/^2001-01-01T00:00:00[+-]\d{2}:\d{2}\[Asia\/\w+\]$/);
  });

  test("rejects the test case when the zone has no value in the bounds", () => {
    const seen = new Set<string>();
    check(
      gs.temporal.zonedDatetimes({
        zones: gs.sampledFrom(["Asia/Tokyo", "Asia/Kolkata"]),
        minInstant: 1_710_050_400_000_000_000n,
        maxInstant: 1_710_050_400_000_000_000n + 1_799_999_999_999n,
        precision: "hour",
      }),
      (value) => {
        expect(value.toString()).toBe("2024-03-10T15:00:00+09:00[Asia/Tokyo]");
        seen.add(value.timeZone);
      },
      20,
    );
    expect([...seen]).toEqual(["Asia/Tokyo"]);
  });

  test("rejects a draw whose offset the host disagrees with", () => {
    const utc = gs.temporal.zones({ names: ["UTC"], source: ZoneSource.bundled() });
    const format = vi
      .spyOn(Intl.DateTimeFormat.prototype, "format", "get")
      .mockReturnValue(() => "1/1/2000, GMT+01:00");
    try {
      expect(draw(gs.temporal.zonedDatetimes({ zones: utc }))).toThrow(
        hegel.FailedHealthCheckError,
      );
    } finally {
      format.mockRestore();
    }
    check(gs.temporal.zonedDatetimes({ zones: utc }), (value) =>
      expect(value.offset).toBe("+00:00"),
    );
  });

  test("skips the host check beyond the range Intl covers", () => {
    const far = new PlainDateTime(275000, 1, 1);
    check(
      gs.temporal.zonedDatetimes({ zones: newYork, minValue: far, maxValue: far }),
      (value) => {
        expect(value.year).toBe(275000);
      },
      3,
    );
  });

  test("prints a drawn value as the engine writes it", () => {
    const gap = new PlainDateTime(2024, 3, 10, 2, 30);
    expect(
      report(gs.temporal.zonedDatetimes({ zones: newYork, minValue: gap, maxValue: gap })),
    ).toContain("const draw_1 = ZonedDateTime(2024-03-10T02:30:00-04:00[America/New_York][gap]);");
    expect(report(gs.temporal.dates())).toContain("const draw_1 = PlainDate(2000-01-01);");
    expect(report(gs.arrays(gs.temporal.times(), { minSize: 1, maxSize: 1 }))).toContain(
      "const draw_1 = [PlainTime(00:00:00)];",
    );
    expect(report(gs.temporal.spans())).toContain("const draw_1 = Duration(0s);");
    expect(report(gs.temporal.offsets())).toContain('const draw_1 = "+00:00";');
  });
});

describe("gs.temporal.spans()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.spans({ sign: "mixed" as "uniform" })).toThrow(
      'sign must be "uniform" or "non-negative"',
    );
    expect(() => gs.temporal.spans({ hours: { minValue: 2, maxValue: 1 } })).toThrow(
      "hours: cannot have maxValue < minValue",
    );
    expect(() => gs.temporal.spans({ years: { maxValue: 2 ** 32 } })).toThrow(
      "years.maxValue must be an integer between -4294967295 and 4294967295",
    );
    expect(() => gs.temporal.spans({ days: { minValue: 0.5 } })).toThrow(
      "days.minValue must be an integer",
    );
    expect(() => gs.temporal.spans({ days: { maxValue: -1 }, sign: "non-negative" })).toThrow(
      "days: maxValue -1 is before the default minValue, 0: pass minValue as well",
    );
  });

  test("has the engine check the signs when it is built", () => {
    expect(() => gs.temporal.spans({ minutes: { minValue: -1 }, sign: "non-negative" })).toThrow(
      /hegel_span_spec_validate failed.*requires min >= 0 for minutes, got -1/,
    );
    expect(() => gs.temporal.spans({ hours: { minValue: 1 }, minutes: { maxValue: -1 } })).toThrow(
      /hegel_span_spec_validate failed.*admit none in common/,
    );
  });

  test("draws Temporal-valid durations of one sign, shrinking to zero", () => {
    check(
      gs.temporal.spans(),
      (duration) => {
        expect(duration).toBeInstanceOf(Duration);
        expect(Math.abs(duration.years)).toBeLessThanOrEqual(2 ** 32 - 1);
        expect(Math.abs(duration.days)).toBeLessThanOrEqual(1e10);
      },
      100,
    );
    expect(minimal(gs.temporal.spans(), () => true).toString()).toBe("0s");
    check(
      gs.temporal.spans({ hours: { minValue: 1, maxValue: 3 }, minutes: { maxValue: 59 } }),
      (duration) => {
        expect(duration.sign).toBe(1);
        expect(duration.hours).toBeGreaterThanOrEqual(1);
        expect(duration.hours).toBeLessThanOrEqual(3);
        expect(duration.minutes).toBeLessThanOrEqual(59);
        expect(
          duration.years +
            duration.months +
            duration.weeks +
            duration.days +
            duration.seconds +
            duration.milliseconds +
            duration.microseconds +
            duration.nanoseconds,
        ).toBe(0);
      },
    );
    check(gs.temporal.spans({ sign: "non-negative", days: {} }), (duration) => {
      expect(duration.sign).toBeGreaterThanOrEqual(0);
      expect(duration.hours).toBe(0);
    });
    check(gs.temporal.spans({ seconds: { minValue: -5, maxValue: -2 } }), (duration) => {
      expect(duration.seconds).toBeGreaterThanOrEqual(-5);
      expect(duration.seconds).toBeLessThanOrEqual(-2);
    });
    expect(gs.temporal.spans({ sign: "non-negative" }).asBasic()?.schema).toMatchObject({
      sign: NativeSpanSign.NONNEGATIVE,
      units: expect.arrayContaining([{ unit: 0, min: 0n, max: 4294967295n }]),
    });
  });
});

describe("gs.temporal.tzdbVersion()", () => {
  test("reports the release of the database the engine bundles", () => {
    expect(gs.temporal.tzdbVersion()).toBe(
      readFileSync(
        `${import.meta.dirname}/../../../libhegel/src/tzdata/version.txt`,
        "utf8",
      ).trim(),
    );
  });

  test("works when only the generators entry point is imported", async () => {
    vi.resetModules();
    const fresh = await import("../src/generators/node.js");
    expect(fresh.temporal.tzdbVersion()).toBe(gs.temporal.tzdbVersion());
  });

  test("needs a composition root to have loaded the engine", async () => {
    vi.resetModules();
    const fresh = await import("../src/generators/temporal.js");
    expect(() => fresh.tzdbVersion()).toThrow('import "@hegeldev/hegel"');
    expect(() => new fresh.PlainDate(2024, 3, 10).toString()).toThrow('import "@hegeldev/hegel"');
  });
});

describe("gs.temporal.jsDates()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.jsDates({ minValue: new Date(NaN) })).toThrow(
      "minValue must be a valid Date",
    );
    expect(() => gs.temporal.jsDates({ maxValue: "2024" as unknown as Date })).toThrow(
      "maxValue must be a valid Date",
    );
    expect(() => gs.temporal.jsDates({ minValue: new Date(1), maxValue: new Date(0) })).toThrow(
      "jsDates(): cannot have maxValue < minValue",
    );
    expect(gs.temporal.jsDates().asBasic()?.schema).toMatchObject({
      precision: NativePrecision.MILLISECONDS,
      min: { second: -8_640_000_000_000n, nanosecond: 0 },
    });
  });

  test("draws valid Dates within the bounds, shrinking to the epoch", () => {
    check(
      gs.temporal.jsDates(),
      (date) => {
        expect(date).toBeInstanceOf(Date);
        expect(Number.isNaN(date.getTime())).toBe(false);
      },
      100,
    );
    const min = new Date(-1500);
    const max = new Date(1500);
    check(gs.temporal.jsDates({ minValue: min, maxValue: max }), (date) => {
      expect(date.getTime()).toBeGreaterThanOrEqual(-1500);
      expect(date.getTime()).toBeLessThanOrEqual(1500);
    });
    expect(minimal(gs.temporal.jsDates(), () => true).toISOString()).toBe(
      "2000-01-01T00:00:00.000Z",
    );
  });
});
