/**
 * The ISO 8601 string generators of earlier releases, kept as deprecated
 * wrappers over the {@link temporal} generators.
 *
 * @packageDocumentation
 */

import type { Generator } from "./core.js";
import * as temporal from "./temporal.js";
import type { PlainDateLike, PlainTimeLike } from "../temporal.js";

const pad = (value: number, width: number): string => String(value).padStart(width, "0");

function formatDate(d: PlainDateLike): string {
  return `${pad(d.year, 4)}-${pad(d.month, 2)}-${pad(d.day, 2)}`;
}

function formatTime(t: Required<PlainTimeLike>): string {
  const base = `${pad(t.hour, 2)}:${pad(t.minute, 2)}:${pad(t.second, 2)}`;
  const nanoseconds = t.millisecond * 1_000_000 + t.microsecond * 1_000 + t.nanosecond;
  if (nanoseconds === 0) {
    return base;
  }
  if (t.nanosecond === 0) {
    return `${base}.${pad(nanoseconds / 1_000, 6)}`;
  }
  return `${base}.${pad(nanoseconds, 9)}`;
}

const MIN_DATE = { year: 1, month: 1, day: 1 };
const MAX_DATE = { year: 9999, month: 12, day: 31 };

/**
 * Generate date strings (ISO 8601, `YYYY-MM-DD`) from 0001-01-01 to
 * 9999-12-31.
 *
 * @deprecated Use `temporal.dates()`, which generates `PlainDate`
 * values.
 */
export function dates(): Generator<string> {
  return temporal.dates({ minValue: MIN_DATE, maxValue: MAX_DATE }).map(formatDate);
}

/**
 * Generate time-of-day strings (ISO 8601): `HH:MM:SS`, with `.ffffff` for a
 * whole number of microseconds or `.fffffffff` otherwise.
 *
 * @deprecated Use `temporal.times()`, which generates `PlainTime`
 * values.
 */
export function times(): Generator<string> {
  return temporal.times().map(formatTime);
}

/**
 * Generate naive datetime strings (ISO 8601, `<date>T<time>` in the forms
 * of {@link dates} and {@link times}) from 0001-01-01 to 9999-12-31.
 *
 * @deprecated Use `temporal.datetimes()`, which generates
 * `PlainDateTime` values.
 */
export function datetimes(): Generator<string> {
  return temporal
    .datetimes({
      minValue: { ...MIN_DATE, hour: 0 },
      maxValue: {
        ...MAX_DATE,
        hour: 23,
        minute: 59,
        second: 59,
        millisecond: 999,
        microsecond: 999,
        nanosecond: 999,
      },
    })
    .map((value) => `${formatDate(value)}T${formatTime(value)}`);
}
