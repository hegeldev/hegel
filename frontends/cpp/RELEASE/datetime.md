RELEASE_TYPE: minor

This release replaces the string-valued date generators with typed ones backed by the engine, in a new `hegel::generators::temporal` namespace (header `hegel/generators/temporal.h`, included by `hegel/hegel.h`). `gs::dates()`, `gs::times()` and `gs::datetimes()` in `hegel::generators` are deprecated, and still draw the same ISO 8601 strings as before. Use `gs::temporal::dates()`, `gs::temporal::times()` and `gs::temporal::datetimes()`, which draw `gs::temporal::Date`, `Time` and `DateTime` values instead of `std::string`. These are plain aggregates that compare and print (as `2024-03-10`, `02:30:00.5`, `2024-03-10T02:30:00`), and the failure report shows them the same way:

```cpp
// before
std::string date = tc.draw(gs::dates());   // "2024-03-10"

// after
gs::temporal::Date date = tc.draw(gs::temporal::dates());   // {2024, 3, 10}
std::ostringstream text;
text << date;                                               // "2024-03-10"
```

The generators take bounds and options now. `dates()` accepts `min_value`, `max_value` and the `weekdays` and `months` a date may fall on; `times()` and `datetimes()` accept bounds and a `precision` (`Precision::Nanoseconds` by default, `Seconds` for whole seconds, and so on). A bound that is not a valid value, or that lies below the other bound, throws `std::invalid_argument` when the generator is built.

```cpp
namespace temporal = hegel::generators::temporal;

auto weekday = tc.draw(temporal::dates({
    .min_value = temporal::Date{2024, 1, 1},
    .max_value = temporal::Date{2024, 12, 31},
    .weekdays = {{temporal::Weekday::Monday, temporal::Weekday::Tuesday,
                  temporal::Weekday::Wednesday, temporal::Weekday::Thursday,
                  temporal::Weekday::Friday}},
}));
auto when = tc.draw(temporal::datetimes({.precision = temporal::Precision::Seconds}));
```

New generators in the same namespace cover the rest of the engine's date and time model:

- `instants()` draws an `Instant` (seconds and nanoseconds since the Unix epoch) within `min_value` / `max_value` at a `precision`.
- `offsets()` draws a `UtcOffset` within bounds at a `granularity` (whole seconds, minutes, quarter hours or hours) and, with `.unknown = true`, sometimes the unknown offset `-00:00`.
- `zones()` draws a `TimeZone` from the bundled IANA database: every current zone by default, or the given `names`; `links`, `legacy` and `unknown` admit link names, legacy names and the `Etc/Unknown` zone, and `opaque` accepts a listed name the bundled database lacks as a zone only the caller's database knows.
- `zoned_datetimes()` draws a `ZonedDateTime`: a wall-clock time in a zone, resolved to its offset and instant, with `kind` telling whether the wall time was unique, in a gap or in a fold. It takes its zones from any generator of `TimeZone` (`.zones = temporal::zones({.names = ...})`, or any other such as `gs::just(zone)`, where a name the bundled database lacks is drawn as an opaque zone), bounds either on the wall time (`min_value` / `max_value`) or on the instant (`min_instant` / `max_instant`), a `precision`, whether to `allow_imaginary` wall times (ones in a gap, which the engine resolves forward) and which side of a fold `ambiguous` may pick.
- `durations()` draws a `SignedDuration` (seconds and nanoseconds of one sign) within bounds at a `precision`, and `durations<D>()` draws any `std::chrono::duration` `D` directly, at the precision of its period and within its representable range.
- `spans()` draws a calendar `Span` of years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds and nanoseconds, each unit bounded by `.years = {{0, 10}}` and so on, all of one `sign` or `SpanSign::Uniform`.

```cpp
auto meeting = tc.draw(temporal::zoned_datetimes({
    .zones = temporal::zones({.names = std::vector<std::string>{"America/New_York"}}),
    .min_value = temporal::DateTime{{2024, 1, 1}, {0, 0, 0, 0}},
    .max_value = temporal::DateTime{{2024, 12, 31}, {23, 59, 59, 0}},
    .precision = temporal::Precision::Minutes,
}));
std::chrono::milliseconds timeout = tc.draw(temporal::durations<std::chrono::milliseconds>());
```

Every one of these types has a default generator, so `gs::default_generator<T>()` and reflected structs holding a `Date`, a `ZonedDateTime` or a `std::chrono::seconds` work without configuration. The values convert to and from the standard library: `Instant::to_chrono()` and `from_chrono()` for `std::chrono::system_clock` time points, `SignedDuration::to_chrono<D>()` and `from_chrono()` for `std::chrono::duration`, and, where the library has calendar support, `Date::to_chrono()` and `from_chrono()` for `std::chrono::year_month_day`. Each returns `std::nullopt` rather than a wrong value when the conversion does not fit.

`hegel::generators::temporal` also exposes the engine's time zone database: `tzdb_version()` names its IANA release, `info(name)` describes a zone (primary name, link, rank, tags), `resolve(name, civil)` resolves a wall time in a zone the way the generator does, and `offset_at(name, instant)` gives the offset in force at an instant.

Every zone draw and query resolves wall times against a **zone source**: the engine's bundled IANA database by default, or the database the code under test uses. Build a `gs::temporal::ZoneSource` from TZif files (`add_tzif`), from fields (`add_zone`, taking a `ZoneData` of transitions and a yearly `ZoneRule`), from an offset oracle (`add_oracle`, taking `std::function` callbacks, for a library that only answers "what is the offset at this instant"), from links (`add_link`), or lazily, by declaring names and setting a loader that adds a name's data the first time a draw or query needs it. Pass it to one generator with `zones({.source = source})`, which `zoned_datetimes()` over that generator also resolves against, or make it the process default with `gs::temporal::set_default_zone_source(source)`. `ZoneSource::bundled()` is the engine's database, and `source.info()`, `source.resolve()` and `source.offset_at()` query a source as the free functions query the default. An exception a loader or oracle throws becomes a `std::invalid_argument` carrying its message.

```cpp
namespace temporal = gs::temporal;

temporal::ZoneSource source;
source.declare({"America/New_York", "Europe/London"})
    .set_loader([](temporal::ZoneSource& s, const std::string& name) {
        s.add_tzif(name, read_file("/usr/share/zoneinfo/" + name));
    });
auto meeting = tc.draw(temporal::zoned_datetimes(
    {.zones = temporal::zones({.source = source})}));
```

The new header `hegel/extras/boost_date_time.h` turns Boost.DateTime time zones into zone sources: `hegel::extras::boost_date_time::zone_source(db)` holds every region of a `boost::local_time::tz_database` under its name, and `zone_data(zone)` converts one `time_zone_ptr`, such as a `posix_time_zone`, for `add_zone`. A Boost zone has one rule for all time and no history, which is what Boost computes too. The library itself keeps no Boost dependency; configure with `-DHEGEL_BOOST_DATE_TIME=ON` for the `hegel::boost_date_time` CMake target, which links an installed Boost or fetches the Boost release tarball.

The new header `hegel/extras/chrono_tzdb.h` does the same for the C++20 time zone database where the standard library implements it (libstdc++ from GCC 13 on): `hegel::extras::chrono_tzdb::zone_source()` draws from every name of `std::chrono::get_tzdb()` and probes each zone through `get_info()` the first time it is used, and `oracle(zone)` gives one zone's oracle for `add_oracle`.
