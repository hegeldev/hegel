/**
 * Calendar arithmetic and UTC offset formatting shared by the date and time
 * values and generators.
 *
 * @internal
 * @packageDocumentation
 */

/** The widest UTC offset the engine draws, in seconds: just under 26 hours. */
const MAX_OFFSET_SECONDS = 93_599;

export function integerField(name: string, value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be an integer between ${min} and ${max}, got ${String(value)}`);
  }
  return value;
}

export function pad(value: number, width: number): string {
  return String(value).padStart(width, "0");
}

/** A proleptic Gregorian date's fields. */
export interface CivilDate {
  year: number;
  month: number;
  day: number;
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
export function civilFromDays(days: number): CivilDate {
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
  return { year: era * 400 + yoe + (month <= 2 ? 1 : 0), month, day };
}

/** The ISO day of the week, 1 (Monday) to 7 (Sunday), of the date `days` after 1970-01-01. */
export function weekdayFromDays(days: number): number {
  return (((days % 7) + 10) % 7) + 1;
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
