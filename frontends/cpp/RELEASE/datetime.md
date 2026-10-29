RELEASE_TYPE: minor

This release replaces the string-valued date generators with typed ones backed by the engine. `gs::dates()`, `gs::times()` and `gs::datetimes()` no longer return `std::string`; they draw the new `hegel::datetime::Date`, `Time` and `DateTime` values, plain aggregates that compare and print (as `2024-03-10`, `02:30:00.5`, `2024-03-10T02:30:00`) and that the failure report shows the same way:

```cpp
// before
std::string date = tc.draw(gs::dates());   // "2024-03-10"

// after
hegel::datetime::Date date = tc.draw(gs::dates());   // {2024, 3, 10}
std::ostringstream text;
text << date;                                        // "2024-03-10"
```

The generators take bounds and options now. `gs::dates()` accepts `min_value`, `max_value` and the `weekdays` and `months` a date may fall on; `gs::times()` and `gs::datetimes()` accept bounds and a `precision` (`hegel::datetime::Precision::Nanoseconds` by default, `Seconds` for whole seconds, and so on). A bound that is not a valid value, or that lies below the other bound, throws `std::invalid_argument` when the generator is built.

```cpp
namespace dt = hegel::datetime;

auto weekday = tc.draw(gs::dates({
    .min_value = dt::Date{2024, 1, 1},
    .max_value = dt::Date{2024, 12, 31},
    .weekdays = {{dt::Weekday::Monday, dt::Weekday::Tuesday, dt::Weekday::Wednesday,
                  dt::Weekday::Thursday, dt::Weekday::Friday}},
}));
auto when = tc.draw(gs::datetimes({.precision = dt::Precision::Seconds}));
```

New generators cover the rest of the engine's date and time model:

- `gs::timestamps()` draws a `Timestamp` (seconds and nanoseconds since the Unix epoch) within `min_value` / `max_value` at a `precision`.
- `gs::utc_offsets()` draws a `UtcOffset` within bounds at a `granularity` (whole seconds, minutes, quarter hours or hours) and, with `.unknown = true`, sometimes the unknown offset `-00:00`.
- `gs::timezones()` draws a `TimeZone` from the bundled IANA database: every current zone by default, or the given `names`; `links`, `legacy` and `unknown` admit link names, legacy names and the `Etc/Unknown` zone, and `opaque` accepts a listed name the bundled database lacks as a zone only the caller's database knows.
- `gs::zoned_datetimes()` draws a `ZonedDateTime`: a wall-clock time in a zone, resolved to its offset and instant, with `kind` telling whether the wall time was unique, in a gap or in a fold. It takes its zones as a `TimezonesParams` (`.timezones = {.names = ...}`), bounds either on the wall time (`min_value` / `max_value`) or on the instant (`min_instant` / `max_instant`), a `precision`, whether to `allow_imaginary` wall times (ones in a gap, which the engine resolves forward) and which side of a fold `ambiguous` may pick.
- `gs::signed_durations()` draws a `SignedDuration` (seconds and nanoseconds of one sign), and `gs::durations<D>()` draws any `std::chrono::duration` `D` directly, at the precision of its period and within its representable range.
- `gs::spans()` draws a calendar `Span` of years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds and nanoseconds, each unit bounded by `.years = {{0, 10}}` and so on, all of one `sign` or `SpanSign::Uniform`.

```cpp
auto meeting = tc.draw(gs::zoned_datetimes({
    .timezones = {.names = std::vector<std::string>{"America/New_York"}},
    .min_value = dt::DateTime{{2024, 1, 1}, {0, 0, 0, 0}},
    .max_value = dt::DateTime{{2024, 12, 31}, {23, 59, 59, 0}},
    .precision = dt::Precision::Minutes,
}));
std::chrono::milliseconds timeout = tc.draw(gs::durations<std::chrono::milliseconds>());
```

Every one of these types has a default generator, so `gs::default_generator<T>()` and reflected structs holding a `Date`, a `ZonedDateTime` or a `std::chrono::seconds` work without configuration. The values convert to and from the standard library: `Timestamp::to_chrono()` and `from_chrono()` for `std::chrono::system_clock` time points, `SignedDuration::to_chrono<D>()` and `from_chrono()` for `std::chrono::duration`, and, where the library has calendar support, `Date::to_chrono()` and `from_chrono()` for `std::chrono::year_month_day`. Each returns `std::nullopt` rather than a wrong value when the conversion does not fit.

`hegel::datetime` also exposes the engine's time zone database: `tzdb_version()` names its IANA release, `info(name)` describes a zone (primary name, link, rank, tags), `resolve(name, civil)` resolves a wall time in a zone the way the generator does, and `offset_at(name, timestamp)` gives the offset in force at an instant.
