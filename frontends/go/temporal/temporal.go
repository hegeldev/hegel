// Package temporal has Hegel's generators of dates, times, durations and time zones.
//
// It is the namespace for everything in Hegel that is about time. Its
// generators are drawn from like any other, with [hegel.Draw], and produce
// values of the standard time package:
//
//   - [Dates] and [Datetimes] generate a [time.Time] in UTC.
//   - [Durations] generates a [time.Duration].
//   - [Offsets] generates a fixed-offset [time.Location], and [Zones] an IANA
//     time zone as one.
//   - [ZonedDatetimes] generates a [time.Time] in a generated time zone, and
//     looks for the times around daylight-saving and offset transitions.
//
// Each constructor returns a generator with builder methods that narrow it,
// such as Min, Max and Precision:
//
//	func TestRetryBacksOff(t *testing.T) {
//		hegel.Test(t, func(ht *hegel.T) {
//			start := hegel.Draw(ht, temporal.ZonedDatetimes().Precision(temporal.PrecisionSeconds))
//			timeout := hegel.Draw(ht, temporal.Durations().Min(time.Second).Max(time.Hour))
//			if deadline := start.Add(timeout); !deadline.After(start) {
//				ht.Fatalf("the deadline %v is not after %v", deadline, start)
//			}
//		})
//	}
//
// A [ZoneSource] is the time zone database the zone generators draw from:
// [HostZoneSource] (the default), [BundledZoneSource] or
// [ZoneSourceFromLocations]. [SetDefaultZoneSource] changes the default for
// the process.
//
// The generator types, [ZoneSource], [Precision], [Granularity] and
// [Ambiguous] are aliases of types that package hegel declares, because only
// package hegel can implement a generator. Their methods are documented
// there, on the type each alias names. Code that uses these generators needs
// no name from package hegel other than the ones every test uses, such as
// [hegel.Test] and [hegel.Draw].
package temporal

import (
	"time"

	"hegel.dev/go/hegel"
	"hegel.dev/go/hegel/internal/libhegel"
	"hegel.dev/go/hegel/internal/temporalbridge"
)

var constructors = temporalbridge.Registered.(temporalbridge.Constructors[
	DateGenerator, DatetimeGenerator, DurationGenerator, OffsetGenerator,
	ZoneGenerator, ZonedDatetimeGenerator, ZoneSource,
])

// Precision is the unit a generated time is a whole number of.
//
// A generator with a Precision draws only times of day, instants or durations
// that are a whole number of that unit, and shrinks in it.
type Precision = hegel.Precision

const (
	// PrecisionNanoseconds allows every value; it is the default.
	PrecisionNanoseconds Precision = libhegel.PRECISION_NANOSECONDS
	// PrecisionMicroseconds draws whole microseconds.
	PrecisionMicroseconds Precision = libhegel.PRECISION_MICROSECONDS
	// PrecisionMilliseconds draws whole milliseconds.
	PrecisionMilliseconds Precision = libhegel.PRECISION_MILLISECONDS
	// PrecisionSeconds draws whole seconds.
	PrecisionSeconds Precision = libhegel.PRECISION_SECONDS
	// PrecisionMinutes draws whole minutes.
	PrecisionMinutes Precision = libhegel.PRECISION_MINUTES
	// PrecisionHours draws whole hours.
	PrecisionHours Precision = libhegel.PRECISION_HOURS
)

// Granularity is the unit a generated UTC offset is a whole number of.
type Granularity = hegel.Granularity

const (
	// GranularityHours draws offsets of whole hours.
	GranularityHours Granularity = libhegel.OFFSET_GRANULARITY_HOURS
	// GranularityQuarterHours draws offsets of whole quarter-hours.
	GranularityQuarterHours Granularity = libhegel.OFFSET_GRANULARITY_QUARTER_HOURS
	// GranularityMinutes draws offsets of whole minutes.
	GranularityMinutes Granularity = libhegel.OFFSET_GRANULARITY_MINUTES
	// GranularitySeconds draws offsets of whole seconds; it is the default.
	GranularitySeconds Granularity = libhegel.OFFSET_GRANULARITY_SECONDS
)

// Ambiguous says which instant a repeated wall-clock time stands for.
//
// When the clocks go back, an hour of wall-clock times happens twice. The
// policy decides which of the two instants [ZonedDatetimes] generates there.
type Ambiguous = hegel.Ambiguous

const (
	// AmbiguousBoth generates either instant; it is the default.
	AmbiguousBoth Ambiguous = libhegel.AMBIGUOUS_BOTH
	// AmbiguousEarlier generates the first occurrence only.
	AmbiguousEarlier Ambiguous = libhegel.AMBIGUOUS_EARLIER
	// AmbiguousLater generates the second occurrence only.
	AmbiguousLater Ambiguous = libhegel.AMBIGUOUS_LATER
)

// DateGenerator generates dates at midnight UTC within inclusive bounds.
//
// Its methods Min, Max, Weekdays and Months are documented at
// [hegel.DateGenerator].
type DateGenerator = hegel.DateGenerator

// Dates returns a DateGenerator of dates from 0001-01-01 to 9999-12-31.
//
// Each value is a [time.Time] at midnight UTC. Bounds set with Min and Max
// may use years from -999999 through 999999.
func Dates() DateGenerator {
	return constructors.Dates()
}

// DatetimeGenerator generates UTC datetimes within inclusive bounds.
//
// Its methods Min, Max and Precision are documented at
// [hegel.DatetimeGenerator].
type DatetimeGenerator = hegel.DatetimeGenerator

// Datetimes returns a DatetimeGenerator of datetimes in the years 1 to 9999.
//
// Each value is a [time.Time] in UTC, from 0001-01-01 through the last
// nanosecond of 9999-12-31. Bounds set with Min and Max may use years from
// -999999 through 999999. For times in other zones, see [ZonedDatetimes].
func Datetimes() DatetimeGenerator {
	return constructors.Datetimes()
}

// DurationGenerator generates durations within inclusive bounds.
//
// Its methods Min, Max and Precision are documented at
// [hegel.DurationGenerator].
type DurationGenerator = hegel.DurationGenerator

// Durations returns a DurationGenerator over the whole range of [time.Duration].
//
// Durations shrink toward zero, and round ones (a second, an hour, a day, a
// week) and the bounds are drawn more often than chance.
func Durations() DurationGenerator {
	return constructors.Durations()
}

// OffsetGenerator generates fixed-offset time zones within inclusive bounds.
//
// Its methods Min, Max and Granularity are documented at
// [hegel.OffsetGenerator].
type OffsetGenerator = hegel.OffsetGenerator

// Offsets returns an OffsetGenerator of fixed-offset locations.
//
// Each location is a [time.FixedZone] named after its offset, such as
// "+05:30", "-08:00" or "+00:00". By default the offset is any whole number
// of seconds from 25:59:59 west of UTC to 25:59:59 east of it. Offsets shrink
// toward UTC, and the offsets real time zones have used are drawn more often
// than chance.
//
// The locations have no time zone database entry, so to draw times in them
// with [ZonedDatetimes], apply the offset to an instant yourself or put the
// locations in a [ZoneSourceFromLocations].
func Offsets() OffsetGenerator {
	return constructors.Offsets()
}

// ZoneGenerator generates time zones from a zone source.
//
// Its methods Source, Names, AllowLinks and AllowLegacy are documented at
// [hegel.ZoneGenerator].
type ZoneGenerator = hegel.ZoneGenerator

// Zones returns a ZoneGenerator of the time zones in the default zone source.
//
// It generates IANA time zones such as Europe/Paris as [time.Location]
// values. The default source is Go's own database (see [HostZoneSource] and
// [SetDefaultZoneSource]), read when Zones is called. Link names such as
// Asia/Calcutta are included and legacy names such as EST5EDT and US/Eastern
// are not. Zones with an unusual feature (a fractional offset, a skipped day,
// negative daylight saving) are drawn more often than chance, and a zone
// shrinks toward UTC.
func Zones() ZoneGenerator {
	return constructors.Zones()
}

// ZonedDatetimeGenerator generates times in generated time zones.
//
// Its methods Zones, Source, Min, Max, MinWall, MaxWall, Precision and
// Ambiguous are documented at [hegel.ZonedDatetimeGenerator].
type ZonedDatetimeGenerator = hegel.ZonedDatetimeGenerator

// ZonedDatetimes returns a ZonedDatetimeGenerator of times in time zones.
//
// Each value is a [time.Time] whose location is a generated zone. Unlike
// [Datetimes], which is always in UTC, it looks for the times that break
// time zone code: about one draw in twenty lands within a second, an hour or
// a day of one of the zone's daylight-saving or offset transitions, or of a
// well-known instant such as the Unix epoch, so both occurrences of a
// repeated wall-clock time are generated. A wall-clock time the clocks
// skipped is never generated, since a time.Time cannot hold one.
//
// By default the zones are those of [Zones] and the wall-clock time is
// anywhere in the years 1 to 9999. The default zone source is read when
// ZonedDatetimes is called.
func ZonedDatetimes() ZonedDatetimeGenerator {
	return constructors.ZonedDatetimes()
}

// ZoneSource is a time zone database that zone generators draw from.
//
// It decides which names [Zones] draws and which daylight-saving and offset
// transitions [ZonedDatetimes] aims at. [HostZoneSource], the default, is the
// database Go's own time package reads; [BundledZoneSource] is the one built
// into Hegel; [ZoneSourceFromLocations] holds locations of your choosing. A
// ZoneSource is safe for concurrent use and is meant to be built once, in a
// package-level variable, not inside a test body. Values compare equal when
// they are the same source. The zero value is not a source: generators given
// it fail when drawn from.
type ZoneSource = hegel.ZoneSource

// HostZoneSource returns Go's own time zone database as a zone source.
//
// It is the database [time.LoadLocation] reads: the ZONEINFO environment
// variable, the system's zoneinfo directory, the Go installation, or an
// embedded [time/tzdata]. Its names are those of the IANA database that
// time.LoadLocation can load, and each zone's transitions are the ones the
// loaded [time.Location] reports, so a generated wall-clock time is skipped or
// repeated exactly when Go says it is. It is the default zone source.
//
// HostZoneSource returns the same source on every call. Where Go has no time
// zone database at all (Windows without a Go installation, ZONEINFO or
// time/tzdata), drawing from a generator that uses it fails with an error
// that says to import time/tzdata.
func HostZoneSource() ZoneSource {
	return constructors.HostZoneSource()
}

// BundledZoneSource returns the time zone database built into Hegel.
//
// It is one pinned release of the IANA database, the same on every machine,
// so a test that uses it draws the same zones and transitions everywhere. A
// drawn zone is still returned as a [time.Location] loaded with
// [time.LoadLocation]: a name Go cannot load, and a time Go reads at another
// wall-clock time than the bundled database does, reject the test case.
//
// BundledZoneSource returns the same source on every call.
func BundledZoneSource() ZoneSource {
	return constructors.BundledZoneSource()
}

// ZoneSourceFromLocations returns a zone source holding exactly the given locations.
//
// Each location is known under the name its String method returns, and its
// transitions are read from the location itself, so the source also works for
// locations no database has: fixed zones from [time.FixedZone], and locations
// built with [time.LoadLocationFromTZData]. A name the IANA database also has
// keeps that database's classification, so a location named like a legacy
// zone (US/Eastern, say) is drawn only with AllowLegacy.
//
// ZoneSourceFromLocations returns a new source on every call. It panics for a
// nil location and for two locations with one name.
func ZoneSourceFromLocations(locations ...*time.Location) ZoneSource {
	return constructors.ZoneSourceFromLocations(locations...)
}

// DefaultZoneSource returns the zone source generators use when given none.
//
// It is [HostZoneSource] until [SetDefaultZoneSource] changes it.
func DefaultZoneSource() ZoneSource {
	return constructors.DefaultZoneSource()
}

// SetDefaultZoneSource changes the zone source generators use when given none.
//
// The default is process-wide, and a generator reads it when it is built: when
// [Zones] or [ZonedDatetimes] is called. A generator built earlier keeps the
// source it read. Set it once, before the tests run, for example in TestMain.
//
// SetDefaultZoneSource returns the previous default, so that a caller can
// restore it. It panics for the zero ZoneSource.
func SetDefaultZoneSource(source ZoneSource) ZoneSource {
	return constructors.SetDefaultZoneSource(source)
}
