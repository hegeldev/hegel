RELEASE_TYPE: minor

This release adds generators for durations, UTC offsets, time zones and times in time zones (`Durations()`, `Offsets()`, `Zones()` and `ZonedDatetimes()`), zone sources that say which time zone database they draw from, and new options on `Dates()` and `Datetimes()`. `Dates()` and `Datetimes()` draw what they did unless given one of the new options. `Default` changes for two types, described at the end.

`ZonedDatetimes()` generates a `time.Time` in a generated zone, the zone and the time in one engine draw. Unlike `Datetimes()`, which is always in UTC, it looks for the times that break time zone code: about one draw in twenty lands within a second, an hour or a day of one of the zone's daylight-saving or offset transitions, or of a well-known instant such as the Unix epoch. A wall-clock time that happens twice is generated as either instant by default, or as the one `Ambiguous(hegel.AmbiguousEarlier)` or `AmbiguousLater` asks for, and a wall-clock time the clocks skipped, which a `time.Time` cannot hold, is never generated. Bounds are either on the instant (`Min`, `Max`) or on what the clock shows in the generated zone (`MinWall`, `MaxWall`); setting both kinds on one generator is an error.

```go
zoned := hegel.ZonedDatetimes().
	Zones(hegel.Zones().Names("America/New_York", "Europe/London")).
	MinWall(time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)).
	MaxWall(time.Date(2024, 12, 31, 23, 59, 59, 0, time.UTC)).
	Precision(hegel.PrecisionSeconds)
t := hegel.Draw(ht, zoned)
```

`Zones()` generates IANA time zones as `*time.Location` values. Link names such as `Asia/Calcutta` are included and legacy names such as `EST5EDT` are not, unless `AllowLinks(false)` or `AllowLegacy(true)` says otherwise, and `Names(...)` restricts the draw to a list. Zones with an unusual feature (a fractional offset, a skipped day, negative daylight saving) are drawn more often than chance, and a zone shrinks toward UTC through the zones a reader recognises. `Offsets()` generates fixed offsets as `time.FixedZone` locations named after the offset (`"+05:30"`), with `Min`, `Max` and a `Granularity` of hours, quarter-hours, minutes or seconds. `Durations()` generates a `time.Duration` with `Min`, `Max` and `Precision`, shrinking toward zero.

```go
loc := hegel.Draw(ht, hegel.Zones().AllowLinks(false))
offset := hegel.Draw(ht, hegel.Offsets().Min(-12*time.Hour).Max(14*time.Hour).Granularity(hegel.GranularityQuarterHours))
timeout := hegel.Draw(ht, hegel.Durations().Min(0).Max(time.Hour).Precision(hegel.PrecisionMilliseconds))
```

The engine aims at each zone's transitions using the data of a `ZoneSource`. The default, `HostZoneSource()`, is the database `time.LoadLocation` reads, so a generated time is skipped or repeated exactly when Go says it is. `BundledZoneSource()` is the pinned IANA release built into Hegel, which aims draws at the same transitions on every machine; a drawn value still means what Go's database says, and a time the two disagree about rejects the test case rather than failing it. `ZoneSourceFromLocations(...)` holds exactly the locations it is given, including ones no database has, such as a `time.FixedZone` or a location built with `time.LoadLocationFromTZData`. A source is built once, in a package-level variable, and shared:

```go
var officeZones = hegel.ZoneSourceFromLocations(newYork, time.FixedZone("Office", 5*60*60))

loc := hegel.Draw(ht, hegel.Zones().Source(officeZones))
t := hegel.Draw(ht, hegel.ZonedDatetimes().Source(officeZones))
```

`Zones().Source(s)` and `ZonedDatetimes().Source(s)` set the source of one generator, and `SetDefaultZoneSource(s)` sets the default for the process, returning the default it replaced so that a caller can put it back; `DefaultZoneSource()` reads it. A generator reads the default when it is built and keeps that source afterwards. `ZonedDatetimes().Zones(g)` uses the source of the `Zones()` generator it is given, and giving it both such a generator and `Source(...)` is an error. The zones of any other generator, such as `hegel.Just(loc)` or a filtered `Zones()`, are looked up by name in the source given to `Source(...)`, or else in the default, and a zone that source does not know is an error naming it, never a silent fallback.

Where Go has no time zone database at all (Windows without a Go installation, for example), a generator that uses the host source fails with an error saying to add `import _ "time/tzdata"`, which embeds one in the test binary.

`Dates()` gains `Weekdays(...)` and `Months(...)`, which move a drawn date to the nearest allowed one rather than filtering, and `Datetimes()` gains `Precision(...)`, as `Durations()` and `ZonedDatetimes()` have: every generated value is a whole number of that unit, from `PrecisionNanoseconds` (the default) to `PrecisionHours`.

```go
weekend := hegel.Draw(ht, hegel.Dates().Weekdays(time.Saturday, time.Sunday))
minute := hegel.Draw(ht, hegel.Datetimes().Precision(hegel.PrecisionMinutes))
```

Go has no type for a time of day without a date or for a calendar period of years, months and days, so this release has no time-of-day generator and no span generator, which other Hegel frontends have.

Two things change for existing tests. `Default[time.Duration]()`, and a `time.Duration` field of a struct `Default` fills, now uses `Durations()`: the range is still every `time.Duration`, but round durations and the two ends of the range are drawn more often and a failing test's replayed draws differ. `Default` now fills a `*time.Location` with `Zones()`, using the zone source that is the default at each draw; such a pointer is never nil. A drawn `*time.Location` prints in a failing test's draw report as `time.Location("Europe/Paris")` rather than as a dump of its transitions.
