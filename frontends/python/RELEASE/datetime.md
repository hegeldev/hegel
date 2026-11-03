RELEASE_TYPE: minor

This release moves the date and time generators into a `gs.temporal` namespace, under the names every Hegel frontend uses for them, moves date and time generation into the engine, which now bundles a pinned IANA time zone database, and adds `zoned_datetimes()`, `instants()`, `spans()`, `zone_names()`, `offsets()` and `tzdb_version()`.

The released top-level names are deprecated. They keep working, with a `DeprecationWarning`, and accept their old arguments:

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

The module `hegel.generators.time` is now `hegel.generators.temporal`, and `from_type` resolves `datetime.timedelta` to `durations()`, `datetime.tzinfo` and `zoneinfo.ZoneInfo` to `zones()`, `datetime.timezone` to `offsets()` and `gs.temporal.Span` to `spans()`.

`zones()` and the new `zone_names()` draw IANA zones and their names from the names `zoneinfo` can load. Half the draws are weighted toward zones with a rare feature (a fractional offset, negative daylight saving, a skipped day), and a name shrinks down a ladder from `UTC` through the zones a reader recognises rather than alphabetically. Link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` excluded by default; `links` and `legacy` switch each, and `names` restricts the draw to an explicit list.

`datetimes()` with a `zones` generator now draws the zone and the wall-clock time in one engine draw when the zone is a `zoneinfo.ZoneInfo`, against the transitions `zoneinfo` itself reads for it: one draw in twenty on average lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch or the end of the signed 32-bit epoch. A repeated wall time is drawn with either value of `fold`, where `fold` used to be always 0, or with the one `ambiguous="earlier"` or `"later"` asks for, and a skipped wall time is reached deliberately; `allow_imaginary=False` now makes the engine draw a real wall time instead of rejecting the test case. `datetimes()` also accepts timezone-aware bounds, as Hypothesis does: both bounds are then moments in time, and every datetime is aware and lies between them in a zone drawn from `zones`, which defaults to `zones()` and must not generate `None`. A bound may be omitted (`min_value` and `max_value` now default to `None`); one naive and one aware bound is an `InvalidArgument`. The new `zoned_datetimes()` takes the same arguments but always draws aware datetimes, in a zone from `zones()` unless `zones` says otherwise.

The new `instants()` draws moments in time as UTC datetimes between two aware bounds, targeting instants such as the Unix epoch, the 32-bit boundaries and the transitions of every zone in the database. The new `spans()` draws calendar spans such as "1 month and 2 days", which a `timedelta` cannot represent, as `gs.temporal.Span` objects with a field per unit from years to nanoseconds; each unit takes a `(min, max)` bound, naming any unit draws only the named ones, and `sign` chooses one sign for the whole span (`"uniform"`, the default), a sign per unit (`"mixed"`) or magnitudes only (`"non_negative"`):

```python
span = tc.draw(gs.temporal.spans(months=(0, 11), days=(0, 30)))
str(span)  # "3mo 12d", as every Hegel frontend prints it
```

`dates()` takes `weekdays` (numbered as `date.weekday()` numbers them) and `months` to draw only, say, weekends or Februaries, moving a drawn date to the nearest allowed one rather than filtering. `times()`, `datetimes()`, `zoned_datetimes()`, `instants()` and `durations()` take a `precision`, one of the `timespec` names `isoformat` uses (`"milliseconds"`, `"seconds"`, `"minutes"` or `"hours"`), so every drawn time or delta is a whole number of that unit. `tzdb_version()` reports the IANA release of the engine's bundled database. `offsets()` draws fixed offsets as `datetime.timezone` objects, within bounds and at a `granularity` of hours, quarter-hours, minutes or seconds, targeting the offsets real zones have used.

`times()`, `datetimes()` and `durations()` draw whole microseconds, Python's resolution, rather than nanoseconds truncated afterwards. `dates()` targets the leap days nearest the bounds, the year boundaries, the 2038 and 2106 overflow days and ISO week 53 in one draw of twenty on average, and `durations()` round durations such as a second, an hour, a day or a week; a `timedelta` now shrinks toward zero as a whole, so the simplest negative delta is a microsecond below zero rather than a whole day.

Zoned draws take their time zone data from a `gs.temporal.ZoneSource`, so the gaps and folds they aim at are those of the database the code under test reads. The default, `ZoneSource.zoneinfo()`, is `zoneinfo`'s own database: the names `zoneinfo.available_timezones()` lists, and each zone read from the TZif file `zoneinfo.ZoneInfo` would load for it (from `zoneinfo.TZPATH`, then the `tzdata` package) the first time a draw needs it. `ZoneSource.bundled()` is the engine's database. `zones()` and `zone_names()` take a `source`, and `set_default_zone_source()` replaces the default for generators built afterwards:

```python
gs.temporal.zones(source=gs.temporal.ZoneSource.bundled())
```

An integration with another date and time library can fill an empty `ZoneSource()` with that library's data, as TZif bytes (`add_tzif`), fields (`add_zone` with a `ZoneData`), an offset oracle the engine probes (`add_oracle` with a `ZoneOracle`) or links (`add_link`), up front or from a loader the first time a draw needs a zone (`declare` and `set_loader`). An exception the loader raises fails the draw with an `InvalidArgument` caused by it.
