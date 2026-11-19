RELEASE_TYPE: minor

This release moves date and time generation into the engine, which now bundles a pinned IANA time zone database (`gs::temporal::tzdb_version()` names the release), gathers it under the new module `hegel::generators::temporal`, and rebuilds the `jiff` and `chrono` integrations on it. Time zones, zoned datetimes, instants, offsets, durations and calendar spans are now engine draws with the engine's shrinking, a drawn zone is stored by name so a saved failure means the same zone on every machine, and one draw in twenty on average targets a transition of the zone or an interesting calendar value, so the skipped and repeated wall times where date bugs live are generated deliberately rather than by luck. These parts of it are breaking:

- `gs::durations()` and `gs::DurationGenerator` (over `std::time::Duration`) are deprecated in favour of `gs::temporal::std_durations()` and `gs::temporal::StdDurationGenerator`. The deprecated `gs::durations()` keeps the default range it had, zero to `u64::MAX` nanoseconds; `std_durations()` goes up to `i64::MAX` seconds by default, so bound it with `max_value` where a test adds the draw to an `Instant` or reads it as a `u64` of nanoseconds. Both now draw through the engine's duration draw and gain a `precision` builder.

  ```rust
  // before
  tc.draw(gs::durations().max_value(Duration::from_secs(60)))

  // after
  tc.draw(gs::temporal::std_durations().max_value(Duration::from_secs(60)))
  ```

- `hegel::extras::jiff::zoneds()` renames its `timezones(...)` builder to `zones(...)` and loses the `timestamps(impl Generator<Timestamp>)` builder: bound the draw with `min_value`/`max_value` (wall-clock) or `min_instant`/`max_instant` (instant) instead. `ZonedGenerator` no longer has type parameters. `zones` still takes any generator of `jiff::tz::TimeZone` that draws IANA-named or fixed-offset zones; any other zone, such as one built from a POSIX time zone string, is a usage error.

  ```rust
  // before
  jiff_gs::zoneds().timezones(gs::just(TimeZone::UTC)).timestamps(jiff_gs::timestamps().min_value(min))

  // after
  jiff_gs::zoneds().zones(gs::just(TimeZone::UTC)).min_instant(min)
  ```

- `hegel::extras::jiff::spans()` (and `gs::default::<jiff::Span>()`) draws every unit from years to nanoseconds rather than nanoseconds alone, so adding an unconstrained span to a `Timestamp`, which takes no calendar units, now fails. Name the units to draw with the new per-unit builders, such as `spans().nanoseconds(min, max)` or `spans().days(0, 30).hours(0, 23)`.

- `hegel::extras::jiff::datetimes()` draws jiff's whole range by default, `-9999-01-01T00:00:00` through `9999-12-31T23:59:59.999999999`, and the jiff generators print a counterexample with jiff's `Display` (`2024-03-10T02:30:00`) rather than its `Debug` form.

- `gs::default::<jiff::tz::TimeZone>()` draws an IANA zone or a fixed offset and no longer yields `TimeZone::unknown()`, which is behind `jiff_gs::zones().allow_unknown(true)`.

- `hegel::extras::chrono::datetimes()` renames its `timezones(...)` builder to `zones(...)`, which takes a generator from the sealed `TimeZones` trait rather than any generator of any `chrono::TimeZone`: every generator of `Utc` or `FixedOffset` (such as `gs::just(Utc)` and `fixed_offsets()`) still works, and with the new `chrono-tz` feature so does every generator of `chrono_tz::Tz`, but a generator of some other zone type such as `chrono::Local` no longer compiles, as the engine cannot classify wall times in it. A bound in chrono's leap-second representation (a `nanosecond()` of a billion or more) is now a usage error for `naive_times()`, `naive_datetimes()` and `datetimes()`, and no drawn time uses it.

- `gs::from_lark(...).explicit(name, generator)` no longer requires the generator to be `UnwindSafe + RefUnwindSafe`, so a `.boxed()` generator works there; in exchange `LarkGenerator` itself is no longer `UnwindSafe` or `RefUnwindSafe`. Wrap it in `std::panic::AssertUnwindSafe` where a closure passed to `catch_unwind` captures it.

`hegel::generators::temporal` holds value types mirroring the engine's model, for a test with any date library to wrap with `map`: `Date`, `Time`, `DateTime`, `Instant`, `UtcOffset`, `TimeZone`, `ZonedDateTime`, `SignedDuration` and `Span`. They are plain data with public fields and no arithmetic or parsing. `Date::new(2024, 3, 10)` and the other `new` constructors are `const` and check nothing, so a value that is not a real date can be written down, and a generator given one as a bound reports it as a usage error when it draws. Each prints as the engine prints a counterexample in every language, in RFC 9557 form: `2024-03-10T02:30:00-04:00[America/New_York][gap]`. Beside them are `TransitionKind` (whether a wall time is unique, skipped or repeated), the option enums `Precision`, `Granularity`, `Weekday`, `Month`, `Ambiguous`, `SpanSign` and `SpanUnit`, the constants `MIN_YEAR`, `MAX_YEAR`, `MIN_INSTANT`, `MAX_INSTANT`, `MAX_OFFSET` and `UNKNOWN_ZONE`, and the queries `resolve`, `offset_at` and `info`, which ask the default zone source outside any test and return `ZonedDateTime`, `ZoneOffset` and `ZoneInfo` (with its `Tag`s), or an `Error`. The enums that mirror an engine enumeration (`Precision`, `Granularity`, `Ambiguous`, `SpanSign`, `SpanUnit` and `Tag`) are `#[non_exhaustive]`.

The generators over them are `dates()` with `weekdays` and `months` masks, `times()` and `datetimes()` with a `precision`, `instants()`, `offsets()` with a `granularity` and an `allow_unknown` switch for RFC 3339's `-00:00`, `zones()` over the names of a zone source with `source`, `names`, `allow_links`, `allow_legacy`, `allow_unknown` and `allow_opaque` builders, `zoned_datetimes()`, `durations()` over `SignedDuration` and `spans()` with per-unit bounds and a sign policy, plus `std_durations()` and `system_times()` over `std::time::Duration` and `std::time::SystemTime`. Every one of these types, and `SystemTime`, has a `DefaultGenerator` impl.

`zoned_datetimes()` draws a zone, a wall-clock time in it and, for a repeated wall time, which repeat is meant. It takes wall-clock bounds (`min_value`, `max_value`) or instant bounds (`min_instant`, `max_instant`), and giving both kinds is a usage error; `allow_imaginary` and `ambiguous` say what to do with skipped and repeated wall times. Its `zones` builder takes any generator of `TimeZone`. Given `zones()` itself, the zone and the wall time are one engine draw against that generator's zone source. Any other generator, such as `gs::sampled_from(...)` or `zones()` boxed, mapped or filtered, draws the zone first and then a wall time in it, looking the zone's name up in the source given to the `source` builder, or in the default zone source otherwise. A name that source does not know is a usage error that says to pass the right `source`.

```rust
use hegel::generators as gs;
use hegel::generators::temporal::{Precision, TransitionKind};

#[hegel::test]
fn my_test(tc: hegel::TestCase) {
    let z = tc.draw(gs::temporal::zoned_datetimes()
        .zones(gs::temporal::zones().names(["Europe/London", "America/New_York"]))
        .precision(Precision::Seconds)
        .allow_imaginary(true));
    assert!(z.offset_before == z.offset_after || z.kind != TransitionKind::Unique);
}
```

Zones are drawn, and wall times classified as unique, skipped or repeated, against a `ZoneSource`: the database the code under test uses, so the gaps and folds the engine targets are that database's own. Rust has no standard database, so the default is the engine's bundled one (`ZoneSource::bundled()`). `default_zone_source()` returns the current default and `set_default_zone_source(source)` replaces it for the process, returning the one it replaced. A generator reads the default when it is built, by `zones()` or `zoned_datetimes()`, and keeps that source if the default changes afterwards. `zones().source(...)` and `zoned_datetimes().source(...)` draw against another source for one generator. The jiff integration's generators draw against `hegel::extras::jiff::zone_source()`, whichever database jiff itself reads (the system's zoneinfo or its bundled copy), and the chrono-tz ones against `hegel::extras::chrono::zone_source()`, chrono-tz's compiled-in release, so neither rejects draws where its database and the engine's disagree any more.

```rust
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;

#[hegel::test]
fn my_test(tc: hegel::TestCase) {
    let z = tc.draw(gs::temporal::zoned_datetimes().source(jiff_gs::zone_source()));
    assert!(jiff::tz::TimeZone::get(&z.zone.name).is_ok());
}
```

`ZoneSource::builder()` wraps any other database from TZif files, zone data given as fields (`ZoneData`), or an offset oracle (`ZoneOracle`) probed into transitions, each added up front or by a loader closure the first time a name is needed. The loader is dropped once nothing uses the source any more. A source's `resolve`, `offset_at` and `info` methods query it directly, and `zone(name)` gives the `TimeZone` a name stands for in it. Build a source once, in a `static`, rather than in a test body.

The `jiff` integration is complete: `dates` gains weekday and month masks, `times`, `datetimes`, `timestamps` and `signed_durations` a precision, `offsets` a granularity, `spans` per-unit builders and a `non_negative` switch, the new `zones()` draws the names of jiff's database with `names`, `allow_links`, `allow_legacy` and `allow_unknown` builders (`Etc/Unknown` maps to `TimeZone::unknown()`), `zoneds()` draws a wall-clock time in a drawn zone with both folds reached and gaps avoided by default, and `iso_week_dates`, `weekdays` and `eras` are added, with `DefaultGenerator` impls for `Weekday`, `Era`, `AmbiguousZoned` and `AmbiguousTimestamp` beside the existing ones.

The `chrono` integration's naive generators gain a precision and the weekday and month masks, `fixed_offsets` a granularity, `time_deltas` a precision and the engine's targeting of round durations, and `datetimes()` instant bounds, a precision, the `allow_imaginary` switch and the `ambiguous` policy. The new `chrono-tz` cargo feature adds `zones()`, drawing `chrono_tz::Tz` with `names`, `allow_links` and `allow_legacy` builders, and `DateTime<chrono_tz::Tz>` through `datetimes().zones(chrono_gs::zones())` or any other generator of `chrono_tz::Tz`.

The new `time` cargo feature adds `hegel::extras::time` for the `time` crate: `dates`, `times`, `primitive_datetimes`, `utc_offsets`, `offset_datetimes` (a wall-clock time in a drawn IANA zone, carrying the offset in force, with the `zones` and `source` builders of `zoned_datetimes()`), `durations`, `weekdays` and `months`, with `DefaultGenerator` impls for every value type including `UtcDateTime`; enabling `time/large-dates` widens the date range to years ±999,999.
