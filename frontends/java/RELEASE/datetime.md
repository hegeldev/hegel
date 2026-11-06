RELEASE_TYPE: minor

This release moves date and time generation into the engine, which draws time zones from this JVM's own zone rules or from a pinned IANA time zone database it bundles, gathers every date and time generator into the new `Generators.Temporal` class under the names every Hegel frontend shares, and adds `instants()`, `zonedDateTimes()` and `spans()` along with the date and time options every frontend shares. A few changes can break existing code or tests.

The date and time factories are now static methods of the nested class `Generators.Temporal`, so with `import static dev.hegel.Generators.*;` they are written `Temporal.dates()`, `Temporal.times()`, `Temporal.datetimes()`, `Temporal.zonedDateTimes()`, `Temporal.instants()`, `Temporal.offsets()` (a `ZoneOffset`), `Temporal.zones()` (a `ZoneId`), `Temporal.durations()` (a `Duration`) and `Temporal.spans()` (a `Period`). Their generator classes and enums move to the package `dev.hegel.generators.temporal`, named after the factories: `DateGenerator`, `TimeGenerator`, `DateTimeGenerator`, `ZonedDateTimeGenerator`, `InstantGenerator`, `OffsetGenerator` (was `dev.hegel.generators.ZoneOffsetGenerator`), `ZoneGenerator`, `DurationGenerator`, `SpanGenerator`, `OffsetGranularity` and `Ambiguous`. The top-level `Generators.dates()`, `times()`, `datetimes()`, `zoneOffsets()`, `zoneIds()` and `durations()`, and `DateTimeGenerator.timezones(...)` (now `zones(...)`), keep working as deprecated aliases, but code that names one of the moved classes needs its import updated:

```java
// before
import static dev.hegel.Generators.*;
import dev.hegel.generators.DateGenerator;
import dev.hegel.generators.ZoneOffsetGenerator;

DateGenerator days = dates().min(LocalDate.of(2020, 1, 1));
ZoneOffsetGenerator offsets = zoneOffsets();
Generator<ZonedDateTime> zoned = datetimes().timezones(zoneIds());

// after
import static dev.hegel.Generators.*;
import dev.hegel.generators.temporal.DateGenerator;
import dev.hegel.generators.temporal.OffsetGenerator;

DateGenerator days = Temporal.dates().min(LocalDate.of(2020, 1, 1));
OffsetGenerator offsets = Temporal.offsets();
Generator<ZonedDateTime> zoned = Temporal.datetimes().zones(Temporal.zones());
```

`durations()` now spans the whole signed range `Duration` represents, from `Long.MIN_VALUE` to `Long.MAX_VALUE` seconds, shrinking toward `Duration.ZERO`; it used to draw non-negative durations only. A test that relied on that gets it back with a bound:

```java
// before
Generator<Duration> gen = durations();

// after
Generator<Duration> gen = Temporal.durations().min(Duration.ZERO);
```

`zones()` (and the deprecated `zoneIds()`) returns a `ZoneGenerator` rather than a `Generator<ZoneId>`, and draws the names `ZoneId.getAvailableZoneIds()` lists; link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default. Code holding the result in a `Generator<ZoneId>` variable still compiles; code that relied on legacy names appearing asks for them:

```java
Generator<ZoneId> zones = Temporal.zones().legacy(true);
Generator<ZoneId> some = Temporal.zones().names("Europe/London", "Asia/Kolkata");
```

Half the zone draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks down a ladder from `UTC` through the zones a reader recognises rather than alphabetically.

The new `zonedDateTimes()` draws a `ZonedDateTime` as a zone and a wall-clock time in one engine draw, and `datetimes().zones(zones)` now returns one too: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch. A repeated wall time is drawn as either instant by default, or as the one `ambiguous(Ambiguous.EARLIER)` or `LATER` asks for, and a skipped wall time, which a `ZonedDateTime` cannot hold, is never drawn. Bounds are on the wall clock (`min`/`max` of a `LocalDateTime`) or on the instant (`min`/`max` of an `Instant` or a `ZonedDateTime`); setting one kind replaces the other.

The engine targets each zone's transitions using the zone data of a `ZoneSource`. The default, `ZoneSource.javaTime()`, uploads this JVM's own `ZoneRules` the first time a draw needs a zone, so drawn values agree exactly with `java.time`. `ZoneSource.bundled()` is the engine's pinned IANA release (whose version `Temporal.tzdbVersion()` reports), which replays a saved failure the same way on every JVM; with it, a draw that the JVM resolves to a different instant rejects the test case rather than failing it. `ZoneSource.builder()` assembles a source from a library's own data: TZif bytes, `ZoneRules`, a function from instants to offsets, links, and names declared up front and loaded on first use. `ZoneSource.setDefault(source)` changes the source for the process and `zones().source(source)` for one generator; `version()` reports the release a source holds:

```java
ZoneSource.setDefault(ZoneSource.bundled());
Generator<ZoneId> mine = Temporal.zones().source(ZoneSource.builder()
        .rules("America/New_York", vendorRules)
        .version("vendor-3")
        .build());
```

```java
Generator<ZonedDateTime> transitions = Temporal.zonedDateTimes()
        .zones(Temporal.zones().names("America/New_York"))
        .min(LocalDateTime.of(2024, 1, 1, 0, 0))
        .max(LocalDateTime.of(2024, 12, 31, 23, 59))
        .ambiguous(Ambiguous.LATER);
```

`dates()` takes `weekdays(DayOfWeek...)` and `months(Month...)` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering, and targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average. `times()`, `datetimes()`, `instants()`, `durations()` and `zonedDateTimes()` take a `precision(ChronoUnit)` of `NANOS` (the default), `MICROS`, `MILLIS`, `SECONDS`, `MINUTES` or `HOURS`, so every drawn value is a whole number of that unit. `offsets()` takes a `granularity(OffsetGranularity)` of hours, quarter-hours, minutes or seconds and targets the offsets real zones have used. The new `instants()` draws `Instant` values across the engine's range of years `[-999999, 999999]`, and `spans()` draws `Period` values with each of years, months and days independently signed, or `nonNegative(true)`; enabling a unit replaces the default of all three:

```java
Generator<Period> stays = Temporal.spans().days(1, 30).months(0, 11).nonNegative(true);
Generator<Instant> recent =
        Temporal.instants().min(Instant.parse("2020-01-01T00:00:00Z")).precision(ChronoUnit.MILLIS);
```

`Generators.forType` and record derivation now cover `Instant`, `Period`, `Year`, `YearMonth` and `MonthDay` as well as `LocalDate`, `LocalTime`, `LocalDateTime`, `ZonedDateTime`, `OffsetDateTime`, `ZoneId`, `ZoneOffset` and `Duration`; `DayOfWeek` and `Month` draw as any enum does.

The low-level `Libhegel` binding contract gains the engine's new functions: `generateDate` takes a date options handle and `generateTime` and `generateDatetime` a precision, and `generateTimestamp`, `generateOffset`, `generateDuration`, `generateTimezone`, `generateZonedDatetime`, `generateSpan`, the `dateOptions*`, `timezoneSpec*`, `zonedSpec*` and `spanSpec*` handle functions, `timezoneInfo`, `timezoneOffsetAt`, `timezoneResolve`, the `zoneSource*` functions, `setDefaultZoneSource` and `tzdbVersion` are new. A custom binding needs to implement them.
