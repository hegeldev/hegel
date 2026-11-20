/**
 * Zone sources: the host, bundled and custom sources, their uploads to the
 * real engine, the oracle probe, and the host's `Temporal` and `Intl` paths.
 */

import { describe, test, expect, vi, afterEach } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import { PlainDateTime, type ZonedDateTime } from "../src/temporal.js";
import { hostZoneData, probeOracle } from "../src/zoneSource.js";
import { TIMEZONE_NAMES } from "../src/timezone-names.js";
import {
  hostKnowsZone,
  hostTemporal,
  hostTzdbVersion,
  temporalOracle,
  type TemporalLike,
  type TemporalZonedDateTimeLike,
} from "../src/hostZones.js";
import { NativeZoneAfter } from "../src/engine.js";
import { tzif } from "./tzif.js";

const { ZoneSource } = gs.temporal;

afterEach(() => {
  vi.unstubAllGlobals();
  ZoneSource.setDefault(ZoneSource.host());
});

function check<T>(
  generator: gs.PrintableGenerator<T>,
  body: (value: T) => void,
  testCases = 30,
): void {
  hegel.test((tc) => body(tc.draw(generator)), { testCases, database: hegel.Database.disabled });
}

function drawError(generator: gs.PrintableGenerator<unknown>): () => void {
  return () =>
    hegel.test((tc) => void tc.draw(generator), {
      testCases: 5,
      database: hegel.Database.disabled,
    });
}

function at(
  zones: gs.Generator<string>,
  value: PlainDateTime,
  source?: gs.temporal.ZoneSource,
): gs.PrintableGenerator<ZonedDateTime> {
  return gs.temporal.zonedDatetimes({ zones, source, minValue: value, maxValue: value });
}

const cet = { offsetSeconds: 3600, abbreviation: "CET" };
const cest = { offsetSeconds: 7200, isDst: true, abbreviation: "CEST" };
const europe = {
  standard: cet,
  dst: {
    type: cest,
    start: { month: 3, week: 5, weekday: 0 },
    end: { month: 10, week: 5, weekday: 0, time: 10800 },
  },
};
const springForward = new PlainDateTime(2024, 3, 31, 2, 30);
const summer = new PlainDateTime(2031, 7, 1, 12);
const winter = new PlainDateTime(2031, 1, 1, 12);

describe("ZoneSource", () => {
  test("the host source is the default until another is set, which returns the one it replaces", () => {
    expect(ZoneSource.default()).toBe(ZoneSource.host());
    const previous = ZoneSource.setDefault(ZoneSource.bundled());
    expect(previous).toBe(ZoneSource.host());
    expect(ZoneSource.default()).toBe(ZoneSource.bundled());
    expect(ZoneSource.setDefault(previous)).toBe(ZoneSource.bundled());
    expect(ZoneSource.default()).toBe(ZoneSource.host());
    expect(() => ZoneSource.setDefault("bundled" as never)).toThrow(
      "setDefault needs a ZoneSource, got bundled",
    );
  });

  test("a generator reads the default when it is built and keeps it", () => {
    const paris = { data: { initial: { offsetSeconds: 60 } } };
    const toy = ZoneSource.custom({ zones: { "Europe/Paris": paris } });
    const builtBefore = {
      zones: gs.temporal.zones(),
      zoned: gs.temporal.zonedDatetimes(),
      named: at(gs.just("Europe/Paris"), winter),
    };
    ZoneSource.setDefault(toy);
    const builtAfter = {
      zones: gs.temporal.zones(),
      zoned: gs.temporal.zonedDatetimes(),
      named: at(gs.just("Europe/Paris"), winter),
    };
    ZoneSource.setDefault(ZoneSource.bundled());
    const seen = new Set<string>();
    check(builtBefore.zones, (zone) => void seen.add(zone), 50);
    check(builtBefore.zoned, (value) => void seen.add(value.timeZone), 50);
    expect(seen.size).toBeGreaterThan(5);
    check(builtBefore.named, (value) => expect(value.offset).toBe("+01:00"), 5);
    check(builtAfter.zones, (zone) => expect(zone).toBe("Europe/Paris"), 5);
    check(builtAfter.zoned, (value) => expect(value.offset).toBe("+00:01"), 5);
    check(builtAfter.named, (value) => expect(value.offset).toBe("+00:01"), 5);
  });

  test("reports versions", () => {
    expect(ZoneSource.bundled().version).toBe(gs.temporal.tzdbVersion());
    expect(ZoneSource.host().version).toBe(hostTzdbVersion());
    expect(ZoneSource.custom({ version: "2099a" }).version).toBe("2099a");
    expect(ZoneSource.custom().version).toBe("");
    expect(() => ZoneSource.custom({ version: 5 as never })).toThrow("version must be a string");
    expect(hostTzdbVersion({})).toBe("");
    expect(hostTzdbVersion({ process: { versions: { tz: "2030z" } } })).toBe("2030z");
  });

  test("zones() checks its source and names against it", () => {
    expect(() => gs.temporal.zones({ source: "host" as never })).toThrow(
      "source must be a ZoneSource, got host",
    );
    expect(() => gs.temporal.zones({ names: ["Mars/Base"], source: ZoneSource.bundled() })).toThrow(
      "The time zone Mars/Base is not known to the zone source in use (the bundled database, limited to the names this host knows): pass the zone source that has it as source",
    );
    expect(() =>
      gs.temporal.zones({ names: ["Mars/Base"], source: ZoneSource.custom({ names: ["X/Y"] }) }),
    ).toThrow("The time zone Mars/Base is not known to the zone source in use (a custom source)");
  });

  test("the bundled source draws bundled names the host knows and resolves them", () => {
    const zones = gs.temporal.zones({ source: ZoneSource.bundled() });
    for (const generator of [
      gs.temporal.zonedDatetimes({ zones }),
      gs.temporal.zonedDatetimes({ source: ZoneSource.bundled() }),
    ]) {
      check(generator, (value) => {
        expect(TIMEZONE_NAMES).toContain(value.timeZone);
        expect(hostKnowsZone(value.timeZone)).toBe(true);
      });
    }
    check(
      at(
        gs.temporal.zones({ names: ["Europe/Berlin"], source: ZoneSource.bundled() }),
        springForward,
      ),
      (value) => expect(value.kind).toBe("gap"),
    );
  });

  test("the host source draws a zone when named", () => {
    check(at(gs.temporal.zones({ names: ["Europe/Paris"] }), springForward), (value) => {
      expect(value.kind).toBe("gap");
      expect(value.offsetBefore).toBe("+01:00");
      expect(value.offsetAfter).toBe("+02:00");
    });
  });

  test("the host source draws a name only Intl knows when named", () => {
    const spelling = "europe/paris";
    expect(TIMEZONE_NAMES).not.toContain(spelling);
    const zones = gs.temporal.zones({ names: [spelling, "UTC"] });
    const seen = new Set<string>();
    check(zones, (zone) => void seen.add(zone), 40);
    expect(seen).toEqual(new Set([spelling, "UTC"]));
    check(at(gs.temporal.zones({ names: [spelling] }), winter), (value) => {
      expect(value.timeZone).toBe(spelling);
      expect(value.offset).toBe("+01:00");
    });
    check(at(gs.just("asia/tokyo"), winter), (value) => expect(value.offset).toBe("+09:00"));
  });
});

describe("the zone source of zonedDatetimes()", () => {
  const toy = ZoneSource.custom({
    zones: {
      "Europe/Paris": { data: { initial: { offsetSeconds: 60 } } },
      "Test/Fixed": { data: { initial: { offsetSeconds: -12_345 } } },
    },
  });
  const wrapped = gs.temporal.zones({ source: toy }).filter((zone) => zone.length > 0);

  test("draws the default zones() from source", () => {
    const offsets = new Set<string>();
    check(gs.temporal.zonedDatetimes({ source: toy }), (value) => void offsets.add(value.offset));
    expect(offsets).toEqual(new Set(["+00:01", "-03:25:45"]));
  });

  test("resolves the names of a wrapped zones() generator against source", () => {
    const offsets = new Set<string>();
    check(at(wrapped, winter, toy), (value) => void offsets.add(value.offset));
    expect(offsets).toEqual(new Set(["+00:01", "-03:25:45"]));
    check(at(gs.sampledFrom(["Europe/Paris"]), winter, toy), (value) =>
      expect(value.offset).toBe("+00:01"),
    );
    check(
      at(gs.oneOf(gs.temporal.zones({ source: toy }), gs.just("Test/Fixed")), winter, toy),
      (value) => expect(["+00:01", "-03:25:45"]).toContain(value.offset),
    );
  });

  test("resolves them against the default when given no source", () => {
    check(at(gs.just("Europe/Paris"), winter), (value) => expect(value.offset).toBe("+01:00"));
    expect(
      drawError(
        at(
          wrapped.filter((zone) => zone === "Test/Fixed"),
          winter,
        ),
      ),
    ).toThrow(
      "The time zone Test/Fixed is not known to the zone source in use (this host's database): pass the zone source that has it as source",
    );
  });

  test("raises for a name its source lacks", () => {
    expect(drawError(at(gs.just("Europe/London"), winter, toy))).toThrow(
      "The time zone Europe/London is not known to the zone source in use (a custom source): pass the zone source that has it as source",
    );
    expect(drawError(at(gs.just("Mars/Base"), winter, ZoneSource.bundled()))).toThrow(
      "The time zone Mars/Base is not known to the zone source in use (the bundled database",
    );
  });

  test("refuses a source beside a zones() generator, which has its own", () => {
    expect(() =>
      gs.temporal.zonedDatetimes({ zones: gs.temporal.zones({ source: toy }), source: toy }),
    ).toThrow(
      "zonedDatetimes(): zones is a zones() generator, which has its own zone source: pass source to zones() instead",
    );
    check(
      at(gs.temporal.zones({ source: toy, names: ["Test/Fixed"] }), winter),
      (value) => expect(value.offset).toBe("-03:25:45"),
      5,
    );
  });
});

describe("custom zone sources", () => {
  test("draw against structured data with a rule, without checking the host", () => {
    const source = ZoneSource.custom({
      zones: {
        "Test/Rule": { data: { initial: cet, rule: europe } },
        "Test/Days": {
          data: {
            initial: cet,
            transitions: [{ instant: 0, type: cet }],
            rule: {
              standard: cet,
              dst: { type: cest, start: { julianDay: 90 }, end: { dayOfYear: 300, time: 0 } },
            },
          },
        },
        "Test/Fixed": { data: { initial: { offsetSeconds: -12_345 } } },
      },
    });
    const zones = (name: string) => gs.temporal.zones({ names: [name], source });
    check(at(zones("Test/Rule"), springForward), (value) => {
      expect(value.timeZone).toBe("Test/Rule");
      expect(value.kind).toBe("gap");
    });
    check(at(zones("Test/Days"), summer), (value) => expect(value.offset).toBe("+02:00"));
    check(at(zones("Test/Days"), winter), (value) => expect(value.offset).toBe("+01:00"));
    check(gs.temporal.zonedDatetimes({ zones: gs.temporal.zones({ source }) }), (value) =>
      expect(["Test/Rule", "Test/Days", "Test/Fixed"]).toContain(value.timeZone),
    );
    check(at(zones("Test/Fixed"), winter), (value) => expect(value.offset).toBe("-03:25:45"));
  });

  test("draw and print an offset of more than a day", () => {
    const source = ZoneSource.custom({
      zones: { "Test/Far": { data: { initial: { offsetSeconds: 108_000 } } } },
    });
    check(at(gs.temporal.zones({ source }), winter), (value) => {
      expect(value.offset).toBe("+30:00");
      expect(value.offsetNanoseconds).toBe(108_000e9);
      expect(value.toString()).toBe("2031-01-01T12:00:00+30:00[Test/Far]");
      expect(value.toInstant().toString()).toBe("2030-12-31T06:00:00Z");
    });
  });

  test("infer a rule from the last two years of structured data", () => {
    const transitions = [
      [1_679_792_400, cest],
      [1_698_541_200, cet],
      [1_711_846_800, cest],
      [1_729_990_800, cet],
    ] as const;
    const source = ZoneSource.custom({
      zones: {
        "Test/Infer": {
          data: {
            initial: cet,
            transitions: transitions.map(([instant, type]) => ({ instant: BigInt(instant), type })),
            inferAfter: 2024,
          },
        },
      },
    });
    const zones = gs.temporal.zones({ names: ["Test/Infer"], source });
    check(at(zones, summer), (value) => expect(value.offset).toBe("+02:00"));
    check(at(zones, winter), (value) => expect(value.offset).toBe("+01:00"));
  });

  test("read TZif files", () => {
    const bytes = tzif(
      [
        { utoff: 3600, isDst: false, abbreviation: "CET" },
        { utoff: 7200, isDst: true, abbreviation: "CEST" },
      ],
      [
        [1_711_846_800, 1],
        [1_729_990_800, 0],
      ],
      "CET-1CEST,M3.5.0,M10.5.0/3",
    );
    const source = ZoneSource.custom({ zones: { "Test/Tzif": { tzif: bytes } } });
    const zones = gs.temporal.zones({ names: ["Test/Tzif"], source });
    check(at(zones, springForward), (value) => expect(value.kind).toBe("gap"));
    check(at(zones, summer), (value) => expect(value.offset).toBe("+02:00"));
    const broken = ZoneSource.custom({ zones: { "Test/Broken": { tzif: new Uint8Array([1]) } } });
    expect(
      drawError(at(gs.temporal.zones({ names: ["Test/Broken"], source: broken }), winter)),
    ).toThrow("hegel_zone_source_add_tzif failed");
  });

  test("follow links, and filter by the catalogue's link and legacy flags", () => {
    const fixed = { data: { initial: { offsetSeconds: 19_800 } } };
    const source = ZoneSource.custom({
      zones: {
        "Asia/Kolkata": fixed,
        "Asia/Calcutta": { link: "Asia/Kolkata" },
        "US/Eastern": fixed,
      },
    });
    check(at(gs.temporal.zones({ names: ["Asia/Calcutta"], source }), winter), (value) => {
      expect(value.timeZone).toBe("Asia/Calcutta");
      expect(value.offset).toBe("+05:30");
    });
    const drawn = new Set<string>();
    check(gs.temporal.zones({ source, allowLinks: false }), (zone) => void drawn.add(zone), 50);
    expect([...drawn]).toEqual(["Asia/Kolkata"]);
    check(gs.temporal.zones({ source, allowLegacy: true }), (zone) => void drawn.add(zone), 100);
    expect(drawn).toEqual(new Set(["Asia/Kolkata", "Asia/Calcutta", "US/Eastern"]));
  });

  test("load each zone's data once, the first time a draw needs it", () => {
    const load = vi.fn((name: string) =>
      name === "Lazy/Known" || name === "Lazy/Target"
        ? { data: { initial: { offsetSeconds: 600 } } }
        : name === "Lazy/Link"
          ? { link: "Lazy/Target" }
          : undefined,
    );
    const source = ZoneSource.custom({ names: ["Lazy/Known"], load });
    check(at(gs.temporal.zones({ names: ["Lazy/Known"], source }), winter), (value) =>
      expect(value.offset).toBe("+00:10"),
    );
    check(gs.temporal.zones({ names: ["Lazy/Known", "Lazy/Link"], source }), (zone) =>
      expect(["Lazy/Known", "Lazy/Link"]).toContain(zone),
    );
    check(at(gs.temporal.zones({ names: ["Lazy/Link"], source }), winter), (value) =>
      expect(value.offset).toBe("+00:10"),
    );
    expect(load.mock.calls.map(([name]) => name).sort()).toEqual([
      "Lazy/Known",
      "Lazy/Link",
      "Lazy/Target",
    ]);
    const missing = ZoneSource.custom({ names: ["Lazy/Missing"], load });
    const zones = gs.temporal.zones({ source: missing });
    for (let attempt = 0; attempt < 2; attempt++) {
      expect(drawError(gs.temporal.zonedDatetimes({ zones }))).toThrow(
        "The time zone Lazy/Missing is not known to the zone source in use (a custom source)",
      );
    }
    expect(load.mock.calls.filter(([name]) => name === "Lazy/Missing")).toHaveLength(1);
  });

  test("probe oracles", () => {
    const step = { offsetAt: (s: number) => (s < 1e9 ? 0 : 3600) };
    const source = ZoneSource.custom({
      zones: {
        "Oracle/Exact": {
          oracle: {
            ...step,
            nextTransition: (s: number) => (s < 1e9 ? 1e9 : null),
            firstYear: 1990,
            lastYear: 2010,
            after: "hold",
          },
        },
        "Oracle/Sampled": { oracle: step },
      },
    });
    const zones = (name: string) => gs.temporal.zones({ names: [name], source });
    for (const name of ["Oracle/Exact", "Oracle/Sampled"]) {
      check(at(zones(name), winter), (value) => expect(value.offset).toBe("+01:00"), 5);
      check(
        at(zones(name), new PlainDateTime(1980, 1, 1)),
        (value) => expect(value.offset).toBe("+00:00"),
        5,
      );
    }
  });

  test("report malformed definitions and data when drawn", () => {
    const draw = (definition: unknown) => {
      const source = ZoneSource.custom({ zones: { "Bad/Zone": definition as never } });
      return drawError(at(gs.temporal.zones({ names: ["Bad/Zone"], source }), winter));
    };
    expect(draw({})).toThrow("A zone is defined by tzif, data, oracle or link");
    expect(draw({ link: 5 })).toThrow("A zone is defined by tzif, data, oracle or link");
    expect(draw({ tzif: [1, 2] })).toThrow("tzif must be a Uint8Array");
    expect(draw({ data: { initial: cet, rule: europe, inferAfter: 2024 } })).toThrow(
      "takes a rule or inferAfter, not both",
    );
    expect(draw({ data: { initial: { offsetSeconds: 0.5 } } })).toThrow(
      "offsetSeconds must be an integer",
    );
    expect(draw({ data: { initial: { offsetSeconds: 0, abbreviation: 1 } } })).toThrow(
      "abbreviation must be a string",
    );
    expect(draw({ data: { initial: { offsetSeconds: 200_000 } } })).toThrow(
      "hegel_zone_source_add_zone failed",
    );
  });
});

describe("probeOracle", () => {
  test("validates the oracle", () => {
    const offsetAt = () => 0;
    expect(() => probeOracle({ offsetAt, after: "never" as never })).toThrow(
      'after must be "hold" or "infer"',
    );
    expect(() => probeOracle({ offsetAt, firstYear: 2000, lastYear: 2000 })).toThrow(
      "needs a probe range of two years",
    );
    expect(() => probeOracle({ offsetAt, firstYear: 2000, lastYear: 1999 })).toThrow(
      "lastYear must be an integer",
    );
    expect(() => probeOracle({ offsetAt, nextTransition: (s) => s })).toThrow("is not later");
  });

  test("records each change of type, with a cap", () => {
    const data = probeOracle({
      offsetAt: (s) => ({ offsetSeconds: s < 1e9 ? 0 : 3600, abbreviation: s < 1e9 ? "A" : "B" }),
      firstYear: 2001,
      lastYear: 2002,
    });
    expect(data).toMatchObject({
      initial: { utoff: 0, isDst: false, abbreviation: "A" },
      transitions: [{ instant: 1_000_000_000n, type: { utoff: 3600, abbreviation: "B" } }],
      after: NativeZoneAfter.INFER,
      lastYear: 2002,
    });
    expect(
      probeOracle({
        offsetAt: (s) => (s < 1e9 ? 0 : 3600),
        nextTransition: (s) => (s < 5e8 ? 5e8 : s < 1e9 ? 1e9 : null),
        firstYear: 1980,
        lastYear: 2010,
      }).transitions,
    ).toEqual([{ instant: 1_000_000_000n, type: { utoff: 3600, isDst: false, abbreviation: "" } }]);
    expect(() =>
      probeOracle({
        offsetAt: (s) => (s % 2) * 60,
        nextTransition: (s) => s + 1,
        firstYear: 2000,
        lastYear: 2000,
        after: "hold",
      }),
    ).toThrow("more than 100000 times");
  });
});

/** A `Temporal` holding one zone that moves from UTC to +01:00 at 2001-09-09T01:46:40Z. */
const toyTemporal: TemporalLike = {
  Instant: {
    fromEpochMilliseconds: (ms) => ({
      toZonedDateTimeISO: () => toyZoned(ms),
    }),
  },
  ZonedDateTime: { prototype: { getTimeZoneTransition: () => null } },
};

function toyZoned(ms: number): TemporalZonedDateTimeLike {
  return {
    offsetNanoseconds: ms < 1e12 ? 0 : 3.6e12,
    epochMilliseconds: ms,
    getTimeZoneTransition: () => (ms < 1e12 ? toyZoned(1e12) : null),
  };
}

describe("the host's database", () => {
  test("is read through Temporal where the host has it", () => {
    vi.stubGlobal("Temporal", toyTemporal);
    expect(hostZoneData("Toy/Zone").transitions).toEqual([
      { instant: 1_000_000_000n, type: { utoff: 3600, isDst: false, abbreviation: "" } },
    ]);
    expect(temporalOracle(toyTemporal, "Toy/Zone").offsetAt(0)).toBe(0);
  });

  test("is probed through Intl otherwise", () => {
    vi.stubGlobal("Temporal", undefined);
    const london = hostZoneData("Europe/London");
    expect(london.initial.utoff).toBe(-75);
    expect(london.transitions).toContainEqual({
      instant: 1_711_846_800n,
      type: { utoff: 3600, isDst: false, abbreviation: "" },
    });
    expect(london.after).toBe(NativeZoneAfter.INFER);
  });

  test("is probed through Intl when the host's Temporal cannot report transitions", () => {
    const viaIntl = (temporal: unknown) => {
      vi.stubGlobal("Temporal", temporal);
      expect(hostTemporal()).toBeUndefined();
      expect(hostZoneData("Europe/London").initial.utoff).toBe(-75);
    };
    viaIntl({ Instant: toyTemporal.Instant });
    viaIntl({ Instant: toyTemporal.Instant, ZonedDateTime: {} });
    viaIntl({ Instant: toyTemporal.Instant, ZonedDateTime: { prototype: {} } });
    vi.stubGlobal("Temporal", toyTemporal);
    expect(hostTemporal()).toBe(toyTemporal);
  });

  test.runIf(hostTemporal() !== undefined)("agrees between Temporal and Intl", () => {
    const viaTemporal = hostZoneData("America/Sao_Paulo");
    vi.stubGlobal("Temporal", undefined);
    expect(hostZoneData("America/Sao_Paulo")).toEqual(viaTemporal);
  });
});
