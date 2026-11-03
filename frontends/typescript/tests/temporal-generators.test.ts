/**
 * The date and time generators against the real engine: argument validation,
 * the values they draw, their shrink targets, and the handle lifecycle of the
 * schema interpreter's date and time draws against the fake engine.
 */

import { describe, test, it, expect, vi } from "vitest";
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
import { generateValue, HandleCache } from "../src/generate.js";
import { EngineError, NativePrecision, NativeSpanSign, NativeTimeZoneTag } from "../src/engine.js";
import { AssumeError } from "../src/testCase.js";
import {
  fakeEngine,
  context,
  testCase,
  dateOptions,
  timeZoneSpec,
  zonedSpec,
  spanSpec,
} from "./fakeEngine.js";
import { TIMEZONE_NAMES } from "../src/timezone-names.js";
import { hostKnowsZone } from "../src/hostZones.js";

/** Run `body` as a property and return the shrunk failing draw of `generator`. */
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

function check<T>(generator: gs.Generator<T>, body: (value: T) => void, testCases = 60): void {
  hegel.test((tc) => body(tc.draw(generator)), { testCases, database: hegel.Database.disabled });
}

describe("gs.temporal.dates()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.dates({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("maxValue < minValue");
    expect(() => gs.temporal.dates({ weekdays: [0] })).toThrow(
      "weekdays must be integers between 1 and 7",
    );
    expect(() => gs.temporal.dates({ months: [13] })).toThrow(
      "months must be integers between 1 and 12",
    );
    expect(() => gs.temporal.dates({ weekdays: [] })).toThrow("weekdays must not be empty");
    const tuesday = { year: 2024, month: 3, day: 12 };
    expect(() =>
      gs.temporal.dates({ minValue: tuesday, maxValue: tuesday, weekdays: [1] }),
    ).toThrow("no date between 2024-03-12 and 2024-03-12");
    expect(() =>
      gs.temporal.dates({
        minValue: tuesday,
        maxValue: { year: 2024, month: 3, day: 31 },
        weekdays: [2],
        months: [4],
      }),
    ).toThrow("allowed weekdays and months");
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
      "maxValue < minValue",
    );
    expect(() => gs.temporal.times({ precision: "fortnight" as "hour" })).toThrow(
      "precision must be one of",
    );
    expect(() =>
      gs.temporal.times({
        minValue: { hour: 1, nanosecond: 1 },
        maxValue: { hour: 1, nanosecond: 999 },
        precision: "microsecond",
      }),
    ).toThrow(
      "No time between 01:00:00.000000001 and 01:00:00.000000999 has the requested precision",
    );
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
  });
});

describe("gs.temporal.datetimes()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.datetimes({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("maxValue < minValue");
    expect(() =>
      gs.temporal.datetimes({
        minValue: { year: 2024, month: 1, day: 1, second: 1 },
        maxValue: { year: 2024, month: 1, day: 1, second: 59 },
        precision: "minute",
      }),
    ).toThrow("No datetime between");
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
      "maxValue < minValue",
    );
    expect(() =>
      gs.temporal.instants({ minValue: 1n, maxValue: 999n, precision: "microseconds" }),
    ).toThrow("No instant between");
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
  });
});

describe("gs.temporal.offsets()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.offsets({ granularity: "day" as "hour" })).toThrow(
      "granularity must be one of",
    );
    expect(() => gs.temporal.offsets({ minValue: "+01:00", maxValue: "-01:00" })).toThrow(
      "maxValue < minValue",
    );
    expect(() => gs.temporal.offsets({ minValue: 1.5 })).toThrow(
      "minValue must be a ±HH:MM[:SS] string",
    );
    expect(() => gs.temporal.offsets({ maxValue: 93_600 })).toThrow("maxValue must be");
    expect(() =>
      gs.temporal.offsets({ minValue: "+00:01", maxValue: "+00:59", granularity: "hours" }),
    ).toThrow("No offset between +00:01 and +00:59 has the requested granularity");
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
    expect(minimal(gs.temporal.offsets(), () => true)).toBe("+00:00");
  });
});

describe("gs.temporal.zones()", () => {
  test("validates explicit names against the host", () => {
    expect(() => gs.temporal.zones({ names: [] })).toThrow("names must not be empty");
    expect(() => gs.temporal.zones({ names: ["Mars/Olympus_Mons"] })).toThrow(
      "Mars/Olympus_Mons is not known to this host",
    );
    expect(() => gs.temporal.zones({ names: [5 as unknown as string] })).toThrow(
      "5 is not known to this host",
    );
    expect(gs.temporal.zones({ names: ["UTC", "UTC"] }).asBasic()?.schema).toMatchObject({
      names: ["UTC"],
    });
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

  test("filters links and legacy names", () => {
    check(
      gs.temporal.zones({ links: false }),
      (zone) => expect(zone).not.toBe("Asia/Calcutta"),
      300,
    );
    check(
      gs.temporal.zones({ legacy: true }),
      (zone) => expect(TIMEZONE_NAMES).toContain(zone),
      100,
    );
    const legacy = new Set<string>();
    check(gs.temporal.zones({ links: false, legacy: true }), (zone) => legacy.add(zone), 300);
    expect(
      [...legacy].some(
        (zone) =>
          zone === "EST5EDT" || zone === "GMT" || zone === "US/Eastern" || zone.startsWith("Etc/"),
      ),
    ).toBe(true);
  });

  test("names the engine lacks fail at draw time", () => {
    const engine = fakeEngine();
    engine.timezoneSpecSetNames.mockImplementation(() => {
      throw new EngineError("unknown zone");
    });
    expect(() =>
      generateValue(engine, context, testCase, {
        type: "timezone",
        names: ["Bogus"],
        links: true,
        legacy: false,
        source: gs.temporal.ZoneSource.bundled(),
      }),
    ).toThrow("unknown zone");
    expect(engine.freeTimezoneSpec).toHaveBeenCalledWith(timeZoneSpec);
  });
});

const newYork = gs.temporal.zones({ names: ["America/New_York"] });

describe("gs.temporal.zonedDateTimes()", () => {
  test("validates its options", () => {
    expect(() =>
      gs.temporal.zonedDateTimes({ minValue: { year: 2024, month: 1, day: 1 }, maxInstant: 0n }),
    ).toThrow("not both");
    expect(() => gs.temporal.zonedDateTimes({ ambiguous: "sometimes" as "both" })).toThrow(
      'ambiguous must be "both", "earlier" or "later"',
    );
    expect(() => gs.temporal.zonedDateTimes({ minInstant: 1n, maxInstant: 0n })).toThrow(
      "maxValue < minValue",
    );
    expect(() =>
      gs.temporal.zonedDateTimes({
        minValue: { year: 2024, month: 1, day: 2 },
        maxValue: { year: 2024, month: 1, day: 1 },
      }),
    ).toThrow("maxValue < minValue");
    expect(() =>
      gs.temporal.zonedDateTimes({
        minValue: { year: 2024, month: 1, day: 1, second: 1 },
        maxValue: { year: 2024, month: 1, day: 1, second: 2 },
        precision: "hour",
      }),
    ).toThrow("No datetime between");
  });

  test("reports gaps with the forward shift and both sides of a fold", () => {
    const gap = new PlainDateTime(2024, 3, 10, 2, 30);
    check(
      gs.temporal.zonedDateTimes({ zones: newYork, minValue: gap, maxValue: gap }),
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
      gs.temporal.zonedDateTimes({ zones: newYork, minValue: fold, maxValue: fold }),
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
      gs.temporal.zonedDateTimes({
        zones: newYork,
        minValue: fold,
        maxValue: fold,
        ambiguous: "earlier",
      }),
      (value) => expect(value.fold).toBe(false),
      10,
    );
    check(
      gs.temporal.zonedDateTimes({
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
      gs.temporal.zonedDateTimes({
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
      gs.temporal.zonedDateTimes({
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
      gs.temporal.zonedDateTimes({ maxInstant: 0n }),
      (value) => expect(value.epochNanoseconds <= 0n).toBe(true),
      20,
    );
    check(
      gs.temporal.zonedDateTimes({ minInstant: 0n }),
      (value) => expect(value.epochNanoseconds >= 0n).toBe(true),
      20,
    );
  });

  test("draws any host zone by default and shrinks to the millennium in UTC", () => {
    check(
      gs.temporal.zonedDateTimes(),
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
    expect(minimal(gs.temporal.zonedDateTimes(), () => true).toString()).toBe(
      "2000-01-01T00:00:00+00:00[UTC]",
    );
    expect(gs.temporal.zonedDateTimes().asBasic()).not.toBeNull();
  });

  test("takes the zone from any generator of names, under one span", () => {
    const generator = gs.temporal.zonedDateTimes({
      zones: gs.sampledFrom(["Europe/London", "Asia/Tokyo"]),
      minValue: { year: 2024, month: 6, day: 1 },
      maxValue: { year: 2024, month: 6, day: 30 },
    });
    expect(generator.asBasic()).toBeNull();
    check(generator, (value) => {
      expect(["Europe/London", "Asia/Tokyo"]).toContain(value.timeZone);
      expect(value.offset).toBe(value.timeZone === "Asia/Tokyo" ? "+09:00" : "+01:00");
    });
    expect(() =>
      hegel.test((tc) =>
        tc.draw(gs.temporal.zonedDateTimes({ zones: gs.just(5 as unknown as string) })),
      ),
    ).toThrow("needs a generator of time zone names, got 5");
    expect(() =>
      hegel.test((tc) =>
        tc.draw(gs.temporal.zonedDateTimes({ zones: gs.just("Mars/Olympus_Mons") })),
      ),
    ).toThrow("not known to this host");
    expect(
      minimal(
        gs.temporal.zonedDateTimes({ zones: gs.sampledFrom(["Asia/Tokyo", "Europe/London"]) }),
        () => true,
      ).toString(),
    ).toBe("2000-01-01T00:00:00+09:00[Asia/Tokyo]");
  });

  test("rejects a draw whose offset the host disagrees with", () => {
    const engine = fakeEngine();
    const schema = {
      type: "zoned_datetime",
      names: ["UTC"],
      links: true,
      legacy: false,
      civil: null,
      instant: null,
      precision: 0,
      imaginary: 0,
      ambiguous: 0,
    };
    const drawn = generateValue(engine, context, testCase, schema) as ReturnType<
      typeof engine.generateZonedDatetime
    >;
    expect(drawn).toMatchObject({ zone: "UTC" });
    engine.generateZonedDatetime.mockImplementation(() => ({
      ...drawn,
      value: { ...drawn.value, offsetSeconds: 3600 },
    }));
    expect(() => generateValue(engine, context, testCase, schema)).toThrow(AssumeError);
    expect(engine.freeZonedSpec).toHaveBeenCalledTimes(2);
    expect(engine.freeTimezoneSpec).toHaveBeenCalledTimes(2);
  });

  test("skips the host check beyond the range Intl covers", () => {
    const far = new PlainDateTime(275000, 1, 1);
    check(
      gs.temporal.zonedDateTimes({ zones: newYork, minValue: far, maxValue: far }),
      (value) => {
        expect(value.year).toBe(275000);
      },
      3,
    );
  });

  test("draws the name, then the value from a spec of that zone, freeing every spec", () => {
    const engine = fakeEngine();
    const schema = {
      type: "zoned_datetime",
      names: null,
      links: true,
      legacy: false,
      source: null,
      civil: null,
      instant: null,
      precision: 0,
      imaginary: 0,
      ambiguous: 0,
    };
    const cache = new HandleCache(engine);
    generateValue(engine, context, testCase, schema, cache);
    generateValue(engine, context, testCase, schema, cache);
    expect(engine.generateTimezone).toHaveBeenCalledTimes(2);
    expect(engine.timezoneSpecNew).toHaveBeenCalledTimes(2);
    expect(engine.timezoneSpecSetNames).toHaveBeenLastCalledWith(context, timeZoneSpec, ["UTC"]);
    expect(engine.zoneSourceAddZone).toHaveBeenCalledTimes(1);
    cache.dispose();
    expect(engine.freeTimezoneSpec).toHaveBeenCalledTimes(2);
    expect(engine.freeZonedSpec).toHaveBeenCalledTimes(1);
    engine.zonedSpecNew.mockImplementation(() => {
      throw new EngineError("no spec");
    });
    expect(() => generateValue(engine, context, testCase, { ...schema, names: ["UTC"] })).toThrow(
      "no spec",
    );
    expect(engine.freeTimezoneSpec).toHaveBeenCalledTimes(2);
  });

  test("releases the specs when configuring them fails", () => {
    const engine = fakeEngine();
    const schema = {
      type: "zoned_datetime",
      names: null,
      links: true,
      legacy: false,
      civil: { min: {}, max: {} },
      instant: { min: {}, max: {} },
      precision: 0,
      imaginary: 0,
      ambiguous: 0,
    };
    engine.zonedSpecSetAmbiguous.mockImplementation(() => {
      throw new EngineError("bad policy");
    });
    expect(() => generateValue(engine, context, testCase, schema)).toThrow("bad policy");
    expect(engine.zonedSpecSetCivilBounds).toHaveBeenCalledTimes(1);
    expect(engine.zonedSpecSetInstantBounds).toHaveBeenCalledTimes(1);
    expect(engine.freeZonedSpec).toHaveBeenCalledWith(zonedSpec);
    expect(engine.freeTimezoneSpec).toHaveBeenCalledWith(timeZoneSpec);
    engine.zonedSpecNew.mockImplementation(() => {
      throw new EngineError("no spec");
    });
    expect(() => generateValue(engine, context, testCase, schema)).toThrow("no spec");
    expect(engine.freeTimezoneSpec).toHaveBeenCalledTimes(2);
    expect(engine.freeZonedSpec).toHaveBeenCalledTimes(1);
  });

  test("fills the host zone list once per engine and filters it", () => {
    const engine = fakeEngine();
    engine.timezoneInfo.mockImplementation((_ctx, name) => ({
      known: true,
      primaryName: name,
      isLink: name === "Asia/Calcutta",
      rank: 0,
      tags: name === "EST5EDT" ? NativeTimeZoneTag.LEGACY : 0,
    }));
    const names = (links: boolean, legacy: boolean) => {
      generateValue(engine, context, testCase, {
        type: "timezone",
        names: null,
        links,
        legacy,
        source: gs.temporal.ZoneSource.bundled(),
      });
      return engine.timezoneSpecSetNames.mock.lastCall![2];
    };
    expect(names(true, true)).toContain("Asia/Calcutta");
    expect(names(true, true)).toContain("EST5EDT");
    expect(names(false, true)).not.toContain("Asia/Calcutta");
    expect(names(true, false)).not.toContain("EST5EDT");
    expect(names(false, false)).toContain("America/New_York");
    expect(names(true, true)).not.toContain("Factory");
    const lookups = engine.timezoneInfo.mock.calls.length;
    expect(lookups).toBeGreaterThan(400);
    names(true, true);
    expect(engine.timezoneInfo).toHaveBeenCalledTimes(lookups);
  });
});

describe("gs.temporal.spans()", () => {
  test("validates its options", () => {
    expect(() => gs.temporal.spans({ sign: "mixed" as "uniform" })).toThrow(
      'sign must be "uniform" or "non-negative"',
    );
    expect(() => gs.temporal.spans({ hours: { min: 2, max: 1 } })).toThrow(
      "hours: cannot have max < min",
    );
    expect(() => gs.temporal.spans({ years: { max: 2 ** 32 } })).toThrow(
      "years.max must be an integer within ±4294967295",
    );
    expect(() => gs.temporal.spans({ days: { min: 0.5 } })).toThrow("days.min must be an integer");
    expect(() => gs.temporal.spans({ minutes: { min: -1 }, sign: "non-negative" })).toThrow(
      'minutes.min must not be negative with sign "non-negative"',
    );
    expect(() => gs.temporal.spans({ hours: { min: 1 }, minutes: { max: -1 } })).toThrow(
      "no sign every unit can share",
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
    expect(minimal(gs.temporal.spans(), () => true).toString()).toBe("PT0S");
    check(gs.temporal.spans({ hours: { min: 1, max: 3 }, minutes: { max: 59 } }), (duration) => {
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
    });
    check(gs.temporal.spans({ sign: "non-negative", days: {} }), (duration) => {
      expect(duration.sign).toBeGreaterThanOrEqual(0);
      expect(duration.hours).toBe(0);
    });
    check(gs.temporal.spans({ seconds: { min: -5, max: -2 } }), (duration) => {
      expect(duration.seconds).toBeGreaterThanOrEqual(-5);
      expect(duration.seconds).toBeLessThanOrEqual(-2);
    });
    expect(gs.temporal.spans({ sign: "non-negative" }).asBasic()?.schema).toMatchObject({
      sign: NativeSpanSign.NONNEGATIVE,
      units: expect.arrayContaining([{ unit: 0, min: 0n, max: 4294967295n }]),
    });
  });

  test("releases the spec when configuring it fails", () => {
    const engine = fakeEngine();
    engine.spanSpecSetSign.mockImplementation(() => {
      throw new EngineError("bad sign");
    });
    expect(() =>
      generateValue(engine, context, testCase, gs.temporal.spans().asBasic()!.schema),
    ).toThrow("bad sign");
    expect(engine.spanSpecSetUnitBounds).toHaveBeenCalledTimes(10);
    expect(engine.freeSpanSpec).toHaveBeenCalledWith(spanSpec);
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
      "maxValue < minValue",
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

describe("date option handles", () => {
  it("are built once per schema and released on failure and disposal", () => {
    const engine = fakeEngine();
    const cache = new HandleCache(engine);
    const schema = gs.temporal.dates({ weekdays: [1], months: [1] }).asBasic()!.schema;
    generateValue(engine, context, testCase, schema, cache);
    generateValue(engine, context, testCase, schema, cache);
    expect(engine.dateOptionsNew).toHaveBeenCalledTimes(1);
    expect(engine.dateOptionsSetWeekdays).toHaveBeenCalledWith(context, dateOptions, 1);
    expect(engine.dateOptionsSetMonths).toHaveBeenCalledWith(context, dateOptions, 1);
    expect(engine.generateDate).toHaveBeenLastCalledWith(
      context,
      testCase,
      { year: -271821, month: 4, day: 19 },
      { year: 275760, month: 9, day: 13 },
      dateOptions,
    );
    generateValue(engine, context, testCase, gs.temporal.dates().asBasic()!.schema, cache);
    expect(engine.generateDate).toHaveBeenLastCalledWith(
      context,
      testCase,
      expect.anything(),
      expect.anything(),
      null,
    );
    cache.dispose();
    expect(engine.freeDateOptions).toHaveBeenCalledWith(dateOptions);
    engine.dateOptionsSetMonths.mockImplementation(() => {
      throw new EngineError("bad mask");
    });
    expect(() =>
      generateValue(
        engine,
        context,
        testCase,
        gs.temporal.dates({ months: [2] }).asBasic()!.schema,
      ),
    ).toThrow("bad mask");
    expect(engine.freeDateOptions).toHaveBeenCalledTimes(2);
  });
});
