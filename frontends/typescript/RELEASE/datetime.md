RELEASE_TYPE: minor

This release moves the date and time generators into a new `gs.temporal` namespace, changes them to draw value objects instead of ISO strings, and adds generators for the other date and time values the engine now draws: zoned datetimes, instants, UTC offsets, time zones, calendar spans and JavaScript `Date`s.

`gs.dates()`, `gs.times()` and `gs.datetimes()` are now `gs.temporal.dates()`, `gs.temporal.times()` and `gs.temporal.datetimes()`. The old names are deprecated, and still draw the same ISO 8601 strings as before. The new generators draw the new classes `PlainDate`, `PlainTime` and `PlainDateTime`, which `gs.temporal` exports together with `Instant`, `ZonedDateTime` and `Duration`. Each is an immutable property bag shaped like its `Temporal` namesake, so on a host with `Temporal` it converts with `Temporal.PlainDate.from(value)` and so on, and its `toString()` prints the RFC 9557 form `Temporal` prints. A test moving to the new generators gets an ISO string back from `toString()`:

```ts
// before
const date = tc.draw(gs.dates()); // "2024-03-10"

// after
const date = tc.draw(gs.temporal.dates()); // PlainDate { year: 2024, month: 3, day: 10 }
date.toString(); // "2024-03-10"
date instanceof gs.temporal.PlainDate; // true
Temporal.PlainDate.from(date); // on a host with Temporal
```

Dates now span `Temporal.PlainDate`'s whole range by default (`-271821-04-19` to `+275760-09-13`; a year outside 0000 to 9999 prints with six signed digits) rather than the years 1 to 9999, and all three generators take options: `minValue` and `maxValue` bounds (the class, a `Temporal` value or a plain `{ year, month, day }`), `weekdays` and `months` for `gs.temporal.dates()`, and `precision` (`"nanosecond"` by default, up to `"hour"`) for `gs.temporal.times()` and `gs.temporal.datetimes()`. The draws also target the calendar's own interesting values, such as the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days, ISO week 53 and the last nanosecond of a day, so a short run reaches them. Seeded runs generate different values from this version on.

The new generators:

- `gs.temporal.zonedDateTimes({ zones, minValue, maxValue, minInstant, maxInstant, precision, allowImaginary, ambiguous })` draws `ZonedDateTime`s: the wall-clock fields, `timeZone`, `offset` and `epochNanoseconds`, plus `kind` (`"unique"`, `"gap"` or `"fold"`) with `fold`, `offsetBefore` and `offsetAfter`, so a test sees what happens at a daylight-saving transition; one draw in twenty targets one. `zones` is any generator of IANA names, `gs.temporal.zones()` by default. `Temporal.ZonedDateTime.from(value, { offset: "use" })` converts a value, and the class documents how it reads a wall-clock time a transition skipped. Each draw is made against a zone source (see below).
- `gs.temporal.instants({ minValue, maxValue, precision })` draws `Instant`s (`epochNanoseconds` as a `bigint`, with `epochSeconds` and `epochMilliseconds` as numbers) over `Temporal.Instant`'s range by default, and `gs.temporal.jsDates({ minValue, maxValue })` draws JavaScript `Date`s, every valid one by default.
- `gs.temporal.offsets({ minValue, maxValue, granularity })` draws UTC offsets as `±HH:MM` strings (`±HH:MM:SS` at `"second"` granularity), which are valid `Temporal` time zone identifiers.
- `gs.temporal.zones({ names, links, legacy, source })` draws IANA time zone names. By default these are the names of the engine's bundled catalogue that the host's `Intl` also knows, so `Temporal.ZonedDateTime` and `Intl.DateTimeFormat` accept every value. Link names such as `Asia/Calcutta` are included by default; legacy names such as `EST5EDT` are not.
- `gs.temporal.spans({ years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds, sign })` draws calendar spans as `Duration`s whose units all share one sign, as `Temporal.Duration.from(value)` requires, with `{ min, max }` bounds per unit; naming any unit draws only the named ones.

Zoned draws are made against a zone source, `gs.temporal.ZoneSource`: the time zone data whose transitions they aim at and whose offsets they resolve with. By default this is the host's own database, the one `Temporal` and `Intl` use (`ZoneSource.host()`), so a gap or fold Hegel draws is a gap or fold for the code under test. Each zone's data is read the first time a draw lands on it, once per process. Where the host has `Temporal`, the data comes exactly from `Temporal.ZonedDateTime#getTimeZoneTransition` (well under a millisecond per zone). Otherwise it is probed from `Intl.DateTimeFormat` (around 20 to 40 ms per zone). Either way it covers 1800 to 2100, and later years follow the yearly rule in force at the end of that range. As a safety net, a draw whose offset `Intl` disagrees with is still rejected like a failed assumption. Two other sources are available:

- `ZoneSource.bundled()` is the engine's bundled IANA database, which was the default before this release.
- `ZoneSource.custom({ zones, names, load, version })` is a source for the database of another date library. A zone can be given as the bytes of a TZif file, as structured data (`initial`, `transitions` and a yearly `rule`, or `inferAfter` to infer the rule), as an offset oracle (`offsetAt`, plus an optional `nextTransition`), or as a link to another name. `load` supplies a name's data lazily.

`ZoneSource.setDefault(source)` changes the default, and `gs.temporal.zones({ source })` picks a source for one generator; `gs.temporal.zonedDateTimes()` draws against its `zones` generator's source. Each source has a `version`.

`gs.temporal.tzdbVersion()` returns the IANA release of the time zone database the engine bundles, such as `"2026e"`. Importing `@hegeldev/hegel/generators` now loads the engine itself, as importing `@hegeldev/hegel` does, so this works when a module imports only the generators.

The option types (`DateOptions`, `TimeOptions`, `DateTimeOptions`, `ZonedDateTimeOptions`, `InstantOptions`, `OffsetOptions`, `ZoneOptions`, `SpanOptions`, `UnitBounds`, `JsDateOptions`), the zone source types (`ZoneSourceOptions`, `ZoneDefinition`, `ZoneData`, `ZoneType`, `ZoneTransition`, `ZoneRule`, `RuleDate`, `ZoneOracle`), the `Precision`, `Granularity` and `Ambiguous` types, and the value classes' `*Like` field types, `TransitionKind`, `DurationUnit` and `DURATION_UNITS` are exported from `gs.temporal` too.

Everything shrinks toward a readable value: 2000-01-01, midnight, `UTC`, `+00:00` and `PT0S`.
