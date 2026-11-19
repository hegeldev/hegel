RELEASE_TYPE: minor

This release moves the date and time generators into a new `gs.temporal` namespace, changes them to draw value objects instead of ISO strings, and adds generators for the other date and time values the engine now draws: zoned datetimes, instants, UTC offsets, time zones, calendar spans and JavaScript `Date`s.

`gs.dates()`, `gs.times()` and `gs.datetimes()` are now `gs.temporal.dates()`, `gs.temporal.times()` and `gs.temporal.datetimes()`. The old names are deprecated, and still draw the same ISO 8601 strings as before. The new generators draw the new classes `PlainDate`, `PlainTime` and `PlainDateTime`, which `gs.temporal` exports together with `Instant`, `ZonedDateTime` and `Duration`. Each is an immutable property bag shaped like its `Temporal` namesake. A test moving to the new generators gets the old string back from `toString()`:

```ts
// before
const date = tc.draw(gs.dates()); // "2024-03-10"

// after
const date = tc.draw(gs.temporal.dates()); // PlainDate { year: 2024, month: 3, day: 10 }
date.toString(); // "2024-03-10"
date instanceof gs.temporal.PlainDate; // true
Temporal.PlainDate.from(date); // on a host with Temporal
```

On a host with `Temporal`, a value converts with `Temporal.PlainDate.from(value)`, `Temporal.PlainTime.from(value)`, `Temporal.PlainDateTime.from(value)`, `Temporal.Duration.from(value)`, `Temporal.Instant.fromEpochNanoseconds(value.epochNanoseconds)` and, for a `ZonedDateTime`, `value.toTemporal(Temporal)`. `Temporal.ZonedDateTime.from(value)` is not that conversion: it throws for a wall-clock time a transition skipped, and with `{ offset: "use" }` reads one as a different instant.

`toString()` and `toJSON()` give the text Hegel prints a value as, which is the same in every Hegel frontend, and a failing test reports a value as that text inside the class name, such as `PlainDate(2024-03-10)`. For a date, a time, a datetime and an instant the text is the ISO 8601 string `Temporal` prints. For the other two it is not something `Temporal` parses: a `ZonedDateTime` prints as `2024-03-10T01:30:00-05:00[America/New_York]` with `[gap]`, `[fold=0]` or `[fold=1]` appended at a transition, and a `Duration` as its units (`1y 2mo 3d`, `-1h 30m`, `0s`).

Dates now span `Temporal.PlainDate`'s whole range by default (`-271821-04-19` to `+275760-09-13`; a year outside 0000 to 9999 prints with six signed digits) rather than the years 1 to 9999, and all three generators take options: `minValue` and `maxValue` bounds (the class, a `Temporal` value in the ISO 8601 calendar or a plain `{ year, month, day }`), `weekdays` and `months` for `gs.temporal.dates()`, and `precision` (`"nanosecond"` by default, up to `"hour"`) for `gs.temporal.times()` and `gs.temporal.datetimes()`. The draws also target the calendar's own interesting values, such as the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days, ISO week 53 and the last nanosecond of a day, so a short run reaches them. Seeded runs generate different values from this version on.

The new generators:

- `gs.temporal.zonedDatetimes({ zones, source, minValue, maxValue, minInstant, maxInstant, precision, allowImaginary, ambiguous })` draws `ZonedDateTime`s: the wall-clock fields, `timeZone`, `offset` and `epochNanoseconds`, plus `kind` (`"unique"`, `"gap"` or `"fold"`) with `fold`, `offsetBefore` and `offsetAfter`, so a test sees what happens at a daylight-saving transition; one draw in twenty targets one. `zones` is any generator of IANA names, `gs.temporal.zones()` by default. The bounds are on the wall clock (`minValue`, `maxValue`) or on the instant (`minInstant`, `maxInstant`), and giving both kinds is an error. Each draw is made against a zone source (see below).
- `gs.temporal.instants({ minValue, maxValue, precision })` draws `Instant`s (`epochNanoseconds` as a `bigint`, with `epochMilliseconds` as a number) over `Temporal.Instant`'s range by default, and `gs.temporal.jsDates({ minValue, maxValue })` draws JavaScript `Date`s, every valid one by default. A `PlainDateTime` is not accepted where an instant is expected.
- `gs.temporal.offsets({ minValue, maxValue, granularity })` draws UTC offsets as `±HH:MM` strings, which are valid `Temporal` time zone identifiers, or as `±HH:MM:SS` strings at `"second"` granularity, which are not.
- `gs.temporal.zones({ names, allowLinks, allowLegacy, source })` draws IANA time zone names. By default these are the names of the engine's bundled catalogue that the host's `Intl` also knows, so `Temporal.ZonedDateTime` and `Intl.DateTimeFormat` accept every value. Link names such as `Asia/Calcutta` are included by default; legacy names such as `EST5EDT` are not.
- `gs.temporal.spans({ years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds, nanoseconds, sign })` draws calendar spans as `Duration`s whose units all share one sign, as `Temporal.Duration.from(value)` requires, with `{ minValue, maxValue }` bounds per unit; naming any unit draws only the named ones.

Zoned draws are made against a zone source, `gs.temporal.ZoneSource`: the time zone data whose transitions they aim at and whose offsets they resolve with. By default this is the host's own database, the one `Temporal` and `Intl` use (`ZoneSource.host()`), so a gap or fold Hegel draws is a gap or fold for the code under test. Each zone's data is read the first time a draw lands on it, once per process. Where the host has `Temporal` (Node 26, for example), the data comes exactly from `Temporal.ZonedDateTime#getTimeZoneTransition`, which takes well under a millisecond per zone. Otherwise it is probed from `Intl.DateTimeFormat`, which is much slower: expect on the order of a hundred milliseconds for each zone a run draws. Either way it covers 1800 to 2100, and later years follow the yearly rule in force at the end of that range. As a safety net, a draw whose offset `Intl` disagrees with is still rejected like a failed assumption. Two other sources are available:

- `ZoneSource.bundled()` is the IANA database the engine bundles, which is the same on every host.
- `ZoneSource.custom({ zones, names, load, version })` is a source for the database of another date library. A zone can be given as the bytes of a TZif file, as structured data (`initial`, `transitions` and a yearly `rule`, or `inferAfter` to infer the rule), as an offset oracle (`offsetAt`, plus an optional `nextTransition`), or as a link to another name. `load` supplies a name's data lazily.

`gs.temporal.zones({ source })` and `gs.temporal.zonedDatetimes({ source })` pick the source for one generator. `source` on `zonedDatetimes()` is the source of its default zones and the one the names of any other `zones` generator (a filtered `zones()`, `gs.sampledFrom([...])`) are resolved against; a `zones()` generator passed as `zones` brings its own, so `source` may not be given with one. A name the source does not know is an error.

```ts
const source = gs.temporal.ZoneSource.bundled();
const american = gs.temporal.zonedDatetimes({
  zones: gs.temporal.zones({ source }).filter((zone) => zone.startsWith("America/")),
  source,
});
```

`ZoneSource.setDefault(source)` changes the default for the generators built after it and returns the source it replaces. A generator reads the default when it is built, not when it draws, so build sources and set the default at module level or in test setup rather than inside a test body. Each source has a `version`.

An argument no draw could satisfy is an error raised when the generator is built, never later by a draw: bounds of the wrong shape or in the wrong order, bounds that hold no value at the precision or no date on the allowed weekdays, an empty set of weekdays or months, a name given to `zones()` that its source does not know. The ones the engine checks carry the engine's own message. Only a name that another generator of zones yields is checked when it is drawn.

`gs.temporal.tzdbVersion()` returns the IANA release of the time zone database the engine bundles, such as `"2026e"`. Importing `@hegeldev/hegel/generators` now loads the engine itself, as importing `@hegeldev/hegel` does, so this works when a module imports only the generators.

The option types (`DateOptions`, `TimeOptions`, `DatetimeOptions`, `ZonedDatetimeOptions`, `InstantOptions`, `OffsetOptions`, `ZoneOptions`, `SpanOptions`, `UnitBounds`, `JsDateOptions`), the zone source types (`ZoneSourceOptions`, `ZoneDefinition`, `ZoneData`, `ZoneType`, `ZoneTransition`, `ZoneRule`, `RuleDate`, `ZoneOracle`), the `Precision`, `Granularity` and `Ambiguous` types, and the value classes' `*Like` field types, `TransitionKind`, `DurationUnit` and `DURATION_UNITS` are exported from `gs.temporal` too.

Everything shrinks toward a readable value: 2000-01-01, midnight, `UTC`, `+00:00` and a zero span.
