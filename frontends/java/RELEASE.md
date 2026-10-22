RELEASE_TYPE: minor

This release moves date and time generation into the engine, which now bundles a pinned IANA time zone database, and adds `instants()`, `zonedDateTimes()` and `periods()` along with the date and time options every Hegel frontend shares. Two existing generators change in ways that can break a test.

`durations()` now spans the whole signed range `Duration` represents, from `Long.MIN_VALUE` to `Long.MAX_VALUE` seconds, shrinking toward `Duration.ZERO`; it used to draw non-negative durations only. A test that relied on that gets it back with a bound:

```java
// before
Generator<Duration> gen = durations();

// after
Generator<Duration> gen = durations().min(Duration.ZERO);
```

`zoneIds()` now returns a `ZoneIdGenerator` rather than a `Generator<ZoneId>`, and draws the names of the engine's bundled database that `ZoneId.getAvailableZoneIds()` also lists, so a saved failure means the same zone on every machine; link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default. Code holding the result in a `Generator<ZoneId>` variable still compiles; code that relied on legacy names appearing asks for them:

```java
Generator<ZoneId> zones = zoneIds().legacy(true);
Generator<ZoneId> some = zoneIds().names("Europe/London", "Asia/Kolkata");
```

Half the zone draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks down a ladder from `UTC` through the zones a reader recognises rather than alphabetically.

The new `zonedDateTimes()` draws a `ZonedDateTime` as a zone and a wall-clock time in one engine draw, and `datetimes().timezones(zones)` now returns one too: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch. A repeated wall time is drawn as either instant by default, or as the one `ambiguous(Ambiguous.EARLIER)` or `LATER` asks for, and a skipped wall time is avoided by default or reached deliberately with `allowImaginary(true)`. Bounds are on the wall clock (`min`/`max` of a `LocalDateTime`) or on the instant (`min`/`max` of an `Instant` or a `ZonedDateTime`); setting one kind replaces the other. The value is built with the JVM's own zone rules, and a draw the JVM and the engine resolve to different instants, because their time zone databases differ, rejects the test case rather than failing it; `Generators.tzdbVersion()` reports the engine's IANA release.

```java
Generator<ZonedDateTime> transitions = zonedDateTimes()
        .zones(zoneIds().names("America/New_York"))
        .min(LocalDateTime.of(2024, 1, 1, 0, 0))
        .max(LocalDateTime.of(2024, 12, 31, 23, 59))
        .ambiguous(Ambiguous.LATER);
```

`dates()` takes `weekdays(DayOfWeek...)` and `months(Month...)` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering, and targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average. `times()`, `datetimes()`, `instants()`, `durations()` and `zonedDateTimes()` take a `precision(ChronoUnit)` of `NANOS` (the default), `MICROS`, `MILLIS`, `SECONDS`, `MINUTES` or `HOURS`, so every drawn value is a whole number of that unit. `zoneOffsets()` takes a `granularity(Granularity)` of hours, quarter-hours, minutes or seconds and targets the offsets real zones have used. The new `instants()` draws `Instant` values across the engine's range of years `[-999999, 999999]`, and `periods()` draws `Period` values with each of years, months and days independently signed, or `nonNegative(true)`; enabling a unit replaces the default of all three:

```java
Generator<Period> stays = periods().days(1, 30).months(0, 11).nonNegative(true);
Generator<Instant> recent = instants().min(Instant.parse("2020-01-01T00:00:00Z")).precision(ChronoUnit.MILLIS);
```

`Generators.forType` and record derivation now cover `Instant`, `Period`, `Year`, `YearMonth` and `MonthDay` as well as `LocalDate`, `LocalTime`, `LocalDateTime`, `ZonedDateTime`, `OffsetDateTime`, `ZoneId`, `ZoneOffset` and `Duration`; `DayOfWeek` and `Month` draw as any enum does.

The low-level `Libhegel` binding contract gains the engine's new functions: `generateDate` takes a date options handle and `generateTime` and `generateDatetime` a precision, and `generateTimestamp`, `generateOffset`, `generateDuration`, `generateTimezone`, `generateZonedDatetime`, `generateSpan`, the `dateOptions*`, `timezoneSpec*`, `zonedSpec*` and `spanSpec*` handle functions, `timezoneInfo` and `tzdbVersion` are new. A custom binding needs to implement them.
