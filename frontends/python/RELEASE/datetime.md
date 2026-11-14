RELEASE_TYPE: minor

This release moves the date and time generators into a `gs.temporal` namespace, under the names every Hegel frontend uses for them, moves date and time generation into the engine, which now bundles a pinned IANA time zone database, and adds `zoned_datetimes()`, `instants()`, `spans()`, `zone_names()`, `offsets()` and `tzdb_version()`.

The released top-level names are deprecated. They keep working, with a `DeprecationWarning`, and take the arguments they were released with; the arguments this release adds are on the `gs.temporal` names only:

```python
# before
gs.dates()
gs.times(timezones=gs.timezones())
gs.datetimes(timezones=gs.timezones())
gs.timedeltas()
gs.timezones()

# after
gs.temporal.dates()
gs.temporal.times(zones=gs.temporal.zones())
gs.temporal.datetimes(zones=gs.temporal.zones())
gs.temporal.durations()
gs.temporal.zones()
```

The module `hegel.generators.time` is now `hegel.generators.temporal`, with no alias under the old name, and `from_type` resolves `datetime.timedelta` to `durations()`, `datetime.tzinfo` and `zoneinfo.ZoneInfo` to `zones()`, `datetime.timezone` to `offsets()` and `gs.temporal.Span` to `spans()`.

Every bound of every generator may be omitted or passed as `None`, and the `repr` of a date or time generator shows only the arguments it was given.

`zones()` and the new `zone_names()` draw IANA zones and their names from the names `zoneinfo` can load. Half the draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks down a ladder from `UTC` through the zones a reader recognises rather than alphabetically. Link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default; `allow_links` and `allow_legacy` switch each. `names` restricts the draw to an explicit list instead, and cannot be combined with either.

`datetimes()` with a `zones` generator now draws the wall-clock time in the engine when the zone is a `zoneinfo.ZoneInfo`, against the transitions `zoneinfo` itself reads for it: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch or the end of the signed 32-bit epoch. A repeated wall time is drawn with either value of `fold`, where `fold` used to be always 0, or with the one `ambiguous="earlier"` or `"later"` asks for, and a skipped wall time is reached deliberately; `allow_imaginary=False` now makes the engine draw a real wall time instead of rejecting the test case. A `tzinfo` of another kind (pytz, dateutil, a `ZoneInfo` with no key) is drawn without that targeting, and `fold` and `ambiguous` go by what the `tzinfo` itself reports.

`datetimes()` also accepts timezone-aware bounds, as Hypothesis does: both bounds are then moments in time, and every datetime is aware and lies between them in a zone drawn from `zones`, which defaults to every zone of the zone source and must not generate `None`. One naive and one aware bound is an `InvalidArgument`. The new `zoned_datetimes()` takes the same arguments but always draws aware datetimes.

The new `instants()` draws moments in time as UTC datetimes between two aware bounds, targeting instants such as the Unix epoch, the 32-bit boundaries and the transitions of every zone in the database. The new `spans()` draws calendar spans such as "1 month and 2 days", which a `timedelta` cannot represent, as `gs.temporal.Span` objects with a field per unit from years to nanoseconds; each unit takes a `(min, max)` bound, naming any unit draws only the named ones, and `sign` chooses one sign for the whole span (`"uniform"`, the default), a sign per unit (`"mixed"`) or magnitudes only (`"non_negative"`):

```python
span = tc.draw(gs.temporal.spans(months=(0, 11), days=(0, 30)))
str(span)  # "3mo 12d", as every Hegel frontend prints it
```

A span with no positive unit prints one leading `-` (`"-3d 4h"` is minus three days and minus four hours), and one with units of both signs prints a sign on each (`"-3d +4h"`).

`dates()` takes `weekdays` (numbered as `date.weekday()` numbers them) and `months` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering. `times()`, `datetimes()`, `zoned_datetimes()`, `instants()` and `durations()` take a `precision`, one of the `timespec` names `isoformat` uses (`"milliseconds"`, `"seconds"`, `"minutes"` or `"hours"`), so every drawn time or delta is a whole number of that unit. `tzdb_version()` reports the IANA release of the engine's bundled database. `offsets()` draws fixed offsets as `datetime.timezone` objects, within bounds and at a `granularity` of hours, quarter-hours, minutes or seconds, targeting the offsets real zones have used.

`times()`, `datetimes()` and `durations()` draw whole microseconds, Python's resolution, rather than nanoseconds truncated afterwards. `times()` now raises `InvalidArgument` when its `zones` generator produces something that is not a `tzinfo`. `dates()` targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average, and `durations()` round durations such as a second, an hour, a day or a week; a `timedelta` now shrinks toward zero as a whole, so the simplest negative delta is a microsecond below zero rather than a whole day.

Zoned draws take their time zone data from a `gs.temporal.ZoneSource`, so the gaps and folds they aim at are those of the database the code under test reads. The default, `ZoneSource.zoneinfo()`, is `zoneinfo`'s own database: the names `zoneinfo.available_timezones()` lists, and each zone read from the TZif file `zoneinfo.ZoneInfo` would load for it (from `zoneinfo.TZPATH`, then the `tzdata` package) the first time a draw needs it. `ZoneSource.bundled()` is the engine's database. `zones()`, `zone_names()`, `datetimes()` and `zoned_datetimes()` take a `source`; on the last two it is the data every drawn `ZoneInfo` is read against, and the zones drawn by default. `default_zone_source()` returns the default and `set_default_zone_source()` replaces it, returning the source it replaced. A generator reads its source, and the names the source knows, when it is built:

```python
BUNDLED = gs.temporal.ZoneSource.bundled()
gs.temporal.zoned_datetimes(source=BUNDLED)
gs.temporal.datetimes(zones=gs.temporal.zones(source=BUNDLED))
```

Where `zoneinfo` finds no time zone database at all (Windows without the `tzdata` package), a generator that needs the default source's zones (`zones()`, `zone_names()`, `zoned_datetimes()`, `datetimes()` with aware bounds, `from_type(datetime.tzinfo)`) raises `InvalidArgument` when it is built, saying to install `tzdata`; `zone_names(source=ZoneSource.bundled())` draws names without it. The deprecated `gs.timezones()` still generates only `datetime.UTC` there, as it always has.

An integration with another date and time library can fill an empty `ZoneSource()` with that library's data, as TZif bytes (`add_tzif`), fields (`add_zone` with a `ZoneData`, built from `ZoneType`, `ZoneRule` and `RuleDate`), an offset oracle the engine probes (`add_oracle` with a `ZoneOracle`) or links (`add_link`), up front or from a loader the first time a draw needs a zone (`declare` and `set_loader`), and label it with a `version`. An exception the loader raises fails the draw with an `InvalidArgument` caused by it. `source.info(name)`, `name in source`, `source.resolve(name, wall_time)` and `source.offset_at(name, instant)` read back what the engine made of the data, to check it against the library's own answers.
