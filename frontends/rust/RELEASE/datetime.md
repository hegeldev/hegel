RELEASE_TYPE: minor

This release moves date and time generation into the engine, which now bundles a pinned IANA time zone database (`gs::temporal::tzdb_version()` names the release), gathers it under the new module `hegel::generators::temporal`, and rebuilds the `jiff` and `chrono` integrations on it. Time zones, zoned datetimes, instants, offsets, durations and calendar spans are now engine draws with the engine's shrinking, a drawn zone is stored by name so a saved failure means the same zone on every machine, and one draw in twenty on average targets a transition of the zone or an interesting calendar value, so the skipped and repeated wall times where date bugs live are generated deliberately rather than by luck. Five parts of this are breaking:

- `gs::durations()` and `gs::DurationGenerator` (over `std::time::Duration`) are deprecated in favour of `gs::temporal::std_durations()` and `gs::temporal::StdDurationGenerator`, which they now alias. The new generator draws through the engine's duration draw, and its default range ends at `i64::MAX` seconds rather than `u64::MAX` nanoseconds, so a test that added an unconstrained draw to an `Instant` or read it as a `u64` of nanoseconds now overflows where it used to fit; bound the draw with `max_value`. It also gains a `precision` builder.

  ```rust
  // before
  tc.draw(gs::durations().max_value(Duration::from_secs(60)))

  // after
  tc.draw(gs::temporal::std_durations().max_value(Duration::from_secs(60)))
  ```

- `hegel::extras::jiff::zoneds()` no longer has the `timezones(impl Generator<TimeZone>)` and `timestamps(impl Generator<Timestamp>)` builders, since the zone and the wall time now come from one engine draw. Use `timezones(jiff_gs::timezones()...)` to choose the zones and `min_value`/`max_value` (wall-clock) or `min_instant`/`max_instant` (instant) for the bounds.

  ```rust
  // before
  jiff_gs::zoneds().timezones(gs::just(TimeZone::UTC)).timestamps(jiff_gs::timestamps().min_value(min))

  // after
  jiff_gs::zoneds().timezones(jiff_gs::timezones().names(["UTC"])).min_instant(min)
  ```

- `hegel::extras::chrono::datetimes().timezones(...)` takes a source from the sealed `TimeZones` trait rather than any generator of any `chrono::TimeZone`: every generator of `Utc` or `FixedOffset` (such as `gs::just(Utc)` and `fixed_offsets()`) still works, and `timezones()` from the new `chrono-tz` feature adds IANA zones, but a generator of some other zone type such as `chrono::Local` no longer compiles, as the engine cannot classify wall times in it. A bound in chrono's leap-second representation (a `nanosecond()` of a billion or more) is now a usage error for `naive_times()`, `naive_datetimes()` and `datetimes()`, and no drawn time uses it.

- `gs::default::<jiff::tz::TimeZone>()` draws an IANA zone or a fixed offset and no longer yields `TimeZone::unknown()`, which is behind `timezones().unknown(true)`.

- `gs::from_lark(...).explicit(name, generator)` no longer requires the generator to be `UnwindSafe + RefUnwindSafe`, so a `.boxed()` generator works there; in exchange `LarkGenerator` itself is no longer `UnwindSafe` or `RefUnwindSafe`. Wrap it in `std::panic::AssertUnwindSafe` where a closure passed to `catch_unwind` captures it.

`hegel::generators::temporal` holds plain value types mirroring the engine's model, with public fields, range-checking `new` constructors and no arithmetic or parsing, so a test with any date library wraps the value it draws with `map`: `Date`, `Time`, `DateTime`, `Instant`, `UtcOffset`, `TimeZone`, `ZonedDateTime`, `SignedDuration` and `Span`, with the option enums `Precision`, `Granularity`, `Weekday`, `Month`, `Ambiguous`, `SpanSign`, `SpanUnit` and `Tag`. Each prints in RFC 9557 form, as the engine prints a counterexample in every language: `2024-03-10T02:30:00-04:00[America/New_York][gap]`. The functions `resolve`, `offset_at` and `info` query the default zone source outside any test, and `tzdb_version` names the bundled release.

The generators over them, named as in every other Hegel frontend, are `dates()` with `weekdays` and `months` masks, `times()` and `datetimes()` with a `precision`, `instants()`, `offsets()` with a `granularity` and an `unknown` switch for RFC 3339's `-00:00`, `zones()` over the names of a zone source with `source`, `names`, `links`, `legacy`, `unknown` and `opaque` builders, `zoned_datetimes()` with a `zones` builder taking `zones()` (zone and wall time drawn together) or any other generator of `TimeZone`, boxed ones included, wall-clock or instant bounds, and the `allow_imaginary` switch and `ambiguous` policy for skipped and repeated wall times, `durations()` over `SignedDuration` and `spans()` with per-unit bounds and a sign policy, plus `std_durations()` and `system_times()` over `std::time::Duration` and `std::time::SystemTime`. Every one of these types, and `SystemTime`, has a `DefaultGenerator` impl.

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

Zones are drawn, and wall times classified as unique, skipped or repeated, against a `ZoneSource`: the database the code under test uses, so the gaps and folds the engine targets are that database's own. Rust has no standard database, so the default is the engine's bundled one (`ZoneSource::bundled()`), and `set_default_zone_source` replaces it for the process. `zones().source(...)` draws against another for one generator, and `zoned_datetimes()` uses its zone generator's source. The jiff integration's generators draw against `hegel::extras::jiff::zone_source()`, whichever database jiff itself reads (the system's zoneinfo or its bundled copy), and the chrono-tz ones against `hegel::extras::chrono::zone_source()`, chrono-tz's compiled-in release, so neither rejects draws where its database and the engine's disagree any more. `ZoneSource::builder()` wraps any other database from TZif files, zone data given as fields (`ZoneData`), or an offset oracle (`ZoneOracle`) probed into transitions, each added up front or by a loader closure the first time a name is needed, and a source's `resolve`, `offset_at` and `info` methods query it directly.

```rust
use hegel::extras::jiff as jiff_gs;
use hegel::generators as gs;

#[hegel::test]
fn my_test(tc: hegel::TestCase) {
    let z = tc.draw(gs::temporal::zoned_datetimes()
        .zones(gs::temporal::zones().source(jiff_gs::zone_source())));
    assert!(jiff::tz::TimeZone::get(&z.zone.name).is_ok());
}
```

The `jiff` integration is complete: `dates` gains weekday and month masks, `times`, `datetimes`, `timestamps` and `signed_durations` a precision, `offsets` a granularity, `spans` draws every unit within jiff's limit with per-unit builders such as `.days(0, 30)` and a `non_negative` switch, `timezones()` draws the names of jiff's database (`Etc/Unknown` maps to `TimeZone::unknown()`), `zoneds()` draws a wall-clock time in a drawn zone with both folds reached and gaps avoided by default, and `iso_week_dates`, `weekdays` and `eras` are added, with `DefaultGenerator` impls for `Weekday`, `Era`, `AmbiguousZoned` and `AmbiguousTimestamp` beside the existing ones.

The `chrono` integration's naive generators gain a precision and the weekday and month masks, `fixed_offsets` a granularity, `time_deltas` a precision and the engine's targeting of round durations, and `datetimes()` instant bounds, a precision, the `allow_imaginary` switch and the `ambiguous` policy. The new `chrono-tz` cargo feature adds `timezones()`, drawing `chrono_tz::Tz`, and `DateTime<chrono_tz::Tz>` through `datetimes().timezones(chrono_gs::timezones())`.

The new `time` cargo feature adds `hegel::extras::time` for the `time` crate: `dates`, `times`, `primitive_datetimes`, `utc_offsets`, `offset_datetimes` (a wall-clock time in a drawn IANA zone, carrying the offset in force, with a `zones` builder taking `gs::temporal::zones()`), `durations`, `weekdays` and `months`, with `DefaultGenerator` impls for every value type including `UtcDateTime`; enabling `time/large-dates` widens the date range to years ±999,999.
