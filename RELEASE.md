RELEASE_TYPE: minor

`filter` on a pool's consuming generator (`Pool::values_consumed` and `ConcurrentPool::values_consumed`) now consumes only the value it yields. A rejected attempt used to consume its value for good, so `values_consumed().filter(even)` on a pool of `0..9` silently lost odd values no rule had seen. The attempts now keep each rejected value out of circulation until they are over, so no attempt draws the same value twice, and then put every rejected value back. Two trait methods with default implementations carry this: `Generator::do_draw_filtered` and `PrintableGenerator::do_draw_filtered_and_print` are the attempts a `filter` makes of its source generator, which a generator whose draws have side effects can run itself; existing implementations of either trait are unaffected. A filter of a filter now makes one round of attempts with both predicates rather than nesting one round inside another.

This release moves date and time generation into the engine, which now bundles a pinned IANA time zone database (`hegel::datetime::tzdb_version()` names the release), and rebuilds the `jiff` and `chrono` integrations on it. Time zones, zoned datetimes, instants, offsets, durations and calendar spans are now engine draws with the engine's shrinking, a drawn zone is stored by name so a saved failure means the same zone on every machine, and one draw in twenty on average targets a transition of the zone or an interesting calendar value, so the skipped and repeated wall times where date bugs live are generated deliberately rather than by luck. Three parts of this are breaking:

- `hegel::extras::jiff::zoneds()` no longer has the `timezones(impl Generator<TimeZone>)` and `timestamps(impl Generator<Timestamp>)` builders, since the zone and the wall time now come from one engine draw. Use `time_zones(jiff_gs::time_zones()...)` to choose the zones and `min_value`/`max_value` (wall-clock) or `min_instant`/`max_instant` (instant) for the bounds.

  ```rust
  // before
  jiff_gs::zoneds().timezones(gs::just(TimeZone::UTC)).timestamps(jiff_gs::timestamps().min_value(min))

  // after
  jiff_gs::zoneds().time_zones(jiff_gs::time_zones().names(["UTC"])).min_instant(min)
  ```

- `hegel::extras::chrono::datetimes().timezones(...)` takes a source from the sealed `TimeZones` trait rather than any generator of any `chrono::TimeZone`: every generator of `Utc` or `FixedOffset` (such as `gs::just(Utc)` and `fixed_offsets()`) still works, and `time_zones()` from the new `chrono-tz` feature adds IANA zones, but a generator of some other zone type such as `chrono::Local` no longer compiles, as the engine cannot classify wall times in it. A bound in chrono's leap-second representation (a `nanosecond()` of a billion or more) is now a usage error for `naive_times()`, `naive_datetimes()` and `datetimes()`, and no drawn time uses it.

- `gs::default::<jiff::tz::TimeZone>()` draws an IANA zone or a fixed offset and no longer yields `TimeZone::unknown()`, which is behind `time_zones().unknown(true)`.

The new first-party module `hegel::datetime` holds plain value types mirroring the engine's model, with public fields, range-checking `new` constructors and no arithmetic or parsing, so a test with any date library wraps the value it draws with `map`: `Date`, `Time`, `DateTime`, `Timestamp`, `UtcOffset`, `TimeZone`, `ZonedDateTime`, `SignedDuration` and `Span`, with the option enums `Precision`, `Granularity`, `Weekday`, `Month`, `Imaginary`, `Ambiguous`, `SpanSign`, `SpanUnit` and `Tag`. Each prints in RFC 9557 form, as the engine prints a counterexample in every language: `2024-03-10T02:30:00-04:00[America/New_York][gap]`. The functions `tzdb_version`, `resolve`, `offset_at` and `info` query the bundled database outside any test.

The generators over them live in `hegel::generators`: `dates()` with `weekdays` and `months` masks, `times()` and `datetimes()` with a `precision`, `timestamps()`, `utc_offsets()` with a `granularity` and an `unknown` switch for RFC 3339's `-00:00`, `time_zones()` over the bundled IANA names with `names`, `links`, `legacy`, `unknown` and `opaque` builders, `zoned_datetimes()` with wall-clock or instant bounds and the `imaginary` and `ambiguous` policies for skipped and repeated wall times, `signed_durations()` and `spans()` with per-unit bounds and a sign policy, plus `system_times()` over `std::time::SystemTime`. Every one of these types, and `SystemTime`, has a `DefaultGenerator` impl. `durations()` now draws through the engine's duration draw with a `precision` builder, and its default range ends at `i64::MAX` seconds rather than `u64::MAX` nanoseconds.

```rust
use hegel::generators as gs;
use hegel::datetime::{Imaginary, Precision};

#[hegel::test]
fn my_test(tc: hegel::TestCase) {
    let z = tc.draw(gs::zoned_datetimes()
        .time_zones(gs::time_zones().names(["Europe/London", "America/New_York"]))
        .precision(Precision::Seconds)
        .imaginary(Imaginary::Allow));
    assert!(z.offset_before == z.offset_after || z.kind != hegel::datetime::Kind::Unique);
}
```

The `jiff` integration is complete: `dates` gains weekday and month masks, `times`, `datetimes`, `timestamps` and `signed_durations` a precision, `offsets` a granularity, `spans` draws every unit within jiff's limit with per-unit builders such as `.days(0, 30)` and a `non_negative` switch, `time_zones()` draws the IANA names both jiff and the engine know (`Etc/Unknown` maps to `TimeZone::unknown()`), `zoneds()` draws a wall-clock time in a drawn zone with both folds reached and gaps avoided by default, and `iso_week_dates`, `weekdays` and `eras` are added, with `DefaultGenerator` impls for `Weekday`, `Era`, `AmbiguousZoned` and `AmbiguousTimestamp` beside the existing ones.

The `chrono` integration's naive generators gain a precision and the weekday and month masks, `fixed_offsets` a granularity, and `datetimes()` instant bounds, a precision and the `imaginary` and `ambiguous` policies. The new `chrono-tz` cargo feature adds `time_zones()`, drawing `chrono_tz::Tz`, and `DateTime<chrono_tz::Tz>` through `datetimes().timezones(chrono_gs::time_zones())`, resolved with `LocalResult` by the engine's fold.

The new `time` cargo feature adds `hegel::extras::time` for the `time` crate: `dates`, `times`, `primitive_datetimes`, `utc_offsets`, `offset_datetimes` (a wall-clock time in a drawn IANA zone, carrying the offset in force), `durations`, `weekdays` and `months`, with `DefaultGenerator` impls for every value type including `UtcDateTime`; enabling `time/large-dates` widens the date range to years ±999,999.
