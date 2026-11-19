/**
 * Field validation, the calendar arithmetic a date's weekday needs, and UTC
 * offset parsing, shared by the date and time values and the zone sources.
 *
 * @internal
 * @packageDocumentation
 */

export function integerField(name: string, value: unknown, min: number, max: number): number {
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) {
    throw new Error(`${name} must be an integer between ${min} and ${max}, got ${String(value)}`);
  }
  return value;
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

/** The ISO day of the week, 1 (Monday) to 7 (Sunday), of the date `days` after 1970-01-01. */
export function weekdayFromDays(days: number): number {
  return (((days % 7) + 10) % 7) + 1;
}

/** Parse a `±HH:MM` or `±HH:MM:SS` UTC offset into seconds. */
export function parseUtcOffset(offset: string): number {
  const match = /^([+-])(\d{2}):([0-5]\d)(?::([0-5]\d))?$/.exec(offset);
  if (match === null) {
    throw new Error(`UTC offsets must be formatted as ±HH:MM or ±HH:MM:SS, got ${offset}`);
  }
  const magnitude =
    Number(match[2]) * 3600 +
    Number(match[3]) * 60 +
    (match[4] === undefined ? 0 : Number(match[4]));
  return match[1] === "-" ? -magnitude : magnitude;
}
