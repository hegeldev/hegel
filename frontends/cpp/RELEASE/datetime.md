RELEASE_TYPE: minor

This release replaces the string-valued date generators with typed ones backed by the engine, in a new `hegel::generators::temporal` namespace (header `hegel/generators/temporal.h`, included by `hegel/hegel.h`). `gs::dates()`, `gs::times()` and `gs::datetimes()` in `hegel::generators` are deprecated, and still draw the same ISO 8601 strings as before. Use `gs::temporal::dates()`, `gs::temporal::times()` and `gs::temporal::datetimes()`, which draw `gs::temporal::Date`, `Time` and `DateTime` values instead of `std::string`:

```cpp
// before
std::string date = tc.draw(gs::dates());   // "2024-03-10"

// after
gs::temporal::Date date = tc.draw(gs::temporal::dates());   // {2024, 3, 10}
std::string text = gs::temporal::to_string(date);           // "2024-03-10"
```

The value types are plain aggregates. Each one compares, has a total order and a `std::hash`, so it works as a key of `std::set`, `std::map` and the unordered containers and in `gs::sets()`. `to_string()` and `operator<<` give the text the engine defines for the value (`2024-03-10`, `02:30:00.5`, `2024-03-10T02:30:00`, `2024-03-10T02:30:00-04:00[America/New_York][gap]`), and a failure report shows a drawn value as its type and that text:

```
auto draw_1 = hegel::generators::temporal::Date{"2024-03-10"};
```

The generators take bounds and options now. `dates()` accepts `min_value`, `max_value` and the `weekdays` and `months` a date may fall on; `times()` and `datetimes()` accept bounds and a `precision` (`Precision::Nanoseconds` by default, `Seconds` for whole seconds, and so on). A bound that is not a valid value, or that lies below the other bound, throws `std::invalid_argument` when the generator is built, as do bounds that hold no value at the precision.

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
- `offsets()` draws a `UtcOffset` within bounds at a `granularity` (whole seconds, minutes, quarter hours or hours) and, with `.allow_unknown = true`, sometimes the unknown offset `-00:00`.
- `zones()` draws a `TimeZone`: by default any name of the bundled IANA database, link names such as `Asia/Calcutta` included and legacy names such as `US/Eastern` excluded, or the given `names`. `.allow_links = false` leaves the link names out, `.allow_legacy = true` adds the legacy names, `.allow_unknown = true` adds the `Etc/Unknown` zone, and `.allow_opaque = true` accepts a listed name the database lacks as a zone only the caller's database knows.
- `zoned_datetimes()` draws a `ZonedDateTime`: a wall-clock time in a zone, resolved to its offset and instant, with `kind` telling whether the wall time was unique, in a gap or in a fold. It takes bounds either on the wall time (`min_value` / `max_value`) or on the instant (`min_instant` / `max_instant`), never one of each; a `precision`; whether to `allow_imaginary` wall times (ones in a gap); and which side of a fold `ambiguous` may pick. Its zones come from `zones()` by default, or from any generator of `TimeZone` given as `.zones`.
- `durations()` draws a `SignedDuration` (seconds and nanoseconds of one sign) within bounds at a `precision`. `durations<D>()` draws a `std::chrono::duration` `D` with an integer representation and a period from nanoseconds to hours, at the precision of that period and within the range both `D` and the engine hold; its draws print as `std::chrono::milliseconds{5}`.
- `spans()` draws a calendar `Span` of years, months, weeks, days, hours, minutes, seconds, milliseconds, microseconds and nanoseconds, each unit bounded by `.years = {{0, 10}}` and so on. Its `sign` is `SpanSign::Uniform` (the default: every unit of one drawn sign), `SpanSign::Mixed` (each unit of its own sign) or `SpanSign::NonNegative`.

```cpp
auto meeting = tc.draw(temporal::zoned_datetimes({
    .zones = temporal::zones({.names = std::vector<std::string>{"America/New_York"}}),
    .min_value = temporal::DateTime{{2024, 1, 1}, {0, 0, 0, 0}},
    .max_value = temporal::DateTime{{2024, 12, 31}, {23, 59, 59, 0}},
    .precision = temporal::Precision::Minutes,
}));
std::chrono::milliseconds timeout = tc.draw(temporal::durations<std::chrono::milliseconds>());
```

Every one of these types has a default generator, so `gs::default_generator<T>()` and reflected structs holding a `Date`, a `ZonedDateTime` or a `std::chrono::seconds` work without configuration. The values convert to and from the standard library: `Instant::to_chrono<D>()` and `from_chrono()` for `std::chrono::system_clock` time points of duration `D`, `SignedDuration::to_chrono<D>()` and `from_chrono()` for `std::chrono::duration`, and `Date::to_chrono<std::chrono::year_month_day>()` and `from_chrono()` for a C++20 calendar date (or a type with the same interface, such as `date::year_month_day`). Each returns `std::nullopt` rather than a wrong value when the conversion does not fit: a 64-bit count of nanoseconds, for example, holds only the years 1678 to 2262.

`hegel::generators::temporal` also exposes the engine's time zone database: `tzdb_version()` names its IANA release, `info(name)` describes a zone (primary name, link, rank, tags), `resolve(name, civil)` resolves a wall time in a zone the way the generator does, and `offset_at(name, instant)` gives the offset in force at an instant.

Every zone draw and query resolves wall times against a **zone source**: the engine's bundled IANA database by default, or the database the code under test uses. Build a `gs::temporal::ZoneSource` from TZif files (`add_tzif`), from fields (`add_zone`, taking a `ZoneData` of transitions and a yearly `ZoneRule`), from an offset oracle (`add_oracle`, taking `std::function` callbacks, for a library that only answers "what is the offset at this instant"), from links (`add_link`), or lazily, by declaring names and setting a loader that adds a name's data the first time a draw or query needs it. `ZoneSource::bundled()` is the engine's database, and `source.info()`, `source.resolve()` and `source.offset_at()` query a source as the free functions query the default. An exception a loader or oracle throws becomes a `std::invalid_argument` carrying its message. Build a source once, outside the test body:

```cpp
namespace temporal = gs::temporal;

temporal::ZoneSource system_zones() {
    temporal::ZoneSource source;
    source.declare({"America/New_York", "Europe/London"})
        .set_loader([](temporal::ZoneSource& s, const std::string& name) {
            s.add_tzif(name, read_file("/usr/share/zoneinfo/" + name));
        });
    return source;
}

auto meetings = temporal::zoned_datetimes({.source = system_zones()});
```

A source reaches a generator in one of three ways:

- `zones({.source = source})` draws the source's names, and `zoned_datetimes({.source = source})` draws wall times in every zone of the source.
- `zoned_datetimes({.zones = generator, .source = source})` resolves the names any other generator of zones draws (a filtered or mapped `zones()`, `gs::just(zone)`, `gs::sampled_from(...)`) against `source`. A name the source does not know makes the draw throw `std::invalid_argument`; it never becomes an opaque zone. When `.zones` is a generator that `zones()` returned, it has a source of its own, and giving `.source` as well throws `std::invalid_argument`.
- `gs::temporal::set_default_zone_source(source)` makes it the process default, which every generator and query without a source of its own uses. A generator reads the default when it is built and keeps it, and a query reads it when it is called. The function returns the previous default, so a test can put it back, and `default_zone_source()` returns the current one.

The new header `hegel/extras/boost_date_time.h` turns Boost.DateTime time zones into zone sources: `hegel::extras::boost_date_time::zone_source(db)` holds every region of a `boost::local_time::tz_database` under its name, and `zone_data(zone)` converts one `time_zone_ptr`, such as a `posix_time_zone`, for `add_zone`. A Boost zone has one rule for all time and no history, which is what Boost computes too. The library itself keeps no Boost dependency. A project that builds Hegel inside its own build (`FetchContent` or `add_subdirectory`) can set `HEGEL_BOOST_DATE_TIME=ON` for the `hegel::boost_date_time` CMake target, which links an installed Boost or fetches the Boost release tarball; an installed Hegel has the header and no such target, so link Hegel and Boost yourself.

The new header `hegel/extras/chrono_tzdb.h` does the same for the C++20 time zone database where the standard library implements it (libstdc++ from GCC 13 on): `hegel::extras::chrono_tzdb::zone_source()` draws from every name of `std::chrono::get_tzdb()` and probes each zone through `get_info()` the first time it is used, and `oracle(zone)` gives one zone's oracle for `add_oracle`.
