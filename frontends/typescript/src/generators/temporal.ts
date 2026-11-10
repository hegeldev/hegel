/**
 * Date and time generators, reached as `generators.temporal`: calendar dates
 * ({@link dates}), wall-clock times ({@link times}), datetimes without a zone
 * ({@link datetimes}), datetimes in a zone ({@link zonedDatetimes}), exact
 * moments ({@link instants}), UTC offsets ({@link offsets}), IANA time zone
 * names ({@link zones}), calendar spans ({@link spans}) and JavaScript
 * `Date`s ({@link jsDates}), plus {@link tzdbVersion}, the time zone
 * database release the engine bundles.
 *
 * ```ts
 * import * as hegel from "@hegeldev/hegel";
 * import * as gs from "@hegeldev/hegel/generators";
 *
 * hegel.test((tc) => {
 *   const date = tc.draw(gs.temporal.dates({ minValue: { year: 2000, month: 1, day: 1 } }));
 *   const zoned = tc.draw(gs.temporal.zonedDatetimes());
 * });
 * ```
 *
 * The values are the immutable classes this namespace also exports
 * ({@link PlainDate}, {@link PlainTime}, {@link PlainDateTime},
 * {@link Instant}, {@link ZonedDateTime} and {@link Duration}), shaped like
 * their `Temporal` counterparts; see each generator for the conversion. A
 * failing test's report prints them as the engine writes them, as
 * `PlainDate(2024-03-10)`, which is also what their `toString()` gives.
 *
 * Zones are drawn against a {@link ZoneSource}: the time zone database whose
 * transitions the draws aim at and whose offsets they resolve with. By default
 * that is the host's own, the one `Temporal` and `Intl` use
 * ({@link ZoneSource.host}), so a drawn value means the same to the code under
 * test as to Hegel. {@link ZoneSource.setDefault} changes the default, which a
 * generator reads when it is built, and {@link zones} and
 * {@link zonedDatetimes} take a `source` for one generator:
 * {@link ZoneSource.bundled} for the engine's bundled database, or
 * {@link ZoneSource.custom} for the database of a date library.
 *
 * An argument that no draw could satisfy is an error, thrown when the
 * generator is built: by the generator for what it can see by itself (a
 * bound of the wrong shape, bounds in the wrong order), and by the engine,
 * whose message it is, for the rest (bounds holding no value at the
 * precision, no date on the allowed weekdays, a zone source without a name).
 *
 * @packageDocumentation
 */

import { TestCase, generateRaw } from "../testCase.js";
import type { PrettyPrinter } from "../pretty.js";
import { LabelNames, labelSpec, type LabelSpec } from "../labels.js";
import {
  NativePrecision,
  NativeGranularity,
  NativeImaginary,
  NativeAmbiguous,
  NativeSpanUnit,
  NativeSpanSign,
  type NativeDate,
  type NativeTime,
  type NativeDatetime,
  type NativeTimestamp,
  type NativeSpan,
  type NativeZonedDatetime,
} from "../engine.js";
import {
  PlainDate,
  PlainTime,
  PlainDateTime,
  Instant,
  ZonedDateTime,
  Duration,
  DURATION_UNITS,
  nativeDate,
  nativeTime,
  nativeDatetime,
  nativeTimestamp,
  type PlainDateLike,
  type PlainTimeLike,
  type PlainDateTimeLike,
  type InstantLike,
  type TransitionKind,
  type DurationUnit,
} from "../temporal.js";
import { integerField, parseUtcOffset } from "../calendar.js";
import { entryEngine } from "../runtime.js";
import {
  buildSpanSpec,
  buildTimeZoneSpec,
  buildZonedSpec,
  validateTemporalSchema,
  withCleanup,
  type HandleBuilder,
} from "../generate.js";
import { ZoneSource, unknownZone, zoneSourceKnows } from "../zoneSource.js";

export {
  PlainDate,
  PlainTime,
  PlainDateTime,
  Instant,
  ZonedDateTime,
  Duration,
  DURATION_UNITS,
} from "../temporal.js";
export type {
  PlainDateLike,
  PlainTimeLike,
  PlainDateTimeLike,
  InstantLike,
  ZonedDateTimeLike,
  TransitionKind,
  DurationLike,
  DurationUnit,
} from "../temporal.js";
export { ZoneSource } from "../zoneSource.js";
export type {
  ZoneType,
  ZoneTransition,
  RuleDate,
  ZoneRule,
  ZoneData,
  ZoneOracle,
  ZoneDefinition,
  ZoneSourceOptions,
} from "../zoneSource.js";
import { Generator, PrintableGenerator, BasicGenerator } from "./core.js";

const NANOSECONDS_PER_SECOND = 1_000_000_000n;

/**
 * The unit a drawn time is a whole number of: `"nanosecond"` (the default)
 * draws any value, `"minute"` only whole minutes, and so on. Plurals are
 * accepted too.
 */
export type Precision =
  | "nanosecond"
  | "nanoseconds"
  | "microsecond"
  | "microseconds"
  | "millisecond"
  | "milliseconds"
  | "second"
  | "seconds"
  | "minute"
  | "minutes"
  | "hour"
  | "hours";

const PRECISIONS: Record<string, number> = {
  nanosecond: NativePrecision.NANOSECONDS,
  microsecond: NativePrecision.MICROSECONDS,
  millisecond: NativePrecision.MILLISECONDS,
  second: NativePrecision.SECONDS,
  minute: NativePrecision.MINUTES,
  hour: NativePrecision.HOURS,
};

function precisionCode(precision: Precision | undefined): number {
  const code = PRECISIONS[String(precision ?? "nanosecond").replace(/s$/, "")];
  if (code === undefined) {
    throw new Error(
      `precision must be one of ${Object.keys(PRECISIONS).join(", ")}, got ${String(precision)}`,
    );
  }
  return code;
}

/**
 * The bounds of `generator` in order: each one given, or its default. A
 * bound given alone that lies beyond the other's default says so.
 */
function orderedBounds<L, T>(
  generator: string,
  [minName, maxName]: readonly [string, string],
  [givenMin, givenMax]: readonly [L | undefined, L | undefined],
  [defaultMin, defaultMax]: readonly [T, T],
  from: (like: L) => T,
  compare: (a: T, b: T) => number,
  show: (value: T) => string,
): [T, T] {
  const min = givenMin === undefined ? defaultMin : from(givenMin);
  const max = givenMax === undefined ? defaultMax : from(givenMax);
  if (compare(min, max) > 0) {
    if (givenMax === undefined) {
      throw new Error(
        `${generator}: ${minName} ${show(min)} is after the default ${maxName}, ${show(max)}: pass ${maxName} as well`,
      );
    }
    if (givenMin === undefined) {
      throw new Error(
        `${generator}: ${maxName} ${show(max)} is before the default ${minName}, ${show(min)}: pass ${minName} as well`,
      );
    }
    throw new Error(`${generator}: cannot have ${maxName} < ${minName}`);
  }
  return [min, max];
}

const VALUE_BOUNDS = ["minValue", "maxValue"] as const;
const text = (value: { toString(): string }): string => value.toString();

/**
 * Build the engine's spec for `schema` and release it: the engine reports a
 * configuration no draw could satisfy.
 */
function validate(build: HandleBuilder<unknown>, schema: Record<string, unknown>): void {
  const lib = entryEngine();
  const ctx = lib.newContext();
  withCleanup(
    () => build(lib, ctx, schema).free(),
    () => lib.freeContext(ctx),
  );
}

/**
 * The generator of a `date`, `time`, `datetime`, `timestamp` or `offset`
 * schema, once the engine has checked the schema's arguments as a draw would.
 */
function checkedGenerator<T>(
  schema: Record<string, unknown>,
  parse: (raw: unknown) => T,
): PrintableGenerator<T> {
  const lib = entryEngine();
  const ctx = lib.newContext();
  withCleanup(
    () => validateTemporalSchema(lib, ctx, schema),
    () => lib.freeContext(ctx),
  );
  return new BasicGenerator(schema, parse);
}

function parseDate(raw: unknown): PlainDate {
  const date = raw as NativeDate;
  return new PlainDate(date.year, date.month, date.day);
}

function subseconds(nanosecond: number): [number, number, number] {
  return [
    Math.floor(nanosecond / 1_000_000),
    Math.floor(nanosecond / 1000) % 1000,
    nanosecond % 1000,
  ];
}

function parseTime(raw: unknown): PlainTime {
  const time = raw as NativeTime;
  return new PlainTime(time.hour, time.minute, time.second, ...subseconds(time.nanosecond));
}

function parseDatetime(raw: unknown): PlainDateTime {
  const { date, time } = raw as NativeDatetime;
  return new PlainDateTime(
    date.year,
    date.month,
    date.day,
    time.hour,
    time.minute,
    time.second,
    ...subseconds(time.nanosecond),
  );
}

function parseInstant(raw: unknown): Instant {
  const timestamp = raw as NativeTimestamp;
  return new Instant(timestamp.second * NANOSECONDS_PER_SECOND + BigInt(timestamp.nanosecond));
}

function formatOffset(seconds: number): string {
  return entryEngine().formatOffset({ seconds, known: true });
}

function parseOffset(raw: unknown): string {
  return formatOffset(raw as number);
}

const TRANSITION_KINDS: readonly TransitionKind[] = ["unique", "gap", "fold"];

function parseZonedDatetime(raw: unknown): ZonedDateTime {
  const { value, zone } = raw as { value: NativeZonedDatetime; zone: string };
  const civil = parseDatetime(value.civil);
  return new ZonedDateTime({
    year: civil.year,
    month: civil.month,
    day: civil.day,
    hour: civil.hour,
    minute: civil.minute,
    second: civil.second,
    millisecond: civil.millisecond,
    microsecond: civil.microsecond,
    nanosecond: civil.nanosecond,
    timeZone: zone,
    offset: formatOffset(value.offsetSeconds),
    kind: TRANSITION_KINDS[value.kind],
    fold: value.fold,
    offsetBefore: formatOffset(value.offsetBefore),
    offsetAfter: formatOffset(value.offsetAfter),
    epochNanoseconds: parseInstant(value.timestamp).epochNanoseconds,
  });
}

function parseDuration(raw: unknown): Duration {
  const span = raw as NativeSpan;
  const fields: Partial<Record<DurationUnit, number>> = {};
  for (const unit of DURATION_UNITS) fields[unit] = Number(span[unit]);
  return new Duration(fields);
}

/** Options for {@link dates}. */
export interface DateOptions {
  /** The earliest date, inclusive; a `Temporal.PlainDate` or `{ year, month, day }`. Defaults to `-271821-04-19`. */
  minValue?: PlainDateLike;
  /** The latest date, inclusive. Defaults to `+275760-09-13`. */
  maxValue?: PlainDateLike;
  /** Only dates on these ISO weekdays, 1 (Monday) to 7 (Sunday). */
  weekdays?: readonly number[];
  /** Only dates in these months, 1 (January) to 12 (December). */
  months?: readonly number[];
}

const TEMPORAL_MIN_DATE = new PlainDate(-271821, 4, 19);
const TEMPORAL_MAX_DATE = new PlainDate(275760, 9, 13);
const TEMPORAL_MIN_DATETIME = new PlainDateTime(-271821, 4, 19, 0, 0, 0, 0, 0, 1);
const TEMPORAL_MAX_DATETIME = new PlainDateTime(275760, 9, 13, 23, 59, 59, 999, 999, 999);
const TEMPORAL_INSTANT_LIMIT = 8_640_000_000_000_000_000_000n;

function mask(
  name: string,
  values: readonly number[] | undefined,
  lowest: number,
  highest: number,
): number | undefined {
  if (values === undefined) return undefined;
  let bits = 0;
  for (const value of values) {
    if (!Number.isInteger(value) || value < lowest || value > highest) {
      throw new Error(`${name} must be integers between ${lowest} and ${highest}, got ${value}`);
    }
    bits |= 1 << (value - lowest);
  }
  return bits;
}

/**
 * Generate calendar dates as {@link PlainDate}s, shrinking towards
 * 2000-01-01. The default range is `Temporal.PlainDate`'s, so
 * `Temporal.PlainDate.from(value)` always succeeds; bounds may reach year
 * ±999999.
 */
export function dates(options?: DateOptions): PrintableGenerator<PlainDate> {
  const [min, max] = orderedBounds(
    "dates()",
    VALUE_BOUNDS,
    [options?.minValue, options?.maxValue],
    [TEMPORAL_MIN_DATE, TEMPORAL_MAX_DATE],
    (like) => PlainDate.from(like),
    PlainDate.compare,
    text,
  );
  const weekdays = mask("weekdays", options?.weekdays, 1, 7);
  const months = mask("months", options?.months, 1, 12);
  const schema: Record<string, unknown> = {
    type: "date",
    min: nativeDate(min),
    max: nativeDate(max),
  };
  if (weekdays !== undefined) schema["weekdays"] = weekdays;
  if (months !== undefined) schema["months"] = months;
  return checkedGenerator(schema, parseDate);
}

/** Options for {@link times}. */
export interface TimeOptions {
  /** The earliest time, inclusive; a `Temporal.PlainTime` or `{ hour, minute, ... }`. Defaults to midnight. */
  minValue?: PlainTimeLike;
  /** The latest time, inclusive. Defaults to `23:59:59.999999999`. */
  maxValue?: PlainTimeLike;
  precision?: Precision;
}

const MIDNIGHT = new PlainTime();
const LAST_NANOSECOND = new PlainTime(23, 59, 59, 999, 999, 999);

/**
 * Generate wall-clock times as {@link PlainTime}s, shrinking towards
 * midnight. `Temporal.PlainTime.from(value)` converts a value.
 */
export function times(options?: TimeOptions): PrintableGenerator<PlainTime> {
  const [min, max] = orderedBounds(
    "times()",
    VALUE_BOUNDS,
    [options?.minValue, options?.maxValue],
    [MIDNIGHT, LAST_NANOSECOND],
    (like) => PlainTime.from(like),
    PlainTime.compare,
    text,
  );
  return checkedGenerator(
    {
      type: "time",
      min: nativeTime(min),
      max: nativeTime(max),
      precision: precisionCode(options?.precision),
    },
    parseTime,
  );
}

/** Options for {@link datetimes}. */
export interface DatetimeOptions {
  /** The earliest value, inclusive; a `Temporal.PlainDateTime` or its fields. Defaults to `-271821-04-19T00:00:00.000000001`. */
  minValue?: PlainDateTimeLike;
  /** The latest value, inclusive. Defaults to `+275760-09-13T23:59:59.999999999`. */
  maxValue?: PlainDateTimeLike;
  precision?: Precision;
}

/**
 * Generate wall-clock datetimes without a time zone as {@link PlainDateTime}s,
 * shrinking towards 2000-01-01T00:00:00. The default range is
 * `Temporal.PlainDateTime`'s, so `Temporal.PlainDateTime.from(value)` always
 * succeeds; bounds may reach year ±999999.
 */
export function datetimes(options?: DatetimeOptions): PrintableGenerator<PlainDateTime> {
  const [min, max] = orderedBounds(
    "datetimes()",
    VALUE_BOUNDS,
    [options?.minValue, options?.maxValue],
    [TEMPORAL_MIN_DATETIME, TEMPORAL_MAX_DATETIME],
    (like) => PlainDateTime.from(like),
    PlainDateTime.compare,
    text,
  );
  return checkedGenerator(
    {
      type: "datetime",
      min: nativeDatetime(min),
      max: nativeDatetime(max),
      precision: precisionCode(options?.precision),
    },
    parseDatetime,
  );
}

/** Options for {@link instants}. */
export interface InstantOptions {
  /** The earliest instant, inclusive; a `Temporal.Instant`, an {@link Instant} or epoch nanoseconds. Defaults to `Temporal.Instant`'s minimum. */
  minValue?: InstantLike | bigint;
  /** The latest instant, inclusive. Defaults to `Temporal.Instant`'s maximum. */
  maxValue?: InstantLike | bigint;
  precision?: Precision;
}

const TEMPORAL_MIN_INSTANT = new Instant(-TEMPORAL_INSTANT_LIMIT);
const TEMPORAL_MAX_INSTANT = new Instant(TEMPORAL_INSTANT_LIMIT);

function instantBounds(
  generator: string,
  names: readonly [string, string],
  given: readonly [InstantLike | bigint | undefined, InstantLike | bigint | undefined],
): { min: NativeTimestamp; max: NativeTimestamp } {
  const [min, max] = orderedBounds(
    generator,
    names,
    given,
    [TEMPORAL_MIN_INSTANT, TEMPORAL_MAX_INSTANT],
    (like) => Instant.from(like),
    Instant.compare,
    text,
  );
  return {
    min: nativeTimestamp(min.epochNanoseconds),
    max: nativeTimestamp(max.epochNanoseconds),
  };
}

/**
 * Generate exact moments as {@link Instant}s, shrinking towards
 * 2000-01-01T00:00:00Z. The default range is `Temporal.Instant`'s, so
 * `Temporal.Instant.fromEpochNanoseconds(value.epochNanoseconds)` always
 * succeeds; bounds may reach year ±999999.
 */
export function instants(options?: InstantOptions): PrintableGenerator<Instant> {
  return checkedGenerator(
    {
      type: "timestamp",
      ...instantBounds("instants()", VALUE_BOUNDS, [options?.minValue, options?.maxValue]),
      precision: precisionCode(options?.precision),
    },
    parseInstant,
  );
}

/** The multiples a UTC offset draw is restricted to. Plurals are accepted too. */
export type Granularity =
  | "hour"
  | "hours"
  | "quarter-hour"
  | "quarter-hours"
  | "minute"
  | "minutes"
  | "second"
  | "seconds";

const GRANULARITIES: Record<string, number> = {
  hour: NativeGranularity.HOURS,
  "quarter-hour": NativeGranularity.QUARTER_HOURS,
  minute: NativeGranularity.MINUTES,
  second: NativeGranularity.SECONDS,
};

/** Options for {@link offsets}. */
export interface OffsetOptions {
  /** The smallest offset, inclusive, as `±HH:MM[:SS]` or seconds, down to `-25:59:59`. Defaults to `-23:59`, or `-23:59:59` at second granularity. */
  minValue?: string | number;
  /** The largest offset, inclusive, up to `+25:59:59`. Defaults to `+23:59`, or `+23:59:59` at second granularity. */
  maxValue?: string | number;
  /** Which offsets are drawn: whole hours, quarter hours, minutes (the default) or seconds. */
  granularity?: Granularity;
}

const INT32_LIMIT = 0x7fffffff;

/**
 * Generate UTC offsets as `±HH:MM` strings (`±HH:MM:SS` when the offset is
 * not a whole number of minutes), shrinking towards `+00:00`. At the default
 * minute granularity or coarser the strings are valid `Temporal` time zone
 * identifiers; `Temporal` accepts a `±HH:MM:SS` offset only as the `offset`
 * of a value, not as a time zone.
 */
export function offsets(options?: OffsetOptions): PrintableGenerator<string> {
  const granularity = GRANULARITIES[String(options?.granularity ?? "minute").replace(/s$/, "")];
  if (granularity === undefined) {
    throw new Error(
      `granularity must be one of ${Object.keys(GRANULARITIES).join(", ")}, got ${String(options?.granularity)}`,
    );
  }
  const limit = granularity === NativeGranularity.SECONDS ? 86_399 : 86_340;
  const seconds =
    (name: string) =>
    (value: string | number): number =>
      typeof value === "string"
        ? parseUtcOffset(value)
        : integerField(name, value, -INT32_LIMIT, INT32_LIMIT);
  const min = options?.minValue === undefined ? -limit : seconds("minValue")(options.minValue);
  const max = options?.maxValue === undefined ? limit : seconds("maxValue")(options.maxValue);
  orderedBounds(
    "offsets()",
    VALUE_BOUNDS,
    [
      options?.minValue === undefined ? undefined : min,
      options?.maxValue === undefined ? undefined : max,
    ],
    [-limit, limit],
    (value) => value,
    (a, b) => a - b,
    formatOffset,
  );
  return checkedGenerator({ type: "offset", min, max, granularity }, parseOffset);
}

/** Options for {@link zones}. */
export interface ZoneOptions {
  /**
   * Draw only from these IANA names, each of which the zone source must know.
   * When given, `allowLinks` and `allowLegacy` are ignored.
   */
  names?: readonly string[];
  /** Include link names (aliases such as `Asia/Calcutta` for `Asia/Kolkata`). Defaults to `true`. */
  allowLinks?: boolean;
  /** Include legacy names (`EST5EDT`, `US/Eastern`, ...). Defaults to `false`. */
  allowLegacy?: boolean;
  /**
   * The database the zones come from, which a {@link zonedDatetimes} drawing
   * from this generator draws against. Defaults to {@link ZoneSource.default}
   * as it is when this generator is built.
   */
  source?: ZoneSource;
}

class ZonesGenerator extends BasicGenerator<string> {}

/** The generators of every name of a source, by their flags: built and validated once. */
const wholeSources = new WeakMap<ZoneSource, Map<string, ZonesGenerator>>();

function zoneSource(source: ZoneSource | undefined): ZoneSource {
  if (source === undefined) return ZoneSource.default();
  if (!(source instanceof ZoneSource)) {
    throw new Error(`source must be a ZoneSource, got ${String(source)}`);
  }
  return source;
}

function checkZoneNames(names: readonly string[], source: ZoneSource): string[] {
  const checked = [...new Set(names)];
  if (checked.length === 0) throw new Error("names must not be empty");
  for (const name of checked) requireZone(source, name);
  return checked;
}

function requireZone(source: ZoneSource, name: unknown): string {
  if (typeof name !== "string" || !zoneSourceKnows(source, name)) {
    throw unknownZone(source, String(name));
  }
  return name;
}

/**
 * Generate IANA time zone names (`America/New_York`), shrinking towards
 * `UTC`. Without `names`, the names drawn are the bundled names the host's
 * `Intl` knows for {@link ZoneSource.host} and {@link ZoneSource.bundled}, so
 * `Temporal.ZonedDateTime` and `Intl.DateTimeFormat` accept every value, and
 * the source's own names for a custom source.
 */
export function zones(options?: ZoneOptions): PrintableGenerator<string> {
  const source = zoneSource(options?.source);
  const allowLinks = options?.allowLinks ?? true;
  const allowLegacy = options?.allowLegacy ?? false;
  const build = (names: string[] | null): ZonesGenerator => {
    const generator = new ZonesGenerator({
      type: "timezone",
      names,
      allow_links: allowLinks,
      allow_legacy: allowLegacy,
      source,
    });
    validate(buildTimeZoneSpec, generator.schema);
    return generator;
  };
  if (options?.names !== undefined) return build(checkZoneNames(options.names, source));
  let generators = wholeSources.get(source);
  if (generators === undefined) {
    generators = new Map();
    wholeSources.set(source, generators);
  }
  const key = `${String(allowLinks)},${String(allowLegacy)}`;
  let generator = generators.get(key);
  if (generator === undefined) {
    generator = build(null);
    generators.set(key, generator);
  }
  return generator;
}

/**
 * Which occurrence of a wall-clock time a transition repeated a
 * {@link zonedDatetimes} draw means: `"both"` draws either, shrinking towards
 * the earlier; `"earlier"` and `"later"` pin one.
 */
export type Ambiguous = "both" | "earlier" | "later";

/** Options for {@link zonedDatetimes}. */
export interface ZonedDatetimeOptions {
  /**
   * Where the zone comes from. Defaults to {@link zones}`({ source })`.
   *
   * A {@link zones} generator brings its own zone source, the database the
   * value is drawn against, so `source` may not be given with one. Any other
   * generator of names (a `zones()` generator that was filtered or mapped,
   * `sampledFrom`, `oneOf`, ...) yields only names: each is resolved against
   * `source`, and one that source does not know is an error.
   */
  zones?: Generator<string>;
  /**
   * The database the zones are resolved against: the source of the default
   * {@link zones} generator, or of the names another generator yields.
   * Defaults to {@link ZoneSource.default} as it is when this generator is
   * built. An error with a {@link zones} generator as `zones`, which takes
   * the source itself.
   */
  source?: ZoneSource;
  /** The earliest wall-clock time, inclusive. Defaults to `-271821-04-22T00:00:00`. Mutually exclusive with the instant bounds. */
  minValue?: PlainDateTimeLike;
  /** The latest wall-clock time, inclusive. Defaults to `+275760-09-10T23:59:59.999999999`. */
  maxValue?: PlainDateTimeLike;
  /** The earliest instant, inclusive; mutually exclusive with the wall-clock bounds. */
  minInstant?: InstantLike | bigint;
  /** The latest instant, inclusive. */
  maxInstant?: InstantLike | bigint;
  precision?: Precision;
  /** Whether to draw wall-clock times a transition skipped (`kind: "gap"`). Defaults to `true`. */
  allowImaginary?: boolean;
  /** Which occurrence of a wall-clock time a transition repeated to draw. Defaults to `"both"`. */
  ambiguous?: Ambiguous;
}

const ZONED_MIN_DATETIME = new PlainDateTime(-271821, 4, 22);
const ZONED_MAX_DATETIME = new PlainDateTime(275760, 9, 10, 23, 59, 59, 999, 999, 999);
const AMBIGUOUS = {
  both: NativeAmbiguous.BOTH,
  earlier: NativeAmbiguous.EARLIER,
  later: NativeAmbiguous.LATER,
} as const;

/** Zoned datetimes in the zones a generator of names yields, resolved against `source`. */
class ZonedDatetimesIn extends PrintableGenerator<ZonedDateTime> {
  constructor(
    private readonly zones: Generator<string>,
    private readonly source: ZoneSource,
    private readonly bounds: Record<string, unknown>,
  ) {
    super();
  }

  doDrawAndPrint(tc: TestCase, printer: PrettyPrinter): ZonedDateTime {
    const value = parseZonedDatetime(
      generateRaw(tc, {
        type: "zoned_datetime_in_zone",
        source: this.source,
        zone: requireZone(this.source, tc.drawSilent(this.zones)),
        bounds: this.bounds,
      }),
    );
    printer.value(value);
    return value;
  }

  /** @internal */
  protected override makeLabel(): LabelSpec {
    return labelSpec(LabelNames.ZONED_DATETIME, this.zones.label);
  }
}

/**
 * Generate wall-clock times in a time zone as {@link ZonedDateTime}s,
 * shrinking towards 2000-01-01T00:00:00 in `UTC`. Transitions are explored:
 * values in a gap (`kind: "gap"`, unless `allowImaginary` is false) and in a
 * fold (`kind: "fold"`, with `fold` telling the occurrences apart) are drawn
 * alongside unambiguous ones, against a zone source (see {@link ZoneSource}):
 * the one of a {@link zones} generator given as `zones`, else `source`, else
 * the default as it is when this generator is built. Against
 * {@link ZoneSource.host} or {@link ZoneSource.bundled}, a draw whose offset
 * the host's `Intl` disagrees with is rejected like a failed assumption.
 *
 * ```ts
 * const anywhere = gs.temporal.zonedDatetimes();
 * const american = gs.temporal.zonedDatetimes({
 *   zones: gs.temporal.zones().filter((zone) => zone.startsWith("America/")),
 *   source: gs.temporal.ZoneSource.bundled(),
 * });
 * ```
 *
 * The bounds are on the wall clock (`minValue`, `maxValue`) or on the
 * instant (`minInstant`, `maxInstant`), never both. The default wall-clock
 * range lies two days inside `Temporal.PlainDateTime`'s, so
 * {@link ZonedDateTime.toTemporal} always succeeds for a zone `Temporal`
 * knows.
 */
export function zonedDatetimes(options?: ZonedDatetimeOptions): PrintableGenerator<ZonedDateTime> {
  const civilBounds = options?.minValue !== undefined || options?.maxValue !== undefined;
  const instants = options?.minInstant !== undefined || options?.maxInstant !== undefined;
  if (civilBounds && instants) {
    throw new Error(
      "zonedDatetimes() takes either wall-clock bounds (minValue, maxValue) or instant bounds (minInstant, maxInstant), not both",
    );
  }
  const ambiguous = options?.ambiguous ?? "both";
  if (!Object.hasOwn(AMBIGUOUS, ambiguous)) {
    throw new Error(`ambiguous must be "both", "earlier" or "later", got ${String(ambiguous)}`);
  }
  const bounds: Record<string, unknown> = {
    civil: null,
    instant: null,
    precision: precisionCode(options?.precision),
    imaginary: (options?.allowImaginary ?? true) ? NativeImaginary.ALLOW : NativeImaginary.AVOID,
    ambiguous: AMBIGUOUS[ambiguous],
  };
  if (instants) {
    bounds["instant"] = instantBounds(
      "zonedDatetimes()",
      ["minInstant", "maxInstant"],
      [options?.minInstant, options?.maxInstant],
    );
  } else {
    const [min, max] = orderedBounds(
      "zonedDatetimes()",
      VALUE_BOUNDS,
      [options?.minValue, options?.maxValue],
      [ZONED_MIN_DATETIME, ZONED_MAX_DATETIME],
      (like) => PlainDateTime.from(like),
      PlainDateTime.compare,
      text,
    );
    bounds["civil"] = { min: nativeDatetime(min), max: nativeDatetime(max) };
  }
  validate(buildZonedSpec, bounds);
  const names = options?.zones;
  if (names instanceof ZonesGenerator) {
    if (options?.source !== undefined) {
      throw new Error(
        "zonedDatetimes(): zones is a zones() generator, which has its own zone source: pass source to zones() instead",
      );
    }
    return new BasicGenerator(
      { type: "zoned_datetime", zones: names.schema, bounds },
      parseZonedDatetime,
    );
  }
  const source = zoneSource(options?.source);
  if (names === undefined) {
    const all = zones({ source }) as ZonesGenerator;
    return new BasicGenerator(
      { type: "zoned_datetime", zones: all.schema, bounds },
      parseZonedDatetime,
    );
  }
  return new ZonedDatetimesIn(names, source, bounds);
}

/** Inclusive bounds on one unit of a {@link spans} draw. */
export interface UnitBounds {
  minValue?: number;
  maxValue?: number;
}

/**
 * Options for {@link spans}. Naming any unit enables only the named
 * units; with none named every unit is drawn. Each unit's default bounds
 * keep `Temporal.Duration.from(value)` valid: ±4294967295 for years, months
 * and weeks, ±10¹⁰ days, ±10¹¹ hours, ±10¹³ minutes, ±10¹⁵ seconds and
 * ±2⁵³−1 for milliseconds, microseconds and nanoseconds.
 */
export interface SpanOptions {
  years?: UnitBounds;
  months?: UnitBounds;
  weeks?: UnitBounds;
  days?: UnitBounds;
  hours?: UnitBounds;
  minutes?: UnitBounds;
  seconds?: UnitBounds;
  milliseconds?: UnitBounds;
  microseconds?: UnitBounds;
  nanoseconds?: UnitBounds;
  /** `"uniform"` (the default) draws one sign for every unit; `"non-negative"` draws magnitudes only. */
  sign?: "uniform" | "non-negative";
}

const UNIT_LIMITS: Record<DurationUnit, number> = {
  years: 2 ** 32 - 1,
  months: 2 ** 32 - 1,
  weeks: 2 ** 32 - 1,
  days: 1e10,
  hours: 1e11,
  minutes: 1e13,
  seconds: 1e15,
  milliseconds: Number.MAX_SAFE_INTEGER,
  microseconds: Number.MAX_SAFE_INTEGER,
  nanoseconds: Number.MAX_SAFE_INTEGER,
};

const UNIT_CODES: Record<DurationUnit, number> = {
  years: NativeSpanUnit.YEARS,
  months: NativeSpanUnit.MONTHS,
  weeks: NativeSpanUnit.WEEKS,
  days: NativeSpanUnit.DAYS,
  hours: NativeSpanUnit.HOURS,
  minutes: NativeSpanUnit.MINUTES,
  seconds: NativeSpanUnit.SECONDS,
  milliseconds: NativeSpanUnit.MILLISECONDS,
  microseconds: NativeSpanUnit.MICROSECONDS,
  nanoseconds: NativeSpanUnit.NANOSECONDS,
};

/**
 * Generate calendar spans as {@link Duration}s, shrinking towards zero. The
 * units are kept as drawn rather than balanced (90 minutes stays 90 minutes),
 * so a span of one month is not a fixed amount of time. Every unit of a value
 * has the same sign, as `Temporal.Duration` requires, and
 * `Temporal.Duration.from(value)` converts a value.
 */
export function spans(options?: SpanOptions): PrintableGenerator<Duration> {
  const sign = options?.sign ?? "uniform";
  if (sign !== "uniform" && sign !== "non-negative") {
    throw new Error(`sign must be "uniform" or "non-negative", got ${String(sign)}`);
  }
  const named = DURATION_UNITS.filter((unit) => options?.[unit] !== undefined);
  const enabled = named.length === 0 ? DURATION_UNITS : named;
  const units = enabled.map((unit) => {
    const limit = UNIT_LIMITS[unit];
    const given = options?.[unit] ?? {};
    const [min, max] = orderedBounds(
      unit,
      VALUE_BOUNDS,
      [given.minValue, given.maxValue],
      [sign === "uniform" ? -limit : 0, limit],
      (value) => value,
      (a, b) => a - b,
      String,
    );
    return {
      unit: UNIT_CODES[unit],
      min: BigInt(integerField(`${unit}.minValue`, min, -limit, limit)),
      max: BigInt(integerField(`${unit}.maxValue`, max, -limit, limit)),
    };
  });
  const schema = {
    type: "span",
    units,
    sign: sign === "uniform" ? NativeSpanSign.UNIFORM : NativeSpanSign.NONNEGATIVE,
  };
  validate(buildSpanSpec, schema);
  return new BasicGenerator(schema, parseDuration);
}

/** Options for {@link jsDates}. */
export interface JsDateOptions {
  /** The earliest `Date`, inclusive. Defaults to the earliest `Date` there is. */
  minValue?: Date;
  /** The latest `Date`, inclusive. Defaults to the latest `Date` there is. */
  maxValue?: Date;
}

const DATE_LIMIT_MILLISECONDS = 8.64e15;

function dateMilliseconds(name: string, value: Date): number {
  if (!(value instanceof Date) || Number.isNaN(value.getTime())) {
    throw new Error(`${name} must be a valid Date`);
  }
  return value.getTime();
}

function timestampOfMilliseconds(milliseconds: number): NativeTimestamp {
  const second = Math.floor(milliseconds / 1000);
  return { second: BigInt(second), nanosecond: (milliseconds - second * 1000) * 1_000_000 };
}

function parseJsDate(raw: unknown): Date {
  const timestamp = raw as NativeTimestamp;
  return new Date(Number(timestamp.second) * 1000 + Math.floor(timestamp.nanosecond / 1_000_000));
}

/**
 * Generate JavaScript `Date`s (instants at millisecond precision), shrinking
 * towards 2000-01-01T00:00:00Z. The default range is every valid `Date`.
 */
export function jsDates(options?: JsDateOptions): PrintableGenerator<Date> {
  const min = dateMilliseconds("minValue", options?.minValue ?? new Date(-DATE_LIMIT_MILLISECONDS));
  const max = dateMilliseconds("maxValue", options?.maxValue ?? new Date(DATE_LIMIT_MILLISECONDS));
  if (min > max) throw new Error("jsDates(): cannot have maxValue < minValue");
  return new BasicGenerator(
    {
      type: "timestamp",
      min: timestampOfMilliseconds(min),
      max: timestampOfMilliseconds(max),
      precision: NativePrecision.MILLISECONDS,
    },
    parseJsDate,
  );
}

/**
 * The IANA release of the time zone database the engine bundles, such as
 * `"2026e"`: the database of {@link ZoneSource.bundled}, which is not the
 * default; see {@link ZoneSource.version} for any source's.
 */
export function tzdbVersion(): string {
  return entryEngine().tzdbVersion();
}
