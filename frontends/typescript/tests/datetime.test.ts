/**
 * The date and time value classes: validation, ordering, formatting and the
 * `Temporal` property-bag compatibility they promise; plus the host's `Intl`
 * view of time zones.
 */

import { describe, it, expect, vi } from "vitest";
import {
  PlainDate,
  PlainTime,
  PlainDateTime,
  Instant,
  ZonedDateTime,
  Duration,
  DURATION_UNITS,
  civilFromDays,
  daysFromCivil,
  daysInMonth,
  isLeapYear,
  formatUtcOffset,
  parseUtcOffset,
} from "../src/datetime.js";
import { hostKnowsZone, hostOffsetSeconds, hostZoneFormatter } from "../src/hostZones.js";
import { TIMEZONE_NAMES, TZDB_VERSION } from "../src/timezone-names.js";

describe("calendar arithmetic", () => {
  it("round-trips days since the epoch through civil dates", () => {
    expect(daysFromCivil(1970, 1, 1)).toBe(0);
    expect(daysFromCivil(2000, 3, 1)).toBe(11017);
    expect(daysFromCivil(-271821, 4, 19)).toBe(-100_000_001);
    expect(daysFromCivil(275760, 9, 13)).toBe(100_000_000);
    for (const days of [-100_000_001, -1, 0, 59, 11017, 100_000_000]) {
      const date = civilFromDays(days);
      expect(daysFromCivil(date.year, date.month, date.day)).toBe(days);
    }
    expect(civilFromDays(-1).toString()).toBe("1969-12-31");
  });

  it("knows month lengths and leap years", () => {
    expect(isLeapYear(2000)).toBe(true);
    expect(isLeapYear(1900)).toBe(false);
    expect(isLeapYear(2024)).toBe(true);
    expect(isLeapYear(2023)).toBe(false);
    expect(daysInMonth(2024, 2)).toBe(29);
    expect(daysInMonth(2023, 2)).toBe(28);
    expect(daysInMonth(2023, 4)).toBe(30);
    expect(daysInMonth(2023, 12)).toBe(31);
  });
});

describe("PlainDate", () => {
  it("validates its fields", () => {
    expect(() => new PlainDate(2023, 2, 29)).toThrow("day must be an integer between 1 and 28");
    expect(() => new PlainDate(2023, 13, 1)).toThrow("month must be an integer between 1 and 12");
    expect(() => new PlainDate(1_000_000, 1, 1)).toThrow("year must be an integer");
    expect(() => new PlainDate(2023.5, 1, 1)).toThrow("year must be an integer");
    expect(() => new PlainDate("2023" as unknown as number, 1, 1)).toThrow(
      "year must be an integer",
    );
    expect(Object.isFrozen(new PlainDate(2024, 2, 29))).toBe(true);
  });

  it("formats four-digit years plainly and others with six signed digits", () => {
    expect(new PlainDate(2024, 3, 10).toString()).toBe("2024-03-10");
    expect(new PlainDate(1, 1, 1).toString()).toBe("0001-01-01");
    expect(new PlainDate(0, 1, 1).toString()).toBe("0000-01-01");
    expect(new PlainDate(-5, 1, 1).toString()).toBe("-000005-01-01");
    expect(new PlainDate(10000, 1, 1).toString()).toBe("+010000-01-01");
    expect(new PlainDate(-271821, 4, 19).toJSON()).toBe("-271821-04-19");
    expect(JSON.stringify({ date: new PlainDate(275760, 9, 13) })).toBe('{"date":"+275760-09-13"}');
  });

  it("converts from property bags, compares and knows the weekday", () => {
    const date = new PlainDate(2024, 3, 10);
    expect(PlainDate.from(date)).toBe(date);
    expect(PlainDate.from({ year: 2024, month: 3, day: 10 }).equals(date)).toBe(true);
    expect(PlainDate.compare(date, { year: 2024, month: 3, day: 11 })).toBe(-1);
    expect(PlainDate.compare({ year: 2025, month: 1, day: 1 }, date)).toBe(1);
    expect(PlainDate.compare(date, date)).toBe(0);
    expect(date.dayOfWeek).toBe(7);
    expect(new PlainDate(1970, 1, 1).dayOfWeek).toBe(4);
    expect(new PlainDate(1969, 12, 31).dayOfWeek).toBe(3);
    expect(new PlainDate(2024, 3, 11).dayOfWeek).toBe(1);
    expect(date.epochDays).toBe(19792);
  });
});

describe("PlainTime", () => {
  it("validates its fields and defaults them to zero", () => {
    expect(new PlainTime().toString()).toBe("00:00:00");
    expect(() => new PlainTime(24)).toThrow("hour must be an integer between 0 and 23");
    expect(() => new PlainTime(0, 60)).toThrow("minute must be an integer between 0 and 59");
    expect(() => new PlainTime(0, 0, 60)).toThrow("second must be an integer between 0 and 59");
    expect(() => new PlainTime(0, 0, 0, 1000)).toThrow("millisecond must be an integer");
    expect(() => new PlainTime(0, 0, 0, 0, 1000)).toThrow("microsecond must be an integer");
    expect(() => new PlainTime(0, 0, 0, 0, 0, 1000)).toThrow("nanosecond must be an integer");
  });

  it("prints the fraction without trailing zeros", () => {
    expect(new PlainTime(2, 30).toString()).toBe("02:30:00");
    expect(new PlainTime(2, 30, 0, 500).toString()).toBe("02:30:00.5");
    expect(new PlainTime(23, 59, 59, 999, 999, 999).toJSON()).toBe("23:59:59.999999999");
    expect(new PlainTime(1, 2, 3, 0, 0, 42).toString()).toBe("01:02:03.000000042");
    expect(new PlainTime(1, 2, 3, 0, 42).toString()).toBe("01:02:03.000042");
  });

  it("converts from bags and nanoseconds of day, and compares", () => {
    const time = new PlainTime(12, 34, 56, 7, 8, 9);
    expect(PlainTime.from(time)).toBe(time);
    expect(
      PlainTime.from({
        hour: 12,
        minute: 34,
        second: 56,
        millisecond: 7,
        microsecond: 8,
        nanosecond: 9,
      }),
    ).toEqual(time);
    expect(PlainTime.from({ hour: 5 }).toString()).toBe("05:00:00");
    expect(time.nanosecondsOfDay).toBe(45_296_007_008_009);
    expect(PlainTime.fromNanosecondsOfDay(45_296_007_008_009)).toEqual(time);
    expect(PlainTime.fromNanosecondsOfDay(86_399_999_999_999).toString()).toBe(
      "23:59:59.999999999",
    );
    expect(() => PlainTime.fromNanosecondsOfDay(86_400_000_000_000)).toThrow("nanoseconds must be");
    expect(
      PlainTime.compare(time, {
        hour: 12,
        minute: 34,
        second: 56,
        millisecond: 7,
        microsecond: 8,
        nanosecond: 10,
      }),
    ).toBe(-1);
    expect(PlainTime.compare(time, {})).toBe(1);
    expect(
      time.equals({
        hour: 12,
        minute: 34,
        second: 56,
        millisecond: 7,
        microsecond: 8,
        nanosecond: 9,
      }),
    ).toBe(true);
    expect(time.equals({})).toBe(false);
  });
});

describe("PlainDateTime", () => {
  it("combines a date and a time", () => {
    const value = new PlainDateTime(2024, 3, 10, 2, 30);
    expect(value.toString()).toBe("2024-03-10T02:30:00");
    expect(PlainDateTime.from(value)).toBe(value);
    expect(
      PlainDateTime.from({ year: 2024, month: 3, day: 10, hour: 2, minute: 30 }).equals(value),
    ).toBe(true);
    expect(
      PlainDateTime.of(new PlainDate(2024, 3, 10), { hour: 2, minute: 30 }).equals(value),
    ).toBe(true);
    expect(PlainDateTime.of({ year: 2024, month: 3, day: 10 }).toString()).toBe(
      "2024-03-10T00:00:00",
    );
    expect(value.toPlainDate()).toEqual(new PlainDate(2024, 3, 10));
    expect(value.toPlainTime()).toEqual(new PlainTime(2, 30));
    expect(() => new PlainDateTime(2024, 2, 30)).toThrow("day must be an integer");
    expect(new PlainDateTime(-271821, 4, 19, 0, 0, 0, 0, 0, 1).toJSON()).toBe(
      "-271821-04-19T00:00:00.000000001",
    );
  });

  it("maps to and from nanoseconds since the epoch, before it too", () => {
    const value = new PlainDateTime(2000, 1, 1, 0, 0, 0, 0, 0, 1);
    expect(value.epochNanoseconds).toBe(946_684_800_000_000_001n);
    expect(PlainDateTime.fromEpochNanoseconds(946_684_800_000_000_001n).equals(value)).toBe(true);
    expect(PlainDateTime.fromEpochNanoseconds(-1n).toString()).toBe(
      "1969-12-31T23:59:59.999999999",
    );
    expect(PlainDateTime.fromEpochNanoseconds(-86_400_000_000_000n).toString()).toBe(
      "1969-12-31T00:00:00",
    );
    expect(PlainDateTime.fromEpochNanoseconds(0n).toString()).toBe("1970-01-01T00:00:00");
    expect(PlainDateTime.compare(value, { year: 2000, month: 1, day: 1 })).toBe(1);
    expect(PlainDateTime.compare({ year: 1999, month: 12, day: 31 }, value)).toBe(-1);
  });
});

describe("Instant", () => {
  it("holds epoch nanoseconds within the engine's range", () => {
    expect(new Instant(0n).toString()).toBe("1970-01-01T00:00:00Z");
    expect(Instant.from(946_684_800_000_000_000n).toJSON()).toBe("2000-01-01T00:00:00Z");
    expect(Instant.from({ epochNanoseconds: 1_500_000_000n }).toString()).toBe(
      "1970-01-01T00:00:01.5Z",
    );
    const instant = new Instant(-1n);
    expect(Instant.from(instant)).toBe(instant);
    expect(instant.epochSeconds).toBe(-1);
    expect(instant.epochMilliseconds).toBe(-1);
    expect(instant.nanosecond).toBe(999_999_999);
    expect(instant.toString()).toBe("1969-12-31T23:59:59.999999999Z");
    expect(Instant.fromEpochMilliseconds(-1).epochNanoseconds).toBe(-1_000_000n);
    expect(() => Instant.fromEpochMilliseconds(0.5)).toThrow(
      "Epoch milliseconds must be an integer",
    );
    expect(() => new Instant(1 as unknown as bigint)).toThrow("Instants must be within");
    expect(() => new Instant(-31_619_087_596_801_000_000_000n)).toThrow("Instants must be within");
    expect(new Instant(-31_619_087_596_800_000_000_000n).toString()).toBe(
      "-999999-01-01T00:00:00Z",
    );
    expect(new Instant(31_494_784_780_799_999_999_999n).toString()).toBe(
      "+999999-12-31T23:59:59.999999999Z",
    );
    expect(() => new Instant(31_494_784_780_800_000_000_000n)).toThrow("Instants must be within");
    expect(Instant.compare(instant, { epochNanoseconds: 0n })).toBe(-1);
    expect(instant.equals({ epochNanoseconds: -1n })).toBe(true);
  });
});

const newYorkGap = {
  year: 2024,
  month: 3,
  day: 10,
  hour: 2,
  minute: 30,
  timeZone: "America/New_York",
  offset: "-04:00",
  kind: "gap" as const,
  fold: false,
  offsetBefore: "-05:00",
  offsetAfter: "-04:00",
  epochNanoseconds: 1_710_055_800_000_000_000n,
};

describe("ZonedDateTime", () => {
  it("prints the civil time, offset, zone and transition annotation", () => {
    const gap = new ZonedDateTime(newYorkGap);
    expect(gap.toString()).toBe("2024-03-10T02:30:00-04:00[America/New_York][gap]");
    expect(ZonedDateTime.from(gap)).toBe(gap);
    expect(ZonedDateTime.from({ ...newYorkGap }).equals(gap)).toBe(true);
    expect(gap.offsetNanoseconds).toBe(-14_400_000_000_000);
    expect(gap.epochMilliseconds).toBe(1_710_055_800_000);
    expect(gap.toInstant().toString()).toBe("2024-03-10T07:30:00Z");
    expect(gap.toPlainDateTime().toString()).toBe("2024-03-10T02:30:00");
    expect(gap.toPlainDate().toString()).toBe("2024-03-10");
    expect(gap.toPlainTime().toString()).toBe("02:30:00");
    const fold = new ZonedDateTime({
      ...newYorkGap,
      month: 11,
      day: 3,
      hour: 1,
      offset: "-05:00",
      kind: "fold",
      fold: true,
      offsetBefore: "-04:00",
      offsetAfter: "-05:00",
      epochNanoseconds: 1_730_615_400_000_000_000n,
    });
    expect(fold.toJSON()).toBe("2024-11-03T01:30:00-05:00[America/New_York][fold=1]");
    expect(fold.equals(gap)).toBe(false);
    const first = new ZonedDateTime({ ...newYorkGap, kind: "fold", fold: false });
    expect(first.toString()).toBe("2024-03-10T02:30:00-04:00[America/New_York][fold=0]");
    const unique = new ZonedDateTime({
      year: 2000,
      month: 1,
      day: 1,
      timeZone: "UTC",
      offset: "+00:00",
      kind: "unique",
      fold: false,
      offsetBefore: "+00:00",
      offsetAfter: "+00:00",
      epochNanoseconds: 946_684_800_000_000_000n,
    });
    expect(unique.toString()).toBe("2000-01-01T00:00:00+00:00[UTC]");
    expect(unique.equals({ ...unique, timeZone: "Etc/UTC" })).toBe(false);
    expect(unique.equals({ ...unique, epochNanoseconds: 1n })).toBe(false);
  });

  it("validates the zone, the offsets and the kind", () => {
    expect(() => new ZonedDateTime({ ...newYorkGap, timeZone: "" })).toThrow("timeZone must be");
    expect(() => new ZonedDateTime({ ...newYorkGap, kind: "skip" as "gap" })).toThrow(
      "kind must be",
    );
    expect(() => new ZonedDateTime({ ...newYorkGap, offset: "-4:00" })).toThrow(
      "UTC offsets must be formatted",
    );
    expect(() => new ZonedDateTime({ ...newYorkGap, offsetBefore: "-04:60" })).toThrow(
      "within ±25:59:59",
    );
    expect(() => new ZonedDateTime({ ...newYorkGap, offsetAfter: "+26:00" })).toThrow(
      "within ±25:59:59",
    );
    expect(() => new ZonedDateTime({ ...newYorkGap, day: 32 })).toThrow("day must be");
  });
});

describe("UTC offset formatting", () => {
  it("prints seconds only when needed and parses both forms", () => {
    expect(formatUtcOffset(0)).toBe("+00:00");
    expect(formatUtcOffset(19_800)).toBe("+05:30");
    expect(formatUtcOffset(-2_670)).toBe("-00:44:30");
    expect(formatUtcOffset(93_599)).toBe("+25:59:59");
    expect(() => formatUtcOffset(93_600)).toThrow("offset must be an integer");
    expect(parseUtcOffset("+05:30")).toBe(19_800);
    expect(parseUtcOffset("-00:44:30")).toBe(-2_670);
    expect(parseUtcOffset("+00:00:59")).toBe(59);
    expect(() => parseUtcOffset("+00:00:60")).toThrow("within ±25:59:59");
    expect(() => parseUtcOffset("Z")).toThrow("formatted as ±HH:MM or ±HH:MM:SS");
  });
});

describe("Duration", () => {
  it("validates integer units of one sign within Temporal's limits", () => {
    expect(new Duration().blank).toBe(true);
    expect(() => new Duration({ hours: 1, minutes: -30 })).toThrow("same sign");
    expect(() => new Duration({ years: 2 ** 32 })).toThrow("years must be an integer");
    expect(() => new Duration({ weeks: -(2 ** 32) })).toThrow("weeks must be an integer");
    expect(() => new Duration({ seconds: 2 ** 53 })).toThrow("seconds must be an integer");
    expect(() => new Duration({ days: 1.5 })).toThrow("days must be an integer");
    expect(new Duration({ days: -0 }).days).toBe(0);
    expect(Object.is(new Duration({ days: -0 }).days, -0)).toBe(false);
    expect(DURATION_UNITS).toHaveLength(10);
  });

  it("knows its sign and negates", () => {
    const duration = new Duration({ hours: -1, minutes: -30 });
    expect(duration.sign).toBe(-1);
    expect(new Duration({ weeks: 2 }).sign).toBe(1);
    expect(new Duration().sign).toBe(0);
    expect(duration.negated().equals({ hours: 1, minutes: 30 })).toBe(true);
    expect(duration.abs().equals({ hours: 1, minutes: 30 })).toBe(true);
    const positive = new Duration({ hours: 1 });
    expect(positive.abs()).toBe(positive);
    expect(Duration.from(positive)).toBe(positive);
    expect(Duration.from({ hours: 1 }).equals(positive)).toBe(true);
    expect(positive.equals({ hours: 1, minutes: 1 })).toBe(false);
  });

  it("prints ISO 8601 the way Temporal does", () => {
    expect(new Duration().toString()).toBe("PT0S");
    expect(new Duration({ years: 1, months: 2, days: 3, hours: 4 }).toString()).toBe("P1Y2M3DT4H");
    expect(new Duration({ weeks: 2 }).toJSON()).toBe("P2W");
    expect(
      new Duration({ hours: -1, minutes: -30, seconds: -5, milliseconds: -500 }).toString(),
    ).toBe("-PT1H30M5.5S");
    expect(new Duration({ seconds: 5, milliseconds: 1500 }).toString()).toBe("PT6.5S");
    expect(new Duration({ microseconds: 1, nanoseconds: 500 }).toString()).toBe("PT0.0000015S");
    expect(new Duration({ nanoseconds: 1 }).toString()).toBe("PT0.000000001S");
    expect(new Duration({ days: 1, seconds: 0 }).toString()).toBe("P1D");
    expect(new Duration({ minutes: 90 }).toString()).toBe("PT90M");
  });
});

describe.skipIf(typeof Temporal === "undefined")("Temporal compatibility", () => {
  it("accepts every value class as a property bag", () => {
    expect(Temporal.PlainDate.from(new PlainDate(-5, 1, 1)).toString()).toBe("-000005-01-01");
    expect(Temporal.PlainTime.from(new PlainTime(2, 30, 0, 500)).toString()).toBe("02:30:00.5");
    const datetime = new PlainDateTime(275760, 9, 13, 23, 59, 59, 999, 999, 999);
    expect(Temporal.PlainDateTime.from(datetime).toString()).toBe(datetime.toString());
    expect(
      Temporal.Instant.fromEpochNanoseconds(new Instant(-1n).epochNanoseconds).toString(),
    ).toBe(new Instant(-1n).toString());
    const gap = new ZonedDateTime(newYorkGap);
    const converted = Temporal.ZonedDateTime.from(gap, { offset: "use" });
    expect(converted.toString()).toBe("2024-03-10T01:30:00-05:00[America/New_York]");
    expect(converted.epochNanoseconds).toBe(gap.epochNanoseconds - 3_600_000_000_000n);
    const exact = new Temporal.ZonedDateTime(gap.epochNanoseconds, gap.timeZone);
    expect(exact.toString()).toBe("2024-03-10T03:30:00-04:00[America/New_York]");
    const fold = new ZonedDateTime({
      ...newYorkGap,
      month: 11,
      day: 3,
      hour: 1,
      offset: "-05:00",
      kind: "fold",
      fold: true,
      offsetBefore: "-04:00",
      offsetAfter: "-05:00",
      epochNanoseconds: 1_730_615_400_000_000_000n,
    });
    expect(Temporal.ZonedDateTime.from(fold, { offset: "use" }).epochNanoseconds).toBe(
      fold.epochNanoseconds,
    );
    for (const like of [
      { years: 1, months: 2, days: 3, hours: 4 },
      { hours: -1, minutes: -30, seconds: -5, milliseconds: -500 },
      { seconds: 5, milliseconds: 1500 },
      { microseconds: 1, nanoseconds: 500 },
      { seconds: 0 },
    ]) {
      expect(Temporal.Duration.from(like).toString()).toBe(new Duration(like).toString());
    }
  });
});

describe("host time zones", () => {
  it("caches one formatter per zone and rejects names the host lacks", () => {
    expect(hostZoneFormatter("America/New_York")).toBe(hostZoneFormatter("America/New_York"));
    expect(hostKnowsZone("Asia/Calcutta")).toBe(true);
    expect(hostKnowsZone("Factory")).toBe(false);
    expect(hostKnowsZone("Nowhere/Nonexistent")).toBe(false);
    expect(() => hostOffsetSeconds("Nowhere/Nonexistent", 0)).toThrow("not known to this host");
  });

  it("reads the host's offset at an instant, with seconds, or nothing beyond its range", () => {
    expect(hostOffsetSeconds("UTC", 0)).toBe(0);
    expect(hostOffsetSeconds("America/New_York", 1_710_055_800_000)).toBe(-14_400);
    expect(hostOffsetSeconds("America/New_York", 1_710_055_800_000 - 3_600_000)).toBe(-18_000);
    expect(hostOffsetSeconds("Asia/Kolkata", 0)).toBe(19_800);
    expect(hostOffsetSeconds("Africa/Monrovia", 0)).toBe(-2_670);
    expect(hostOffsetSeconds("UTC", 8.64e15 + 1)).toBeNull();
    expect(hostOffsetSeconds("UTC", -8.64e15 - 1)).toBeNull();
  });

  it("reads a bare GMT, as some hosts print a zero offset, as zero", () => {
    const parts = vi
      .spyOn(Intl.DateTimeFormat.prototype, "formatToParts")
      .mockReturnValue([{ type: "timeZoneName", value: "GMT" }]);
    try {
      expect(hostOffsetSeconds("Europe/London", 0)).toBe(0);
    } finally {
      parts.mockRestore();
    }
  });

  it("ships the engine's zone list", () => {
    expect(TZDB_VERSION).toMatch(/^\d{4}[a-z]$/);
    expect(TIMEZONE_NAMES).toContain("UTC");
    expect(TIMEZONE_NAMES).toContain("Asia/Calcutta");
    expect(new Set(TIMEZONE_NAMES).size).toBe(TIMEZONE_NAMES.length);
  });
});
