/**
 * Date and time generators: calendar dates, wall-clock times, instants, UTC
 * offsets, time zones, zoned datetimes, durations and JavaScript `Date`s.
 *
 * The values are the immutable classes in the `datetime` module, shaped like
 * their `Temporal` counterparts; see each generator for the conversion.
 *
 * @packageDocumentation
 */

import { TestCase, Labels, generateRaw } from "../testCase.js";
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
  civilFromDays,
  formatUtcOffset,
  parseUtcOffset,
  type PlainDateLike,
  type PlainTimeLike,
  type PlainDateTimeLike,
  type InstantLike,
  type TransitionKind,
  type DurationUnit,
} from "../datetime.js";
import { hostKnowsZone } from "../hostZones.js";
import { Generator, BasicGenerator } from "./core.js";

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

interface PrecisionUnit {
  code: number;
  nanoseconds: bigint;
}

const PRECISIONS: Record<string, PrecisionUnit> = {
  nanosecond: { code: NativePrecision.NANOSECONDS, nanoseconds: 1n },
  microsecond: { code: NativePrecision.MICROSECONDS, nanoseconds: 1_000n },
  millisecond: { code: NativePrecision.MILLISECONDS, nanoseconds: 1_000_000n },
  second: { code: NativePrecision.SECONDS, nanoseconds: NANOSECONDS_PER_SECOND },
  minute: { code: NativePrecision.MINUTES, nanoseconds: 60n * NANOSECONDS_PER_SECOND },
  hour: { code: NativePrecision.HOURS, nanoseconds: 3600n * NANOSECONDS_PER_SECOND },
};

function precisionUnit(precision: Precision | undefined): PrecisionUnit {
  const unit = PRECISIONS[String(precision ?? "nanosecond").replace(/s$/, "")];
  if (unit === undefined) {
    throw new Error(
      `precision must be one of ${Object.keys(PRECISIONS).join(", ")}, got ${String(precision)}`,
    );
  }
  return unit;
}

/** The smallest multiple of `unit` that is at least `value`. */
function ceilTo(value: bigint, unit: bigint): bigint {
  const truncated = (value / unit) * unit;
  return truncated < value ? truncated + unit : truncated;
}

/** Throw unless some multiple of `unit` nanoseconds lies in `[min, max]`. */
function requireFit(what: string, min: bigint, max: bigint, unit: bigint, bounds: string): void {
  if (ceilTo(min, unit) > max) {
    throw new Error(`No ${what} between ${bounds} has the requested precision`);
  }
}

function requireOrdered(name: string, inverted: boolean): void {
  if (inverted) throw new Error(`${name}: cannot have maxValue < minValue`);
}

function nativeDate(date: PlainDate): NativeDate {
  return { year: date.year, month: date.month, day: date.day };
}

function nativeTime(time: PlainTime): NativeTime {
  return {
    hour: time.hour,
    minute: time.minute,
    second: time.second,
    nanosecond: time.nanosecondsOfDay % 1_000_000_000,
  };
}

function nativeDatetime(value: PlainDateTime): NativeDatetime {
  return { date: nativeDate(value.toPlainDate()), time: nativeTime(value.toPlainTime()) };
}

function nativeTimestamp(instant: Instant): NativeTimestamp {
  return { second: BigInt(instant.epochSeconds), nanosecond: instant.nanosecond };
}

function parseDate(raw: unknown): PlainDate {
  const date = raw as NativeDate;
  return new PlainDate(date.year, date.month, date.day);
}

function parseTime(raw: unknown): PlainTime {
  const time = raw as NativeTime;
  return PlainTime.fromNanosecondsOfDay(
    ((time.hour * 60 + time.minute) * 60 + time.second) * 1_000_000_000 + time.nanosecond,
  );
}

function parseDatetime(raw: unknown): PlainDateTime {
  const value = raw as NativeDatetime;
  return PlainDateTime.of(parseDate(value.date), parseTime(value.time));
}

function parseInstant(raw: unknown): Instant {
  const timestamp = raw as NativeTimestamp;
  return new Instant(timestamp.second * NANOSECONDS_PER_SECOND + BigInt(timestamp.nanosecond));
}

function parseOffset(raw: unknown): string {
  return formatUtcOffset(raw as number);
}

const TRANSITION_KINDS: readonly TransitionKind[] = ["unique", "gap", "fold"];

function parseZonedDatetime(raw: unknown): ZonedDateTime {
  const { value, zone } = raw as { value: NativeZonedDatetime; zone: string };
  return new ZonedDateTime({
    ...parseDatetime(value.civil),
    timeZone: zone,
    offset: formatUtcOffset(value.offsetSeconds),
    kind: TRANSITION_KINDS[value.kind],
    fold: value.fold,
    offsetBefore: formatUtcOffset(value.offsetBefore),
    offsetAfter: formatUtcOffset(value.offsetAfter),
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
  if (bits === 0) throw new Error(`${name} must not be empty`);
  return bits;
}

function allowedBy(bits: number | undefined, value: number): boolean {
  return bits === undefined || ((bits >> (value - 1)) & 1) === 1;
}

/** Whether `[min, max]` holds a date on an allowed weekday in an allowed month: 400 days see every combination. */
function hasAllowedDate(
  min: PlainDate,
  max: PlainDate,
  weekdays: number | undefined,
  months: number | undefined,
): boolean {
  const first = min.epochDays;
  const last = Math.min(max.epochDays, first + 400);
  for (let days = first; days <= last; days++) {
    const date = civilFromDays(days);
    if (allowedBy(weekdays, date.dayOfWeek) && allowedBy(months, date.month)) return true;
  }
  return false;
}

/**
 * Generate calendar dates as {@link PlainDate}s, shrinking towards
 * 2000-01-01. The default range is `Temporal.PlainDate`'s, so
 * `Temporal.PlainDate.from(value)` always succeeds.
 */
export function dates(options?: DateOptions): Generator<PlainDate> {
  const min = PlainDate.from(options?.minValue ?? TEMPORAL_MIN_DATE);
  const max = PlainDate.from(options?.maxValue ?? TEMPORAL_MAX_DATE);
  requireOrdered("dates()", PlainDate.compare(min, max) > 0);
  const weekdays = mask("weekdays", options?.weekdays, 1, 7);
  const months = mask("months", options?.months, 1, 12);
  if (!hasAllowedDate(min, max, weekdays, months)) {
    throw new Error(
      `dates(): no date between ${min.toString()} and ${max.toString()} is on the allowed weekdays and months`,
    );
  }
  const schema: Record<string, unknown> = {
    type: "date",
    min: nativeDate(min),
    max: nativeDate(max),
  };
  if (weekdays !== undefined) schema["weekdays"] = weekdays;
  if (months !== undefined) schema["months"] = months;
  return new BasicGenerator(schema, parseDate);
}

/** Options for {@link times}. */
export interface TimeOptions {
  /** The earliest time, inclusive; a `Temporal.PlainTime` or `{ hour, minute, ... }`. Defaults to midnight. */
  minValue?: PlainTimeLike;
  /** The latest time, inclusive. Defaults to `23:59:59.999999999`. */
  maxValue?: PlainTimeLike;
  precision?: Precision;
}

/**
 * Generate wall-clock times as {@link PlainTime}s, shrinking towards
 * midnight. `Temporal.PlainTime.from(value)` converts a value.
 */
export function times(options?: TimeOptions): Generator<PlainTime> {
  const min = PlainTime.from(options?.minValue ?? {});
  const max = PlainTime.from(
    options?.maxValue ?? {
      hour: 23,
      minute: 59,
      second: 59,
      millisecond: 999,
      microsecond: 999,
      nanosecond: 999,
    },
  );
  requireOrdered("times()", PlainTime.compare(min, max) > 0);
  const unit = precisionUnit(options?.precision);
  requireFit(
    "time",
    BigInt(min.nanosecondsOfDay),
    BigInt(max.nanosecondsOfDay),
    unit.nanoseconds,
    `${min.toString()} and ${max.toString()}`,
  );
  return new BasicGenerator(
    { type: "time", min: nativeTime(min), max: nativeTime(max), precision: unit.code },
    parseTime,
  );
}

/** Options for {@link datetimes}. */
export interface DateTimeOptions {
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
 * succeeds.
 */
export function datetimes(options?: DateTimeOptions): Generator<PlainDateTime> {
  const min = PlainDateTime.from(options?.minValue ?? TEMPORAL_MIN_DATETIME);
  const max = PlainDateTime.from(options?.maxValue ?? TEMPORAL_MAX_DATETIME);
  requireOrdered("datetimes()", PlainDateTime.compare(min, max) > 0);
  const unit = precisionUnit(options?.precision);
  requireFit(
    "datetime",
    min.epochNanoseconds,
    max.epochNanoseconds,
    unit.nanoseconds,
    `${min.toString()} and ${max.toString()}`,
  );
  return new BasicGenerator(
    { type: "datetime", min: nativeDatetime(min), max: nativeDatetime(max), precision: unit.code },
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

/**
 * Generate exact moments as {@link Instant}s, shrinking towards
 * 2000-01-01T00:00:00Z. The default range is `Temporal.Instant`'s, so
 * `Temporal.Instant.fromEpochNanoseconds(value.epochNanoseconds)` always
 * succeeds.
 */
export function instants(options?: InstantOptions): Generator<Instant> {
  const min = Instant.from(options?.minValue ?? -TEMPORAL_INSTANT_LIMIT);
  const max = Instant.from(options?.maxValue ?? TEMPORAL_INSTANT_LIMIT);
  requireOrdered("instants()", Instant.compare(min, max) > 0);
  const unit = precisionUnit(options?.precision);
  requireFit(
    "instant",
    min.epochNanoseconds,
    max.epochNanoseconds,
    unit.nanoseconds,
    `${min.toString()} and ${max.toString()}`,
  );
  return new BasicGenerator(
    {
      type: "timestamp",
      min: nativeTimestamp(min),
      max: nativeTimestamp(max),
      precision: unit.code,
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

const GRANULARITIES: Record<string, { code: number; seconds: number }> = {
  hour: { code: NativeGranularity.HOURS, seconds: 3600 },
  "quarter-hour": { code: NativeGranularity.QUARTER_HOURS, seconds: 900 },
  minute: { code: NativeGranularity.MINUTES, seconds: 60 },
  second: { code: NativeGranularity.SECONDS, seconds: 1 },
};

/** Options for {@link utcOffsets}. */
export interface UtcOffsetOptions {
  /** The smallest offset, inclusive, as `±HH:MM[:SS]` or seconds. Defaults to `-23:59`, or `-23:59:59` at second granularity. */
  minValue?: string | number;
  /** The largest offset, inclusive. Defaults to `+23:59`, or `+23:59:59` at second granularity. */
  maxValue?: string | number;
  /** Which offsets are drawn: whole hours, quarter hours, minutes (the default) or seconds. */
  granularity?: Granularity;
}

function offsetSeconds(name: string, value: string | number): number {
  if (typeof value === "string") return parseUtcOffset(value);
  if (!Number.isInteger(value) || Math.abs(value) > 93_599) {
    throw new Error(
      `${name} must be a ±HH:MM[:SS] string or a whole number of seconds within ±25:59:59, got ${String(value)}`,
    );
  }
  return value;
}

/**
 * Generate UTC offsets as `±HH:MM` strings (`±HH:MM:SS` when the offset is
 * not a whole number of minutes), shrinking towards `+00:00`. The strings
 * are valid `Temporal` time zone identifiers.
 */
export function utcOffsets(options?: UtcOffsetOptions): Generator<string> {
  const granularity = GRANULARITIES[String(options?.granularity ?? "minute").replace(/s$/, "")];
  if (granularity === undefined) {
    throw new Error(
      `granularity must be one of ${Object.keys(GRANULARITIES).join(", ")}, got ${String(options?.granularity)}`,
    );
  }
  const limit = granularity.seconds === 1 ? 86_399 : 86_340;
  const min = offsetSeconds("minValue", options?.minValue ?? -limit);
  const max = offsetSeconds("maxValue", options?.maxValue ?? limit);
  requireOrdered("utcOffsets()", min > max);
  if (Math.ceil(min / granularity.seconds) * granularity.seconds > max) {
    throw new Error(
      `No offset between ${formatUtcOffset(min)} and ${formatUtcOffset(max)} has the requested granularity`,
    );
  }
  return new BasicGenerator(
    { type: "offset", min, max, granularity: granularity.code },
    parseOffset,
  );
}

/** Options for {@link timeZones}. */
export interface TimeZoneOptions {
  /**
   * Draw only from these IANA names. Each must be known to both the engine's
   * database and the host's `Intl`. When given, `links` and `legacy` are ignored.
   */
  names?: readonly string[];
  /** Include link names (aliases such as `Asia/Calcutta` for `Asia/Kolkata`). Defaults to `true`. */
  links?: boolean;
  /** Include legacy names (`EST5EDT`, `US/Eastern`, ...). Defaults to `false`. */
  legacy?: boolean;
}

class TimeZonesGenerator extends BasicGenerator<string> {
  constructor(
    readonly names: readonly string[] | null,
    readonly links: boolean,
    readonly legacy: boolean,
  ) {
    super({ type: "timezone", names, links, legacy });
  }
}

function checkZoneNames(names: readonly string[]): string[] {
  const checked = [...new Set(names)];
  if (checked.length === 0) throw new Error("names must not be empty");
  for (const name of checked) {
    if (typeof name !== "string" || !hostKnowsZone(name)) {
      throw new Error(`The time zone ${String(name)} is not known to this host`);
    }
  }
  return checked;
}

/**
 * Generate IANA time zone names (`America/New_York`), shrinking towards
 * `UTC`. Only names both the engine's bundled database and the host's `Intl`
 * know are drawn, so `Temporal.ZonedDateTime` and `Intl.DateTimeFormat` accept
 * every value.
 */
export function timeZones(options?: TimeZoneOptions): Generator<string> {
  const names = options?.names === undefined ? null : checkZoneNames(options.names);
  return new TimeZonesGenerator(names, options?.links ?? true, options?.legacy ?? false);
}

/** Options for {@link zonedDateTimes}. */
export interface ZonedDateTimeOptions {
  /** Where the zone comes from. Defaults to {@link timeZones}`()`; any generator of IANA names the host knows works. */
  timeZones?: Generator<string>;
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
  ambiguous?: "both" | "earlier" | "later";
}

const ZONED_MIN_DATETIME = new PlainDateTime(-271821, 4, 22);
const ZONED_MAX_DATETIME = new PlainDateTime(275760, 9, 10, 23, 59, 59, 999, 999, 999);
const AMBIGUOUS = {
  both: NativeAmbiguous.BOTH,
  earlier: NativeAmbiguous.EARLIER,
  later: NativeAmbiguous.LATER,
} as const;

class ZonedDateTimesGenerator extends Generator<ZonedDateTime> {
  private readonly schemas = new Map<string, Record<string, unknown>>();
  private readonly engineSchema: Record<string, unknown> | null;

  constructor(
    private readonly zones: Generator<string>,
    private readonly fields: Record<string, unknown>,
  ) {
    super();
    this.engineSchema =
      zones instanceof TimeZonesGenerator
        ? { ...fields, names: zones.names, links: zones.links, legacy: zones.legacy }
        : null;
  }

  private schemaFor(zone: unknown): Record<string, unknown> {
    if (typeof zone !== "string") {
      throw new Error(`zonedDateTimes() needs a generator of time zone names, got ${String(zone)}`);
    }
    let schema = this.schemas.get(zone);
    if (schema === undefined) {
      schema = { ...this.fields, names: checkZoneNames([zone]), links: true, legacy: true };
      this.schemas.set(zone, schema);
    }
    return schema;
  }

  doDraw(tc: TestCase): ZonedDateTime {
    if (this.engineSchema !== null) return parseZonedDatetime(generateRaw(tc, this.engineSchema));
    tc.startSpan(Labels.ZONED_DATETIME);
    const zone = this.zones.doDraw(tc);
    const value = parseZonedDatetime(generateRaw(tc, this.schemaFor(zone)));
    tc.stopSpan();
    return value;
  }

  override asBasic(): BasicGenerator<ZonedDateTime> | null {
    return this.engineSchema === null
      ? null
      : new BasicGenerator(this.engineSchema, parseZonedDatetime);
  }
}

/**
 * Generate wall-clock times in a time zone as {@link ZonedDateTime}s,
 * shrinking towards 2000-01-01T00:00:00 in `UTC`. Transitions are explored:
 * values in a gap (`kind: "gap"`, unless `allowImaginary` is false) and in a
 * fold (`kind: "fold"`, with `fold` telling the occurrences apart) are drawn
 * alongside unambiguous ones. A draw whose offset the host's `Intl` disagrees
 * with (the two databases differ) is rejected like a failed assumption.
 *
 * The default wall-clock range lies two days inside `Temporal.PlainDateTime`'s,
 * so `Temporal.ZonedDateTime.from(value, { offset: "use" })` always succeeds;
 * see {@link ZonedDateTime} for how it treats a gap value.
 */
export function zonedDateTimes(options?: ZonedDateTimeOptions): Generator<ZonedDateTime> {
  const civilBounds = options?.minValue !== undefined || options?.maxValue !== undefined;
  const instantBounds = options?.minInstant !== undefined || options?.maxInstant !== undefined;
  if (civilBounds && instantBounds) {
    throw new Error(
      "zonedDateTimes() takes either wall-clock bounds (minValue, maxValue) or instant bounds (minInstant, maxInstant), not both",
    );
  }
  const unit = precisionUnit(options?.precision);
  const fields: Record<string, unknown> = {
    type: "zoned_datetime",
    civil: null,
    instant: null,
    precision: unit.code,
    imaginary: (options?.allowImaginary ?? true) ? NativeImaginary.ALLOW : NativeImaginary.AVOID,
  };
  const ambiguous = options?.ambiguous ?? "both";
  if (!(ambiguous in AMBIGUOUS)) {
    throw new Error(`ambiguous must be "both", "earlier" or "later", got ${String(ambiguous)}`);
  }
  fields["ambiguous"] = AMBIGUOUS[ambiguous];
  if (instantBounds) {
    const min = Instant.from(options?.minInstant ?? -TEMPORAL_INSTANT_LIMIT);
    const max = Instant.from(options?.maxInstant ?? TEMPORAL_INSTANT_LIMIT);
    requireOrdered("zonedDateTimes()", Instant.compare(min, max) > 0);
    fields["instant"] = { min: nativeTimestamp(min), max: nativeTimestamp(max) };
  } else {
    const min = PlainDateTime.from(options?.minValue ?? ZONED_MIN_DATETIME);
    const max = PlainDateTime.from(options?.maxValue ?? ZONED_MAX_DATETIME);
    requireOrdered("zonedDateTimes()", PlainDateTime.compare(min, max) > 0);
    requireFit(
      "datetime",
      min.epochNanoseconds,
      max.epochNanoseconds,
      unit.nanoseconds,
      `${min.toString()} and ${max.toString()}`,
    );
    fields["civil"] = { min: nativeDatetime(min), max: nativeDatetime(max) };
  }
  return new ZonedDateTimesGenerator(options?.timeZones ?? timeZones(), fields);
}

/** Inclusive bounds on one unit of a {@link durations} draw. */
export interface UnitBounds {
  min?: number;
  max?: number;
}

/**
 * Options for {@link durations}. Naming any unit enables only the named
 * units; with none named every unit is drawn. Each unit's default bounds
 * keep `Temporal.Duration.from(value)` valid: ±4294967295 for years, months
 * and weeks, ±10¹⁰ days, ±10¹¹ hours, ±10¹³ minutes, ±10¹⁵ seconds and
 * ±2⁵³−1 for milliseconds, microseconds and nanoseconds.
 */
export interface DurationOptions {
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

function unitBound(unit: DurationUnit, side: string, value: number): number {
  if (!Number.isSafeInteger(value) || Math.abs(value) > UNIT_LIMITS[unit]) {
    throw new Error(
      `${unit}.${side} must be an integer within ±${UNIT_LIMITS[unit]}, got ${String(value)}`,
    );
  }
  return value;
}

/**
 * Generate {@link Duration}s, shrinking towards zero. Every unit of a value
 * has the same sign, as `Temporal.Duration` requires, and
 * `Temporal.Duration.from(value)` converts a value.
 */
export function durations(options?: DurationOptions): Generator<Duration> {
  const sign = options?.sign ?? "uniform";
  if (sign !== "uniform" && sign !== "non-negative") {
    throw new Error(`sign must be "uniform" or "non-negative", got ${String(sign)}`);
  }
  const named = DURATION_UNITS.filter((unit) => options?.[unit] !== undefined);
  const enabled = named.length === 0 ? DURATION_UNITS : named;
  const units = enabled.map((unit) => {
    const bounds = options?.[unit] ?? {};
    const min = unitBound(unit, "min", bounds.min ?? (sign === "uniform" ? -UNIT_LIMITS[unit] : 0));
    const max = unitBound(unit, "max", bounds.max ?? UNIT_LIMITS[unit]);
    if (min > max) throw new Error(`${unit}: cannot have max < min`);
    if (sign === "non-negative" && min < 0) {
      throw new Error(`${unit}.min must not be negative with sign "non-negative"`);
    }
    return { unit: UNIT_CODES[unit], min: BigInt(min), max: BigInt(max) };
  });
  if (units.some((u) => u.min > 0n) && units.some((u) => u.max < 0n)) {
    throw new Error("durations(): the unit bounds leave no sign every unit can share");
  }
  return new BasicGenerator(
    {
      type: "span",
      units,
      sign: sign === "uniform" ? NativeSpanSign.UNIFORM : NativeSpanSign.NONNEGATIVE,
    },
    parseDuration,
  );
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
export function jsDates(options?: JsDateOptions): Generator<Date> {
  const min = dateMilliseconds("minValue", options?.minValue ?? new Date(-DATE_LIMIT_MILLISECONDS));
  const max = dateMilliseconds("maxValue", options?.maxValue ?? new Date(DATE_LIMIT_MILLISECONDS));
  requireOrdered("jsDates()", min > max);
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
