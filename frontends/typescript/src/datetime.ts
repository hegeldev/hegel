/**
 * Immutable date and time values.
 *
 * The date and time generators draw these rather than strings. Each class is
 * a plain property bag shaped like the `Temporal` class of the same name, so
 * on a host with `Temporal` the value converts with `Temporal.PlainDate.from(value)`,
 * `Temporal.ZonedDateTime.from(value, { offset: "use" })` and so on, and each
 * `toString()` prints the RFC 9557 form `Temporal` would.
 *
 * @packageDocumentation
 */

const NANOSECONDS_PER_SECOND = 1_000_000_000n;
const NANOSECONDS_PER_DAY = 86_400n * NANOSECONDS_PER_SECOND;

/** The years the engine draws: six signed digits. */
const MAX_YEAR = 999_999;
/** The engine's instant range, in seconds since the epoch, as `Instant` accepts. */
const MIN_EPOCH_SECONDS = -31_619_087_596_800n;
const MAX_EPOCH_SECONDS = 31_494_784_780_799n;
/** The widest UTC offset the engine draws, in seconds: just under 26 hours. */
const MAX_OFFSET_SECONDS = 93_599;
const TWO_32 = 2 ** 32;

function integerField(name: string, value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be an integer between ${min} and ${max}, got ${String(value)}`);
  }
  return value;
}

function compareNumbers(a: number | bigint, b: number | bigint): -1 | 0 | 1 {
  return a < b ? -1 : a > b ? 1 : 0;
}

/** Whether `year` is a leap year in the proleptic Gregorian calendar. */
export function isLeapYear(year: number): boolean {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
}

/** The number of days in `month` of `year`. */
export function daysInMonth(year: number, month: number): number {
  if (month === 2) return isLeapYear(year) ? 29 : 28;
  return [4, 6, 9, 11].includes(month) ? 30 : 31;
}

/** Days since 1970-01-01 of a proleptic Gregorian date. */
export function daysFromCivil(year: number, month: number, day: number): number {
  const y = month <= 2 ? year - 1 : year;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const doy = Math.floor((153 * (month + (month > 2 ? -3 : 9)) + 2) / 5) + day - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146_097 + doe - 719_468;
}

/** The proleptic Gregorian date `days` after 1970-01-01. */
export function civilFromDays(days: number): PlainDate {
  const z = days + 719_468;
  const era = Math.floor(z / 146_097);
  const doe = z - era * 146_097;
  const yoe = Math.floor(
    (doe - Math.floor(doe / 1460) + Math.floor(doe / 36_524) - Math.floor(doe / 146_096)) / 365,
  );
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const day = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const month = mp < 10 ? mp + 3 : mp - 9;
  return new PlainDate(era * 400 + yoe + (month <= 2 ? 1 : 0), month, day);
}

function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

function formatYear(year: number): string {
  if (year >= 0 && year <= 9999) return pad(year, 4);
  return `${year < 0 ? "-" : "+"}${pad(Math.abs(year), 6)}`;
}

/** The seconds fraction `Temporal` prints: nothing when zero, else the nanoseconds with trailing zeros removed. */
function formatFraction(nanoseconds: number | bigint): string {
  if (nanoseconds === 0 || nanoseconds === 0n) return "";
  return `.${pad(Number(nanoseconds), 9).replace(/0+$/, "")}`;
}

function formatClock(
  hour: number,
  minute: number,
  second: number,
  nanosecond: number | bigint,
): string {
  return `${pad(hour, 2)}:${pad(minute, 2)}:${pad(second, 2)}${formatFraction(nanosecond)}`;
}

/**
 * Format a UTC offset in seconds as `±HH:MM`, with `:SS` appended when the
 * offset is not a whole number of minutes (`+05:30`, `-00:44:30`).
 */
export function formatUtcOffset(seconds: number): string {
  integerField("offset", seconds, -MAX_OFFSET_SECONDS, MAX_OFFSET_SECONDS);
  const magnitude = Math.abs(seconds);
  const sign = seconds < 0 ? "-" : "+";
  const base = `${sign}${pad(Math.floor(magnitude / 3600), 2)}:${pad(Math.floor(magnitude / 60) % 60, 2)}`;
  return magnitude % 60 === 0 ? base : `${base}:${pad(magnitude % 60, 2)}`;
}

/** Parse a `±HH:MM` or `±HH:MM:SS` UTC offset into seconds. */
export function parseUtcOffset(offset: string): number {
  const match = /^([+-])(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(offset);
  if (match === null) {
    throw new Error(`UTC offsets must be formatted as ±HH:MM or ±HH:MM:SS, got ${offset}`);
  }
  const magnitude =
    Number(match[2]) * 3600 +
    Number(match[3]) * 60 +
    (match[4] === undefined ? 0 : Number(match[4]));
  if (Number(match[3]) > 59 || Number(match[4] ?? 0) > 59 || magnitude > MAX_OFFSET_SECONDS) {
    throw new Error(`UTC offsets must be within ±25:59:59, got ${offset}`);
  }
  return match[1] === "-" ? -magnitude : magnitude;
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

  /** The value with `like`'s fields; `like` may be a `Temporal.PlainDate`. */
  static from(like: PlainDateLike): PlainDate {
    if (like instanceof PlainDate) return like;
    return new PlainDate(like.year, like.month, like.day);
  }

  static compare(a: PlainDateLike, b: PlainDateLike): -1 | 0 | 1 {
    return compareNumbers(
      daysFromCivil(a.year, a.month, a.day),
      daysFromCivil(b.year, b.month, b.day),
    );
  }

  equals(other: PlainDateLike): boolean {
    return PlainDate.compare(this, other) === 0;
  }

  /** The ISO day of the week: 1 (Monday) to 7 (Sunday). */
  get dayOfWeek(): number {
    return (((daysFromCivil(this.year, this.month, this.day) % 7) + 10) % 7) + 1;
  }

  /** Days since 1970-01-01. */
  get epochDays(): number {
    return daysFromCivil(this.year, this.month, this.day);
  }

  /** `YYYY-MM-DD`, with a six-digit signed year outside 0000 to 9999. */
  toString(): string {
    return `${formatYear(this.year)}-${pad(this.month, 2)}-${pad(this.day, 2)}`;
  }

  toJSON(): string {
    return this.toString();
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

  /** The time `nanoseconds` after midnight. */
  static fromNanosecondsOfDay(nanoseconds: number): PlainTime {
    integerField("nanoseconds", nanoseconds, 0, 86_399_999_999_999);
    const subsecond = nanoseconds % 1_000_000_000;
    const seconds = (nanoseconds - subsecond) / 1_000_000_000;
    return new PlainTime(
      Math.floor(seconds / 3600),
      Math.floor(seconds / 60) % 60,
      seconds % 60,
      Math.floor(subsecond / 1_000_000),
      Math.floor(subsecond / 1000) % 1000,
      subsecond % 1000,
    );
  }

  static compare(a: PlainTimeLike, b: PlainTimeLike): -1 | 0 | 1 {
    return compareNumbers(PlainTime.from(a).nanosecondsOfDay, PlainTime.from(b).nanosecondsOfDay);
  }

  equals(other: PlainTimeLike): boolean {
    return PlainTime.compare(this, other) === 0;
  }

  /** Nanoseconds since midnight. */
  get nanosecondsOfDay(): number {
    return (
      ((this.hour * 60 + this.minute) * 60 + this.second) * 1_000_000_000 +
      this.millisecond * 1_000_000 +
      this.microsecond * 1000 +
      this.nanosecond
    );
  }

  /** `HH:MM:SS`, with the fraction of a second appended when it is not zero (`02:30:00.5`). */
  toString(): string {
    return formatClock(this.hour, this.minute, this.second, this.nanosecondsOfDay % 1_000_000_000);
  }

  toJSON(): string {
    return this.toString();
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

  /** The value with `like`'s fields; `like` may be a `Temporal.PlainDateTime`. */
  static from(like: PlainDateTimeLike): PlainDateTime {
    if (like instanceof PlainDateTime) return like;
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

  /** The value `date` and `time` make together. */
  static of(date: PlainDateLike, time: PlainTimeLike = {}): PlainDateTime {
    return PlainDateTime.from({ ...PlainDate.from(date), ...PlainTime.from(time) });
  }

  /** The wall-clock time `nanoseconds` after 1970-01-01T00:00:00. */
  static fromEpochNanoseconds(nanoseconds: bigint): PlainDateTime {
    const days =
      nanoseconds / NANOSECONDS_PER_DAY - (nanoseconds % NANOSECONDS_PER_DAY < 0n ? 1n : 0n);
    const date = civilFromDays(Number(days));
    const time = PlainTime.fromNanosecondsOfDay(Number(nanoseconds - days * NANOSECONDS_PER_DAY));
    return PlainDateTime.of(date, time);
  }

  static compare(a: PlainDateTimeLike, b: PlainDateTimeLike): -1 | 0 | 1 {
    return compareNumbers(
      PlainDateTime.from(a).epochNanoseconds,
      PlainDateTime.from(b).epochNanoseconds,
    );
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

  /** Nanoseconds from 1970-01-01T00:00:00 to this wall-clock time, read as if in UTC. */
  get epochNanoseconds(): bigint {
    return (
      BigInt(this.toPlainDate().epochDays) * NANOSECONDS_PER_DAY +
      BigInt(this.toPlainTime().nanosecondsOfDay)
    );
  }

  /** `YYYY-MM-DDTHH:MM:SS`, with the fraction of a second appended when it is not zero. */
  toString(): string {
    return `${this.toPlainDate().toString()}T${this.toPlainTime().toString()}`;
  }

  toJSON(): string {
    return this.toString();
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
    return compareNumbers(a.epochNanoseconds, b.epochNanoseconds);
  }

  equals(other: InstantLike): boolean {
    return Instant.compare(this, other) === 0;
  }

  /** Whole seconds since the epoch, rounded down. */
  get epochSeconds(): number {
    return Number(floorDiv(this.epochNanoseconds, NANOSECONDS_PER_SECOND));
  }

  /** Whole milliseconds since the epoch, rounded down. */
  get epochMilliseconds(): number {
    return Number(floorDiv(this.epochNanoseconds, 1_000_000n));
  }

  /** The nanoseconds within the second at {@link epochSeconds}. */
  get nanosecond(): number {
    return Number(this.epochNanoseconds - BigInt(this.epochSeconds) * NANOSECONDS_PER_SECOND);
  }

  /** The UTC wall-clock time followed by `Z` (`2000-01-01T00:00:00Z`). */
  toString(): string {
    return `${PlainDateTime.fromEpochNanoseconds(this.epochNanoseconds).toString()}Z`;
  }

  toJSON(): string {
    return this.toString();
  }
}

function floorDiv(a: bigint, b: bigint): bigint {
  const q = a / b;
  return a % b < 0n ? q - 1n : q;
}

/** How a wall-clock time relates to its zone's transitions. */
export type TransitionKind = "unique" | "gap" | "fold";

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

/**
 * A wall-clock time in a time zone together with the instant it names, like
 * `Temporal.ZonedDateTime`.
 *
 * A wall-clock time in a gap (one a forward transition skipped) carries
 * the offset after the transition and names the instant read with the offset
 * before it, the forward shift jiff and Python's `zoneinfo` make: 02:30 in a
 * gap from 02:00 to 03:00 is 03:30 after the shift. The same instant is
 * `new Temporal.ZonedDateTime(value.epochNanoseconds, value.timeZone)`.
 * `Temporal.ZonedDateTime.from(value, { offset: "use" })` instead applies
 * the carried offset to the wall-clock time, landing one transition's width
 * earlier (01:30 before the shift). A wall-clock time in a fold (one a
 * backward transition repeated) carries the offset of the occurrence it is,
 * so every conversion agrees.
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
    if (!["unique", "gap", "fold"].includes(like.kind)) {
      throw new Error(`kind must be "unique", "gap" or "fold", got ${String(like.kind)}`);
    }
    this.timeZone = like.timeZone;
    this.offset = formatUtcOffset(parseUtcOffset(like.offset));
    this.kind = like.kind;
    this.fold = Boolean(like.fold);
    this.offsetBefore = formatUtcOffset(parseUtcOffset(like.offsetBefore));
    this.offsetAfter = formatUtcOffset(parseUtcOffset(like.offsetAfter));
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

  equals(other: ZonedDateTimeLike): boolean {
    return (
      this.epochNanoseconds === other.epochNanoseconds &&
      this.timeZone === other.timeZone &&
      this.toPlainDateTime().equals(other)
    );
  }

  /**
   * `YYYY-MM-DDTHH:MM:SS±HH:MM[zone]` (`2024-03-10T02:30:00-04:00[America/New_York]`),
   * with `[gap]`, `[fold=0]` or `[fold=1]` appended for a wall-clock time a
   * transition skipped or repeated.
   */
  toString(): string {
    const suffix =
      this.kind === "gap" ? "[gap]" : this.kind === "fold" ? `[fold=${this.fold ? 1 : 0}]` : "";
    return `${this.toPlainDateTime().toString()}${this.offset}[${this.timeZone}]${suffix}`;
  }

  toJSON(): string {
    return this.toString();
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

/** Validate `like`'s units: integers in `Temporal.Duration`'s range, all of one sign, with `-0` normalised. */
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
   * ISO 8601 as `Temporal.Duration` prints it: `P1Y2M3DT4H`, `-PT1H30M5.5S`,
   * `PT0S`, with the sub-second units folded into the seconds fraction.
   */
  toString(): string {
    const magnitude = this.abs();
    let date = "";
    if (magnitude.years) date += `${magnitude.years}Y`;
    if (magnitude.months) date += `${magnitude.months}M`;
    if (magnitude.weeks) date += `${magnitude.weeks}W`;
    if (magnitude.days) date += `${magnitude.days}D`;
    let time = "";
    if (magnitude.hours) time += `${magnitude.hours}H`;
    if (magnitude.minutes) time += `${magnitude.minutes}M`;
    const subsecond =
      BigInt(magnitude.milliseconds) * 1_000_000n +
      BigInt(magnitude.microseconds) * 1000n +
      BigInt(magnitude.nanoseconds);
    const totalNanoseconds = BigInt(magnitude.seconds) * NANOSECONDS_PER_SECOND + subsecond;
    if (totalNanoseconds !== 0n || (date === "" && time === "")) {
      const seconds = totalNanoseconds / NANOSECONDS_PER_SECOND;
      time += `${seconds}${formatFraction(totalNanoseconds % NANOSECONDS_PER_SECOND)}S`;
    }
    return `${this.sign < 0 ? "-" : ""}P${date}${time === "" ? "" : `T${time}`}`;
  }

  toJSON(): string {
    return this.toString();
  }
}
