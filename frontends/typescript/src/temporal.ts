/**
 * Immutable date and time values, exported from the `generators.temporal`
 * namespace alongside the generators that draw them
 * (`gs.temporal.PlainDate` and so on).
 *
 * Each class is a plain property bag shaped like the `Temporal` class of the
 * same name, so on a host with `Temporal` a value converts with
 * `Temporal.PlainDate.from(value)`, `Temporal.PlainTime.from(value)`,
 * `Temporal.PlainDateTime.from(value)`, `Temporal.Duration.from(value)` and
 * `Temporal.Instant.fromEpochNanoseconds(value.epochNanoseconds)`. A
 * {@link ZonedDateTime} converts with {@link ZonedDateTime.toTemporal}.
 *
 * `toString()` and `toJSON()` give the engine's text of the value, the text
 * a failing test's report prints it as and the same in every Hegel
 * frontend. For a date, a time, a datetime and an instant that is the ISO
 * 8601 string `Temporal` prints. For a {@link ZonedDateTime} and a
 * {@link Duration} it is not a string `Temporal` parses: convert the value,
 * not its text.
 *
 * @packageDocumentation
 */

import {
  integerField,
  daysInMonth,
  daysFromCivil,
  weekdayFromDays,
  parseUtcOffset,
} from "./calendar.js";
import { entryEngine } from "./runtime.js";
import { prettyPrint, type PrettyPrinter } from "./pretty.js";
import type { NativeDate, NativeTime, NativeDatetime, NativeTimestamp } from "./engine.js";

const NANOSECONDS_PER_SECOND = 1_000_000_000n;

/** The years the engine draws: six signed digits. */
const MAX_YEAR = 999_999;
/** The engine's instant range, in seconds since the epoch, as `Instant` accepts. */
const MIN_EPOCH_SECONDS = -31_619_087_596_800n;
const MAX_EPOCH_SECONDS = 31_494_784_780_799n;
const TWO_32 = 2 ** 32;

function compareFields(a: readonly number[], b: readonly number[]): -1 | 0 | 1 {
  for (let index = 0; index < a.length; index++) {
    if (a[index] !== b[index]) return a[index] < b[index] ? -1 : 1;
  }
  return 0;
}

function floorDiv(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b < 0n ? q - 1n : q;
}

/** Throw for a `Temporal` value in a calendar other than ISO 8601, whose fields count differently. */
function requireIsoCalendar(like: object): void {
  const calendar = (like as { calendarId?: unknown }).calendarId;
  if (calendar !== undefined && calendar !== "iso8601") {
    throw new Error(
      `Dates are read in the ISO 8601 calendar, got one in the ${String(calendar)} calendar`,
    );
  }
}

/** @internal The engine's form of a date. */
export function nativeDate(date: PlainDateLike): NativeDate {
  return { year: date.year, month: date.month, day: date.day };
}

/** @internal The engine's form of a time of day. */
export function nativeTime(time: Required<PlainTimeLike>): NativeTime {
  return {
    hour: time.hour,
    minute: time.minute,
    second: time.second,
    nanosecond: time.millisecond * 1_000_000 + time.microsecond * 1000 + time.nanosecond,
  };
}

/** @internal The engine's form of a wall-clock datetime. */
export function nativeDatetime(value: PlainDateLike & Required<PlainTimeLike>): NativeDatetime {
  return { date: nativeDate(value), time: nativeTime(value) };
}

/** @internal The engine's form of an instant. */
export function nativeTimestamp(epochNanoseconds: bigint): NativeTimestamp {
  const second = floorDiv(epochNanoseconds, NANOSECONDS_PER_SECOND);
  return { second, nanosecond: Number(epochNanoseconds - second * NANOSECONDS_PER_SECOND) };
}

/** The fields of a {@link PlainDate}; `Temporal.PlainDate` has this shape. */
export interface PlainDateLike {
  year: number;
  month: number;
  day: number;
}

/** A calendar date without a time or time zone, like `Temporal.PlainDate`. */
export class PlainDate implements PlainDateLike {
  readonly year: number;
  /** 1 (January) to 12 (December). */
  readonly month: number;
  readonly day: number;

  constructor(year: number, month: number, day: number) {
    this.year = integerField("year", year, -MAX_YEAR, MAX_YEAR);
    this.month = integerField("month", month, 1, 12);
    this.day = integerField("day", day, 1, daysInMonth(this.year, this.month));
    Object.freeze(this);
  }

  /**
   * The value with `like`'s fields; `like` may be a `Temporal.PlainDate` in
   * the ISO 8601 calendar.
   */
  static from(like: PlainDateLike): PlainDate {
    if (like instanceof PlainDate) return like;
    requireIsoCalendar(like);
    return new PlainDate(like.year, like.month, like.day);
  }

  static compare(a: PlainDateLike, b: PlainDateLike): -1 | 0 | 1 {
    return compareFields([a.year, a.month, a.day], [b.year, b.month, b.day]);
  }

  equals(other: PlainDateLike): boolean {
    return PlainDate.compare(this, other) === 0;
  }

  /** The ISO day of the week: 1 (Monday) to 7 (Sunday). */
  get dayOfWeek(): number {
    return weekdayFromDays(daysFromCivil(this.year, this.month, this.day));
  }

  /** `YYYY-MM-DD`, with a signed year of six digits outside 0000 to 9999. */
  toString(): string {
    return entryEngine().formatDate(nativeDate(this));
  }

  toJSON(): string {
    return this.toString();
  }

  /** Prints the value in a failing test's report, as `PlainDate(2024-03-10)`. */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`PlainDate(${this.toString()})`);
  }
}

/** The fields of a {@link PlainTime}; `Temporal.PlainTime` has this shape. Missing fields are zero. */
export interface PlainTimeLike {
  hour?: number;
  minute?: number;
  second?: number;
  millisecond?: number;
  microsecond?: number;
  nanosecond?: number;
}

function timeFields(time: Required<PlainTimeLike>): number[] {
  return [time.hour, time.minute, time.second, time.millisecond, time.microsecond, time.nanosecond];
}

/** A wall-clock time of day, like `Temporal.PlainTime`. */
export class PlainTime implements Required<PlainTimeLike> {
  readonly hour: number;
  readonly minute: number;
  readonly second: number;
  readonly millisecond: number;
  readonly microsecond: number;
  readonly nanosecond: number;

  constructor(hour = 0, minute = 0, second = 0, millisecond = 0, microsecond = 0, nanosecond = 0) {
    this.hour = integerField("hour", hour, 0, 23);
    this.minute = integerField("minute", minute, 0, 59);
    this.second = integerField("second", second, 0, 59);
    this.millisecond = integerField("millisecond", millisecond, 0, 999);
    this.microsecond = integerField("microsecond", microsecond, 0, 999);
    this.nanosecond = integerField("nanosecond", nanosecond, 0, 999);
    Object.freeze(this);
  }

  /** The value with `like`'s fields; `like` may be a `Temporal.PlainTime`. */
  static from(like: PlainTimeLike): PlainTime {
    if (like instanceof PlainTime) return like;
    return new PlainTime(
      like.hour,
      like.minute,
      like.second,
      like.millisecond,
      like.microsecond,
      like.nanosecond,
    );
  }

  static compare(a: PlainTimeLike, b: PlainTimeLike): -1 | 0 | 1 {
    return compareFields(timeFields(PlainTime.from(a)), timeFields(PlainTime.from(b)));
  }

  equals(other: PlainTimeLike): boolean {
    return PlainTime.compare(this, other) === 0;
  }

  /** `HH:MM:SS`, with the fraction of a second appended when it is not zero (`02:30:00.5`). */
  toString(): string {
    return entryEngine().formatTime(nativeTime(this));
  }

  toJSON(): string {
    return this.toString();
  }

  /** Prints the value in a failing test's report, as `PlainTime(02:30:00)`. */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`PlainTime(${this.toString()})`);
  }
}

/** The fields of a {@link PlainDateTime}; `Temporal.PlainDateTime` has this shape. */
export interface PlainDateTimeLike extends PlainDateLike, PlainTimeLike {}

/** A calendar date and wall-clock time without a time zone, like `Temporal.PlainDateTime`. */
export class PlainDateTime implements PlainDateLike, Required<PlainTimeLike> {
  readonly year: number;
  readonly month: number;
  readonly day: number;
  readonly hour: number;
  readonly minute: number;
  readonly second: number;
  readonly millisecond: number;
  readonly microsecond: number;
  readonly nanosecond: number;

  constructor(
    year: number,
    month: number,
    day: number,
    hour = 0,
    minute = 0,
    second = 0,
    millisecond = 0,
    microsecond = 0,
    nanosecond = 0,
  ) {
    const date = new PlainDate(year, month, day);
    const time = new PlainTime(hour, minute, second, millisecond, microsecond, nanosecond);
    this.year = date.year;
    this.month = date.month;
    this.day = date.day;
    this.hour = time.hour;
    this.minute = time.minute;
    this.second = time.second;
    this.millisecond = time.millisecond;
    this.microsecond = time.microsecond;
    this.nanosecond = time.nanosecond;
    Object.freeze(this);
  }

  /**
   * The value with `like`'s fields; `like` may be a `Temporal.PlainDateTime`
   * in the ISO 8601 calendar.
   */
  static from(like: PlainDateTimeLike): PlainDateTime {
    if (like instanceof PlainDateTime) return like;
    requireIsoCalendar(like);
    return new PlainDateTime(
      like.year,
      like.month,
      like.day,
      like.hour,
      like.minute,
      like.second,
      like.millisecond,
      like.microsecond,
      like.nanosecond,
    );
  }

  static compare(a: PlainDateTimeLike, b: PlainDateTimeLike): -1 | 0 | 1 {
    const first = PlainDateTime.from(a);
    const second = PlainDateTime.from(b);
    return PlainDate.compare(first, second) || compareFields(timeFields(first), timeFields(second));
  }

  equals(other: PlainDateTimeLike): boolean {
    return PlainDateTime.compare(this, other) === 0;
  }

  toPlainDate(): PlainDate {
    return new PlainDate(this.year, this.month, this.day);
  }

  toPlainTime(): PlainTime {
    return PlainTime.from(this);
  }

  /** `YYYY-MM-DDTHH:MM:SS`, with the fraction of a second appended when it is not zero. */
  toString(): string {
    return entryEngine().formatDatetime(nativeDatetime(this));
  }

  toJSON(): string {
    return this.toString();
  }

  /** Prints the value in a failing test's report, as `PlainDateTime(2024-03-10T02:30:00)`. */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`PlainDateTime(${this.toString()})`);
  }
}

/** The field of an {@link Instant}; `Temporal.Instant` has this shape. */
export interface InstantLike {
  epochNanoseconds: bigint;
}

/** An exact moment in time, like `Temporal.Instant`. */
export class Instant implements InstantLike {
  /** Nanoseconds since 1970-01-01T00:00:00Z. */
  readonly epochNanoseconds: bigint;

  constructor(epochNanoseconds: bigint) {
    if (
      typeof epochNanoseconds !== "bigint" ||
      epochNanoseconds < MIN_EPOCH_SECONDS * NANOSECONDS_PER_SECOND ||
      epochNanoseconds > (MAX_EPOCH_SECONDS + 1n) * NANOSECONDS_PER_SECOND - 1n
    ) {
      throw new Error(
        `Instants must be within ±999999 years of the epoch, got ${String(epochNanoseconds)}`,
      );
    }
    this.epochNanoseconds = epochNanoseconds;
    Object.freeze(this);
  }

  /** The instant `like` names, as epoch nanoseconds or an object with them (such as a `Temporal.Instant`). */
  static from(like: InstantLike | bigint): Instant {
    if (like instanceof Instant) return like;
    return new Instant(typeof like === "bigint" ? like : like.epochNanoseconds);
  }

  static fromEpochMilliseconds(epochMilliseconds: number): Instant {
    if (!Number.isInteger(epochMilliseconds)) {
      throw new Error(`Epoch milliseconds must be an integer, got ${String(epochMilliseconds)}`);
    }
    return new Instant(BigInt(epochMilliseconds) * 1_000_000n);
  }

  static compare(a: InstantLike, b: InstantLike): -1 | 0 | 1 {
    return a.epochNanoseconds < b.epochNanoseconds
      ? -1
      : a.epochNanoseconds > b.epochNanoseconds
        ? 1
        : 0;
  }

  equals(other: InstantLike): boolean {
    return Instant.compare(this, other) === 0;
  }

  /** Whole milliseconds since the epoch, rounded down. */
  get epochMilliseconds(): number {
    return Number(floorDiv(this.epochNanoseconds, 1_000_000n));
  }

  /** The UTC wall-clock time followed by `Z` (`2000-01-01T00:00:00Z`). */
  toString(): string {
    return entryEngine().formatTimestamp(nativeTimestamp(this.epochNanoseconds));
  }

  toJSON(): string {
    return this.toString();
  }

  /** Prints the value in a failing test's report, as `Instant(2000-01-01T00:00:00Z)`. */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`Instant(${this.toString()})`);
  }
}

/** How a wall-clock time relates to its zone's transitions. */
export type TransitionKind = "unique" | "gap" | "fold";

const TRANSITION_KINDS: readonly TransitionKind[] = ["unique", "gap", "fold"];

/** The fields of a {@link ZonedDateTime}. */
export interface ZonedDateTimeLike extends PlainDateTimeLike, InstantLike {
  /** An IANA time zone name. */
  timeZone: string;
  /** The UTC offset in force, as `±HH:MM[:SS]`. */
  offset: string;
  kind: TransitionKind;
  /** Whether this is the second occurrence of a repeated wall-clock time. */
  fold: boolean;
  /** The UTC offset before the transition this wall-clock time sits in, or `offset` when `kind` is `"unique"`. */
  offsetBefore: string;
  /** The UTC offset after that transition, or `offset` when `kind` is `"unique"`. */
  offsetAfter: string;
}

/** A UTC offset in the engine's spelling: `+05:30`, `-00:44:30`. */
function canonicalOffset(offset: string): string {
  return entryEngine().formatOffset({ seconds: parseUtcOffset(offset), known: true });
}

/**
 * A wall-clock time in a time zone together with the instant it names, like
 * `Temporal.ZonedDateTime`.
 *
 * A wall-clock time in a gap (one a forward transition skipped) carries
 * the offset after the transition and names the instant read with the offset
 * before it, the forward shift jiff, Python's `zoneinfo` and `Temporal`'s
 * default make: 02:30 in a gap from 02:00 to 03:00 is 03:30 after the shift.
 * A wall-clock time in a fold (one a backward transition repeated) carries
 * the offset of the occurrence it is.
 *
 * {@link ZonedDateTime.toTemporal} gives the `Temporal.ZonedDateTime` at the
 * same instant in the same zone. `Temporal.ZonedDateTime.from(value)` is not
 * that conversion: it rejects every gap value, whose wall-clock time and
 * offset do not occur together, and with `{ offset: "use" }` it reads one as
 * an instant a transition's width earlier.
 */
export class ZonedDateTime implements ZonedDateTimeLike {
  readonly year: number;
  readonly month: number;
  readonly day: number;
  readonly hour: number;
  readonly minute: number;
  readonly second: number;
  readonly millisecond: number;
  readonly microsecond: number;
  readonly nanosecond: number;
  readonly timeZone: string;
  readonly offset: string;
  readonly kind: TransitionKind;
  readonly fold: boolean;
  readonly offsetBefore: string;
  readonly offsetAfter: string;
  readonly epochNanoseconds: bigint;

  constructor(like: ZonedDateTimeLike) {
    const civil = PlainDateTime.from(like);
    this.year = civil.year;
    this.month = civil.month;
    this.day = civil.day;
    this.hour = civil.hour;
    this.minute = civil.minute;
    this.second = civil.second;
    this.millisecond = civil.millisecond;
    this.microsecond = civil.microsecond;
    this.nanosecond = civil.nanosecond;
    if (typeof like.timeZone !== "string" || like.timeZone === "") {
      throw new Error("timeZone must be a time zone name");
    }
    if (!TRANSITION_KINDS.includes(like.kind)) {
      throw new Error(`kind must be "unique", "gap" or "fold", got ${String(like.kind)}`);
    }
    this.timeZone = like.timeZone;
    this.offset = canonicalOffset(like.offset);
    this.kind = like.kind;
    this.fold = Boolean(like.fold);
    this.offsetBefore = canonicalOffset(like.offsetBefore);
    this.offsetAfter = canonicalOffset(like.offsetAfter);
    this.epochNanoseconds = Instant.from(like).epochNanoseconds;
    Object.freeze(this);
  }

  static from(like: ZonedDateTimeLike): ZonedDateTime {
    if (like instanceof ZonedDateTime) return like;
    return new ZonedDateTime(like);
  }

  /** The UTC offset in force, in nanoseconds. */
  get offsetNanoseconds(): number {
    return parseUtcOffset(this.offset) * 1_000_000_000;
  }

  get epochMilliseconds(): number {
    return this.toInstant().epochMilliseconds;
  }

  toInstant(): Instant {
    return new Instant(this.epochNanoseconds);
  }

  toPlainDateTime(): PlainDateTime {
    return PlainDateTime.from(this);
  }

  toPlainDate(): PlainDate {
    return PlainDate.from(this);
  }

  toPlainTime(): PlainTime {
    return PlainTime.from(this);
  }

  /**
   * The `Temporal.ZonedDateTime` at this value's instant in its zone, given
   * the host's `Temporal` or a polyfill's: `value.toTemporal(Temporal)`. Its
   * wall-clock time and offset are this value's, except for a gap value,
   * where they are the ones the clocks showed at that instant (03:30 for
   * 02:30 in a gap from 02:00 to 03:00). It throws what `Temporal` throws
   * for a zone `Temporal` does not know, such as one of a custom
   * {@link ZoneSource}.
   */
  toTemporal<T>(temporal: {
    ZonedDateTime: new (epochNanoseconds: bigint, timeZone: string) => T;
  }): T {
    return new temporal.ZonedDateTime(this.epochNanoseconds, this.timeZone);
  }

  equals(other: ZonedDateTimeLike): boolean {
    return (
      this.epochNanoseconds === other.epochNanoseconds &&
      this.timeZone === other.timeZone &&
      this.toPlainDateTime().equals(other)
    );
  }

  /**
   * `YYYY-MM-DDTHH:MM:SS±HH:MM[zone]` (`2024-03-10T01:30:00-05:00[America/New_York]`),
   * with `[gap]`, `[fold=0]` or `[fold=1]` appended for a wall-clock time a
   * transition skipped or repeated
   * (`2024-03-10T02:30:00-04:00[America/New_York][gap]`), and an offset that
   * is not a whole number of minutes printed with its seconds
   * (`-04:56:02`). `Temporal` does not parse the annotated strings and
   * prints such an offset rounded: use {@link ZonedDateTime.toTemporal}.
   */
  toString(): string {
    return entryEngine().formatZonedDatetime(
      {
        civil: nativeDatetime(this),
        offsetSeconds: parseUtcOffset(this.offset),
        offsetKnown: true,
        fold: this.fold,
        kind: TRANSITION_KINDS.indexOf(this.kind),
        offsetBefore: parseUtcOffset(this.offsetBefore),
        offsetAfter: parseUtcOffset(this.offsetAfter),
        timestamp: nativeTimestamp(this.epochNanoseconds),
      },
      this.timeZone,
    );
  }

  toJSON(): string {
    return this.toString();
  }

  /**
   * Prints the value in a failing test's report, as
   * `ZonedDateTime(2024-03-10T02:30:00-04:00[America/New_York][gap])`.
   */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`ZonedDateTime(${this.toString()})`);
  }
}

/** The fields of a {@link Duration}; `Temporal.Duration` has this shape. Missing fields are zero. */
export interface DurationLike {
  years?: number;
  months?: number;
  weeks?: number;
  days?: number;
  hours?: number;
  minutes?: number;
  seconds?: number;
  milliseconds?: number;
  microseconds?: number;
  nanoseconds?: number;
}

/** The unit fields of a {@link Duration}, largest first. */
export const DURATION_UNITS = [
  "years",
  "months",
  "weeks",
  "days",
  "hours",
  "minutes",
  "seconds",
  "milliseconds",
  "microseconds",
  "nanoseconds",
] as const;

/** A unit of a {@link Duration}. */
export type DurationUnit = (typeof DURATION_UNITS)[number];

/**
 * Validate `like`'s units: integers, at most 2³²−1 in magnitude for years,
 * months and weeks and 2⁵³−1 for the rest, all of one sign, with `-0`
 * normalised.
 */
function durationFields(like: DurationLike): Required<DurationLike> {
  const fields: Partial<Record<DurationUnit, number>> = {};
  let sign = 0;
  for (const unit of DURATION_UNITS) {
    const limit = ["years", "months", "weeks"].includes(unit)
      ? TWO_32 - 1
      : Number.MAX_SAFE_INTEGER;
    const value = integerField(unit, like[unit] ?? 0, -limit, limit);
    if (value !== 0) {
      if (sign !== 0 && Math.sign(value) !== sign) {
        throw new Error("Every unit of a Duration must have the same sign");
      }
      sign = Math.sign(value);
    }
    fields[unit] = value === 0 ? 0 : value;
  }
  return fields as Required<DurationLike>;
}

/**
 * A span of calendar and clock units, like `Temporal.Duration`: every unit
 * has the same sign, and the value is not reduced (90 minutes stays 90
 * minutes rather than becoming an hour and a half).
 *
 * Each unit is an integer of at most 2³²−1 in magnitude for years, months and
 * weeks and 2⁵³−1 for the rest. `Temporal.Duration` also requires the days
 * and smaller units to total less than 2⁵³ seconds, which every value
 * {@link spans} draws does and a value built by hand may not.
 */
export class Duration implements Required<DurationLike> {
  readonly years: number;
  readonly months: number;
  readonly weeks: number;
  readonly days: number;
  readonly hours: number;
  readonly minutes: number;
  readonly seconds: number;
  readonly milliseconds: number;
  readonly microseconds: number;
  readonly nanoseconds: number;

  constructor(like: DurationLike = {}) {
    const fields = durationFields(like);
    this.years = fields.years;
    this.months = fields.months;
    this.weeks = fields.weeks;
    this.days = fields.days;
    this.hours = fields.hours;
    this.minutes = fields.minutes;
    this.seconds = fields.seconds;
    this.milliseconds = fields.milliseconds;
    this.microseconds = fields.microseconds;
    this.nanoseconds = fields.nanoseconds;
    Object.freeze(this);
  }

  /** The value with `like`'s fields; `like` may be a `Temporal.Duration`. */
  static from(like: DurationLike): Duration {
    if (like instanceof Duration) return like;
    return new Duration(like);
  }

  /** -1, 0 or 1: the sign every non-zero unit shares. */
  get sign(): -1 | 0 | 1 {
    for (const unit of DURATION_UNITS) {
      if (this[unit] !== 0) return this[unit] < 0 ? -1 : 1;
    }
    return 0;
  }

  /** Whether every unit is zero. */
  get blank(): boolean {
    return this.sign === 0;
  }

  negated(): Duration {
    const fields: DurationLike = {};
    for (const unit of DURATION_UNITS) fields[unit] = -this[unit];
    return new Duration(fields);
  }

  abs(): Duration {
    return this.sign < 0 ? this.negated() : this;
  }

  equals(other: DurationLike): boolean {
    return DURATION_UNITS.every((unit) => this[unit] === (other[unit] ?? 0));
  }

  /**
   * The units that are not zero, largest first, each with its suffix (`y`,
   * `mo`, `w`, `d`, `h`, `m`, `s`, `ms`, `µs`, `ns`): `1y 2mo 3d`, `90m`,
   * `-1h 30m` (one sign for the whole value) and `0s` for the zero span.
   * This is not the ISO 8601 string `Temporal.Duration` prints and parses.
   */
  toString(): string {
    return entryEngine().formatSpan({
      years: BigInt(this.years),
      months: BigInt(this.months),
      weeks: BigInt(this.weeks),
      days: BigInt(this.days),
      hours: BigInt(this.hours),
      minutes: BigInt(this.minutes),
      seconds: BigInt(this.seconds),
      milliseconds: BigInt(this.milliseconds),
      microseconds: BigInt(this.microseconds),
      nanoseconds: BigInt(this.nanoseconds),
    });
  }

  toJSON(): string {
    return this.toString();
  }

  /** Prints the value in a failing test's report, as `Duration(1y 2mo 3d)`. */
  [prettyPrint](printer: PrettyPrinter): void {
    printer.text(`Duration(${this.toString()})`);
  }
}
