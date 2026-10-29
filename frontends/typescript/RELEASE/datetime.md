RELEASE_TYPE: minor

This release changes `gs.dates()`, `gs.times()` and `gs.datetimes()` to draw value objects instead of ISO strings, and adds generators for the other date and time values the engine now draws: instants, UTC offsets, time zones, zoned datetimes, durations and JavaScript `Date`s.

The new classes `PlainDate`, `PlainTime`, `PlainDateTime`, `Instant`, `ZonedDateTime` and `Duration`, exported from `@hegeldev/hegel`, are immutable property bags shaped like their `Temporal` namesakes, so on a host with `Temporal` they convert with `Temporal.PlainDate.from(value)` and so on, and their `toString()` prints the RFC 9557 form `Temporal` prints. A test that compared the old strings keeps working by calling `toString()`:

```ts
// before
const date = tc.draw(gs.dates()); // "2024-03-10"

// after
const date = tc.draw(gs.dates()); // PlainDate { year: 2024, month: 3, day: 10 }
date.toString(); // "2024-03-10"
Temporal.PlainDate.from(date); // on a host with Temporal
```

Dates now span `Temporal.PlainDate`'s whole range by default (`-271821-04-19` to `+275760-09-13`; a year outside 0000 to 9999 prints with six signed digits) rather than the years 1 to 9999, and all three generators take options: `minValue` and `maxValue` bounds (the class, a `Temporal` value or a plain `{ year, month, day }`), `weekdays` and `months` for `gs.dates()`, and `precision` (`"nanosecond"` by default, up to `"hour"`) for `gs.times()` and `gs.datetimes()`. The draws also target the calendar's own interesting values, such as the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days, ISO week 53 and the last nanosecond of a day, so a short run reaches them. Seeded runs generate different values from this version on.

The new generators:

- `gs.instants({ minValue, maxValue, precision })` draws `Instant`s (`epochNanoseconds` as a `bigint`, with `epochSeconds` and `epochMilliseconds` as numbers) over `Temporal.Instant`'s range by default, and `gs.jsDates({ minValue, maxValue })` draws JavaScript `Date`s, every valid one by default.
- `gs.utcOffsets({ minValue, maxValue, granularity })` draws offsets as `±HH:MM` strings (`±HH:MM:SS` at `"second"` granularity), which are valid `Temporal` time zone identifiers.
- `gs.timeZones({ names, links, legacy })` draws IANA names from the engine's bundled database, restricted to the names the host's `Intl` also knows, so `Temporal.ZonedDateTime` and `Intl.DateTimeFormat` accept every value. Link names such as `Asia/Calcutta` are included by default; legacy names such as `EST5EDT` are not.
- `gs.zonedDateTimes({ timeZones, minValue, maxValue, minInstant, maxInstant, precision, allowImaginary, ambiguous })` draws `ZonedDateTime`s: the wall-clock fields, `timeZone`, `offset` and `epochNanoseconds`, plus `kind` (`"unique"`, `"gap"` or `"fold"`) with `fold`, `offsetBefore` and `offsetAfter`, so a test sees what happens at a daylight-saving transition; one draw in twenty targets one. `Temporal.ZonedDateTime.from(value, { offset: "use" })` converts a value, and the class documents how it reads a wall-clock time a transition skipped. A draw whose offset the host's own time zone database disagrees with is rejected like a failed assumption.
- `gs.durations({ years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds, sign })` draws `Duration`s whose units all share one sign, as `Temporal.Duration.from(value)` requires, with `{ min, max }` bounds per unit; naming any unit draws only the named ones.

Everything shrinks toward a readable value: 2000-01-01, midnight, `UTC`, `+00:00` and `PT0S`.
