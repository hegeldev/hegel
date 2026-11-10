RELEASE_TYPE: minor

This release moves date and time generation into the engine, which draws time zones from this JVM's own zone rules or from a pinned IANA time zone database it bundles, gathers every date and time generator into the new `Generators.Temporals` class under the names every Hegel frontend shares, and adds `instants()`, `zonedDatetimes()` and `spans()` along with the date and time options every frontend shares. A few changes can break existing code or tests.

The date and time factories are now static methods of the nested class `Generators.Temporals`, so with `import static dev.hegel.Generators.*;` they are written `Temporals.dates()`, `Temporals.times()`, `Temporals.datetimes()`, `Temporals.zonedDatetimes()`, `Temporals.instants()`, `Temporals.offsets()` (a `ZoneOffset`), `Temporals.zones()` (a `ZoneId`), `Temporals.durations()` (a `Duration`) and `Temporals.spans()` (a `Period`). Their generator classes and enums move to the package `dev.hegel.generators.temporal`, named after the factories: `DateGenerator`, `TimeGenerator`, `DateTimeGenerator`, `ZonedDateTimeGenerator`, `InstantGenerator`, `OffsetGenerator` (was `dev.hegel.generators.ZoneOffsetGenerator`), `ZoneGenerator`, `DurationGenerator`, `SpanGenerator`, `OffsetGranularity` and `Ambiguous`. The top-level `Generators.dates()`, `times()`, `datetimes()`, `zoneOffsets()`, `zoneIds()` and `durations()`, and `DateTimeGenerator.timezones(...)` (now `zones(...)`), keep working as deprecated aliases, but code that names one of the moved classes needs its import updated, and code compiled against an earlier version needs recompiling, since the aliases' return types moved with the classes:

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

DateGenerator days = Temporals.dates().min(LocalDate.of(2020, 1, 1));
OffsetGenerator offsets = Temporals.offsets();
Generator<ZonedDateTime> zoned = Temporals.datetimes().zones(Temporals.zones());
```

`Temporals.durations()` spans the whole signed range `Duration` represents, from `Long.MIN_VALUE` to `Long.MAX_VALUE` seconds, shrinking toward `Duration.ZERO`. Most of that range is beyond what `Duration.toNanos()` and `toMillis()` can return, so code that converts wants bounds. The deprecated `Generators.durations()` still draws what it always did, a non-negative duration of at most `Long.MAX_VALUE` nanoseconds (about 292 years), and this is the same generator under the new name:

```java
// before
Generator<Duration> gen = durations();

// after
Generator<Duration> gen = Temporals.durations().min(Duration.ZERO).max(Duration.ofNanos(Long.MAX_VALUE));
```

The constructor `DurationGenerator(long minNanos, long maxNanos)` is gone; use `new DurationGenerator(Duration min, Duration max)` or the `min` and `max` methods.

Every generator of a value with a date in it draws years 1 to 9999 by default: `dates()`, `datetimes()`, `zonedDatetimes()`, `instants()`, and the `LocalDate`, `LocalDateTime`, `OffsetDateTime`, `ZonedDateTime` and `Instant` fields `Generators.forType` and record derivation fill in. Explicit bounds reach the engine's whole range of years `[-999999, 999999]`. A bound that is not set stays at the default, so a lower bound beyond the year 9999 needs the upper bound set first, as in `dates().max(LocalDate.of(12999, 12, 31)).min(LocalDate.of(12000, 1, 1))`; `dates().min(LocalDate.of(12000, 1, 1))` alone is an `IllegalArgumentException` that says so.

`zones()` (and the deprecated `zoneIds()`) returns a `ZoneGenerator` rather than a `Generator<ZoneId>`, and draws the names `ZoneId.getAvailableZoneIds()` lists; link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default. Code holding the result in a `Generator<ZoneId>` variable still compiles; code that relied on legacy names appearing asks for them:

```java
Generator<ZoneId> zones = Temporals.zones().allowLegacy(true);
Generator<ZoneId> canonical = Temporals.zones().allowLinks(false);
Generator<ZoneId> some = Temporals.zones().names("Europe/London", "Asia/Kolkata");
```

Half the zone draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks toward `UTC` through the zones a reader recognises, rather than alphabetically.

The new `zonedDatetimes()` draws a `ZonedDateTime` as a zone and a wall-clock time in one engine draw, and `datetimes().zones(zones)` now returns one too: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch. A repeated wall time is drawn as either instant by default, or as the one `ambiguous(Ambiguous.EARLIER)` or `LATER` asks for, and a skipped wall time, which a `ZonedDateTime` cannot hold, is never drawn. Bounds are either on the wall clock (`min`/`max` of a `LocalDateTime`) or on the instant (`min`/`max` of an `Instant` or a `ZonedDateTime`); setting both kinds on one generator is an `IllegalArgumentException`.

```java
Generator<ZonedDateTime> transitions = Temporals.zonedDatetimes()
        .zones(Temporals.zones().names("America/New_York"))
        .min(LocalDateTime.of(2024, 1, 1, 0, 0))
        .max(LocalDateTime.of(2024, 12, 31, 23, 59))
        .ambiguous(Ambiguous.LATER);
```

The engine targets each zone's transitions using the zone data of a `ZoneSource`. The default, `ZoneSource.javaTime()`, uploads this JVM's own `ZoneRules` the first time a generator needs them, so drawn values agree exactly with `java.time`. `ZoneSource.bundled()` is the engine's pinned IANA release (whose version `Temporals.tzdbVersion()` reports), which aims draws at the same transitions on every JVM; a drawn value still means what this JVM's rules say, and a wall-clock time this JVM skips rejects the test case rather than failing it. `ZoneSource.builder()` assembles a source from other data, to aim draws at transitions this JVM's database does not have yet or to draw from fewer zones: TZif bytes, `ZoneRules`, a function from instants to offsets, links, and names declared up front and loaded on first use. Building a source uploads its data, so build one once, in a static field, and share it; `version()` reports the release a source holds:

```java
static final ZoneSource VENDOR = ZoneSource.builder()
        .rules("America/New_York", vendorRules)
        .version("vendor-3")
        .build();

Generator<ZoneId> names = Temporals.zones().source(VENDOR);
Generator<ZonedDateTime> zoned = Temporals.zonedDatetimes().source(VENDOR);
```

`zones().source(source)` and `zonedDatetimes().source(source)` set the source of one generator, and `ZoneSource.setDefault(source)` the default for the process, returning the default it replaced so that a caller can put it back. A generator reads the default when it is created and keeps that source afterwards. `zonedDatetimes()` uses the source of the `zones()` generator it is given, and giving it both such a generator and `source(...)` is an `IllegalArgumentException`. The zones of any other generator, such as `just(zone)` or a filtered `zones()`, are looked up by name in the source given to `source(...)`, or else in the default, and a region zone that source does not know is an `IllegalArgumentException` naming it. A `ZoneOffset` needs no zone data and is drawn in directly:

```java
Generator<ZoneId> american = Temporals.zones().source(VENDOR).filter(z -> z.getId().startsWith("America/"));
Generator<ZonedDateTime> zoned = Temporals.zonedDatetimes().zones(american).source(VENDOR);
Generator<ZonedDateTime> fixed = Temporals.zonedDatetimes().zones(just(ZoneOffset.ofHours(5)));
```

`dates()` takes `weekdays(DayOfWeek...)` and `months(Month...)` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering, and targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average. `times()`, `datetimes()`, `instants()`, `durations()` and `zonedDatetimes()` take a `precision(ChronoUnit)` of `NANOS` (the default), `MICROS`, `MILLIS`, `SECONDS`, `MINUTES` or `HOURS`, so every drawn value is a whole number of that unit. `offsets()` takes a `granularity(OffsetGranularity)` of hours, quarter-hours, minutes or seconds and targets the offsets real zones have used. The new `instants()` draws `Instant` values, and `spans()` draws `Period` values with each of years, months and days independently signed, or `nonNegative(true)`; enabling a unit replaces the default of all three. By default each unit spans the whole `int` range, which `LocalDate.plus(Period)` overflows for most draws, so a span that will be added to a date wants bounds:

```java
Generator<Period> stays = Temporals.spans().days(1, 30).months(0, 11).nonNegative(true);
Generator<Instant> recent =
        Temporals.instants().min(Instant.parse("2020-01-01T00:00:00Z")).precision(ChronoUnit.MILLIS);
```

Every one of these generators reports a bad argument as an `IllegalArgumentException` from the method that was given it, as the date and time generators always did, never from the first draw: bounds in the wrong order, bounds with no date on an allowed weekday and month or no value at the precision, a zone name the zone source does not know, a zone source whose loader fails. Since each check sees the generator as it stands, `zones().names(...)` takes names of the source already set, so `source(...)` comes before `names(...)` when the names are not in the default source.

`Generators.forType` and record derivation now cover `Instant`, `Period`, `Year`, `YearMonth` and `MonthDay` as well as `LocalDate`, `LocalTime`, `LocalDateTime`, `ZonedDateTime`, `OffsetDateTime`, `ZoneId`, `ZoneOffset` and `Duration`; `DayOfWeek` and `Month` draw as any enum does.

The low-level `Libhegel` binding contract gains the engine's new functions: `generateDate` takes a date options handle and `generateTime` and `generateDatetime` a precision, and `generateTimestamp`, `generateOffset`, `generateDuration`, `validateDate`, `validateTime`, `validateDatetime`, `validateTimestamp`, `validateOffset`, `validateDuration`, `generateTimezone`, `generateZonedDatetime`, `generateZonedDatetimeForZone`, `generateSpan`, the `dateOptions*`, `timezoneSpec*`, `zonedSpec*` and `spanSpec*` handle functions (each spec with a `*Validate`), `timezoneInfo`, `timezoneOffsetAt`, `timezoneResolve`, the `zoneSource*` functions (`zoneSourceSetLoader` takes a hook run when the engine releases the loader), `setDefaultZoneSource` and `tzdbVersion` are new. A custom binding needs to implement them.
