package hegel

import (
	"errors"
	"fmt"
	"math"
	"time"

	"hegel.dev/go/hegel/internal/libhegel"
	"hegel.dev/go/hegel/internal/temporalbridge"
)

// Precision is the unit a generated time is a whole number of.
//
// A generator with a Precision draws only times of day, instants or durations
// that are a whole number of that unit, and shrinks in it. Its values are the
// Precision constants of package [hegel.dev/go/hegel/temporal], from
// temporal.PrecisionNanoseconds to temporal.PrecisionHours.
type Precision = libhegel.Precision

// Granularity is the unit a generated UTC offset is a whole number of.
//
// Its values are the Granularity constants of package
// [hegel.dev/go/hegel/temporal], from temporal.GranularityHours to
// temporal.GranularitySeconds.
type Granularity = libhegel.OffsetGranularity

// Ambiguous says which instant a repeated wall-clock time stands for.
//
// When the clocks go back, an hour of wall-clock times happens twice. The
// policy decides which of the two instants a [ZonedDatetimeGenerator]
// generates there. Its values are the Ambiguous constants of package
// [hegel.dev/go/hegel/temporal]: temporal.AmbiguousBoth,
// temporal.AmbiguousEarlier and temporal.AmbiguousLater.
type Ambiguous = libhegel.Ambiguous

func init() {
	temporalbridge.Registered = temporalbridge.Constructors[
		DateGenerator, DatetimeGenerator, DurationGenerator, OffsetGenerator,
		ZoneGenerator, ZonedDatetimeGenerator, ZoneSource,
	]{
		Dates:                   newDates,
		Datetimes:               newDatetimes,
		Durations:               newDurations,
		Offsets:                 newOffsets,
		Zones:                   newZones,
		ZonedDatetimes:          newZonedDatetimes,
		HostZoneSource:          hostZoneSource,
		BundledZoneSource:       bundledZoneSource,
		ZoneSourceFromLocations: zoneSourceFromLocations,
		DefaultZoneSource:       defaultZoneSource,
		SetDefaultZoneSource:    setDefaultZoneSource,
	}
}

// applyDateMask sets a [DateGenerator]'s weekday or month restriction on its
// engine options. values is nil when the restriction was not given; bit maps
// a value in [lo, hi] to its bit in the engine's mask.
func applyDateMask[T ~int](values []T, lo, hi T, what string, bit func(T) int, set func(mask uint32) error) error {
	if values == nil {
		return nil
	}
	var mask uint32
	for _, v := range values {
		if v < lo || v > hi {
			return fmt.Errorf("temporal.Dates: %d is not a %s", int(v), what)
		}
		mask |= 1 << bit(v)
	}
	return set(mask)
}

// wallClockBounds returns the engine's bounds for a pair of optional
// wall-clock bounds, each defaulting to its end of the years 1 to 9999.
func wallClockBounds(minVal, maxVal *time.Time) (libhegel.Datetime, libhegel.Datetime, error) {
	bounds := [2]libhegel.Datetime{{Date: fullDateMin}, {Date: fullDateMax, Time: fullTimeMax}}
	for i, v := range [2]*time.Time{minVal, maxVal} {
		if v == nil {
			continue
		}
		var err error
		bounds[i], err = datetimeFromTime(*v)
		if err != nil {
			return libhegel.Datetime{}, libhegel.Datetime{}, err
		}
	}
	return bounds[0], bounds[1], nil
}

// DurationGenerator generates durations within inclusive bounds.
//
// It is made with Durations of package [hegel.dev/go/hegel/temporal].
type DurationGenerator struct {
	minVal    *time.Duration
	maxVal    *time.Duration
	precision Precision
}

var _ Generator[time.Duration] = DurationGenerator{}

func newDurations() DurationGenerator {
	return DurationGenerator{}
}

// Min sets the inclusive minimum.
func (g DurationGenerator) Min(v time.Duration) DurationGenerator {
	g.minVal = &v
	return g
}

// Max sets the inclusive maximum.
func (g DurationGenerator) Max(v time.Duration) DurationGenerator {
	g.maxVal = &v
	return g
}

// Precision makes every generated duration a whole number of p.
func (g DurationGenerator) Precision(p Precision) DurationGenerator {
	g.precision = p
	return g
}

func engineDuration(d time.Duration) libhegel.Duration {
	return libhegel.Duration{Seconds: int64(d / time.Second), Nanoseconds: int32(d % time.Second)}
}

func (g DurationGenerator) draw(tc TestCase) (time.Duration, error) {
	minVal, maxVal := time.Duration(math.MinInt64), time.Duration(math.MaxInt64)
	if g.minVal != nil {
		minVal = *g.minVal
	}
	if g.maxVal != nil {
		maxVal = *g.maxVal
	}
	ctx, ltc := tc.engine()
	d, err := ltc.GenerateDuration(ctx, engineDuration(minVal), engineDuration(maxVal), g.precision)
	if err != nil {
		return 0, err
	}
	return time.Duration(d.Seconds)*time.Second + time.Duration(d.Nanoseconds), nil
}

// The engine's limits on a UTC offset: 25:59:59 either side of UTC.
const (
	minOffset = -93599 * time.Second
	maxOffset = 93599 * time.Second
)

// OffsetGenerator generates fixed-offset time zones within inclusive bounds.
//
// It is made with Offsets of package [hegel.dev/go/hegel/temporal].
type OffsetGenerator struct {
	minVal      *time.Duration
	maxVal      *time.Duration
	granularity *Granularity
}

var _ Generator[*time.Location] = OffsetGenerator{}

func newOffsets() OffsetGenerator {
	return OffsetGenerator{}
}

// Min sets the inclusive minimum offset east of UTC.
//
// The offset must be a whole number of seconds.
func (g OffsetGenerator) Min(v time.Duration) OffsetGenerator {
	g.minVal = &v
	return g
}

// Max sets the inclusive maximum offset east of UTC.
//
// The offset must be a whole number of seconds.
func (g OffsetGenerator) Max(v time.Duration) OffsetGenerator {
	g.maxVal = &v
	return g
}

// Granularity makes every generated offset a whole number of unit.
func (g OffsetGenerator) Granularity(unit Granularity) OffsetGenerator {
	g.granularity = &unit
	return g
}

// offsetSeconds converts an offset bound to the engine's whole seconds. A
// bound too large for an int32 becomes the nearest one, which the engine
// rejects as out of range like any other.
func offsetSeconds(name string, v time.Duration) (int32, error) {
	if v%time.Second != 0 {
		return 0, fmt.Errorf("temporal.Offsets: %s must be a whole number of seconds, got %v", name, v)
	}
	return int32(min(max(int64(v/time.Second), math.MinInt32), math.MaxInt32)), nil
}

func (g OffsetGenerator) draw(tc TestCase) (*time.Location, error) {
	bounds := [2]time.Duration{minOffset, maxOffset}
	seconds := [2]int32{}
	for i, v := range [2]*time.Duration{g.minVal, g.maxVal} {
		if v != nil {
			bounds[i] = *v
		}
		var err error
		seconds[i], err = offsetSeconds([2]string{"Min", "Max"}[i], bounds[i])
		if err != nil {
			return nil, err
		}
	}
	granularity := libhegel.OFFSET_GRANULARITY_SECONDS
	if g.granularity != nil {
		granularity = *g.granularity
	}
	ctx, ltc := tc.engine()
	offset, err := ltc.GenerateOffset(ctx, seconds[0], seconds[1], granularity, false)
	if err != nil {
		return nil, err
	}
	name, err := ctx.FormatOffset(offset)
	if err != nil {
		return nil, err
	}
	return time.FixedZone(name, int(offset.Seconds)), nil
}

// ZoneGenerator generates time zones from a zone source.
//
// It is made with Zones of package [hegel.dev/go/hegel/temporal]. The zero
// value has no zone source: it fails when drawn from unless Source gives it
// one.
type ZoneGenerator struct {
	source  *zoneSource
	names   []string
	noLinks bool
	legacy  bool
}

var _ Generator[*time.Location] = ZoneGenerator{}

func newZones() ZoneGenerator {
	return ZoneGenerator{source: defaultZoneSourceImpl.Load()}
}

// Source sets the zone source the zones come from.
func (g ZoneGenerator) Source(source ZoneSource) ZoneGenerator {
	g.source = source.impl
	return g
}

// Names restricts the zones to the given names.
//
// Each name must be one the zone source knows. AllowLinks and AllowLegacy do
// not apply to an explicit list.
func (g ZoneGenerator) Names(names ...string) ZoneGenerator {
	g.names = append([]string{}, names...)
	return g
}

// AllowLinks sets whether link names such as Asia/Calcutta are generated.
//
// A link is a second name for a zone. They are generated by default.
func (g ZoneGenerator) AllowLinks(allow bool) ZoneGenerator {
	g.noLinks = !allow
	return g
}

// AllowLegacy sets whether legacy names such as EST5EDT and US/Eastern are generated.
//
// They are not generated by default.
func (g ZoneGenerator) AllowLegacy(allow bool) ZoneGenerator {
	g.legacy = allow
	return g
}

// spec returns the engine's time zone spec for the generator and a function
// to call once the draw that uses it is over.
func (g ZoneGenerator) spec(ctx *libhegel.Context) (*libhegel.TimezoneSpec, func(), error) {
	if g.names == nil {
		spec, err := g.source.sharedSpec(ctx, !g.noLinks, g.legacy)
		return spec, func() {}, err
	}
	spec, err := g.source.newSpec(ctx)
	if err != nil {
		return nil, nil, err
	}
	if err := spec.SetNames(ctx, g.names); err != nil {
		spec.Free()
		return nil, nil, err
	}
	return spec, spec.Free, nil
}

func (g ZoneGenerator) draw(tc TestCase) (*time.Location, error) {
	ctx, ltc := tc.engine()
	spec, done, err := g.spec(ctx)
	if err != nil {
		return nil, err
	}
	defer done()
	name, err := ltc.GenerateTimezone(ctx, spec)
	if err != nil {
		return nil, err
	}
	return g.source.locate(ctx, name)
}

// ZonedDatetimeGenerator generates times in generated time zones.
//
// It is made with ZonedDatetimes of package [hegel.dev/go/hegel/temporal]. The
// zero value has no zone source: it fails when drawn from unless Source gives
// it one.
type ZonedDatetimeGenerator struct {
	zones         Generator[*time.Location]
	source        *zoneSource
	hasSource     bool
	defaultSource *zoneSource
	minInstant    *time.Time
	maxInstant    *time.Time
	minWall       *time.Time
	maxWall       *time.Time
	precision     Precision
	ambiguous     Ambiguous
}

var _ Generator[time.Time] = ZonedDatetimeGenerator{}

func newZonedDatetimes() ZonedDatetimeGenerator {
	return ZonedDatetimeGenerator{defaultSource: defaultZoneSourceImpl.Load()}
}

// Zones sets the generator the time zones are drawn from.
//
// Given a [ZoneGenerator], the zones and their transitions come from that
// generator's zone source; also calling Source is then an error, so give the
// source to it, as temporal.Zones().Source(s). Given any other generator (a
// [ZoneGenerator] wrapped in [Filter] or [Map], [SampledFrom], [Just]), each
// drawn location is looked up by name in the source given to Source, or else
// in the default source read when the ZonedDatetimeGenerator was made. A name
// that source does not know is an error, never a silent fallback.
func (g ZonedDatetimeGenerator) Zones(zones Generator[*time.Location]) ZonedDatetimeGenerator {
	g.zones = zones
	return g
}

// Source sets the zone source the zones and their transitions come from.
//
// See [ZonedDatetimeGenerator.Zones] for how it combines with a zones
// generator.
func (g ZonedDatetimeGenerator) Source(source ZoneSource) ZonedDatetimeGenerator {
	g.source, g.hasSource = source.impl, true
	return g
}

// Min sets the earliest instant generated.
//
// The bound is the instant v stands for, whatever its location. Instant
// bounds cannot be combined with MinWall and MaxWall.
func (g ZonedDatetimeGenerator) Min(v time.Time) ZonedDatetimeGenerator {
	g.minInstant = &v
	return g
}

// Max sets the latest instant generated.
//
// The bound is the instant v stands for, whatever its location. Instant
// bounds cannot be combined with MinWall and MaxWall.
func (g ZonedDatetimeGenerator) Max(v time.Time) ZonedDatetimeGenerator {
	g.maxInstant = &v
	return g
}

// MinWall sets the earliest wall-clock time generated.
//
// The bound is v's date and clock reading, ignoring its location, and applies
// to what each value shows in its own zone. Wall-clock bounds cannot be
// combined with Min and Max.
func (g ZonedDatetimeGenerator) MinWall(v time.Time) ZonedDatetimeGenerator {
	g.minWall = &v
	return g
}

// MaxWall sets the latest wall-clock time generated.
//
// The bound is v's date and clock reading, ignoring its location, and applies
// to what each value shows in its own zone. Wall-clock bounds cannot be
// combined with Min and Max.
func (g ZonedDatetimeGenerator) MaxWall(v time.Time) ZonedDatetimeGenerator {
	g.maxWall = &v
	return g
}

// Precision makes every generated time of day a whole number of p.
func (g ZonedDatetimeGenerator) Precision(p Precision) ZonedDatetimeGenerator {
	g.precision = p
	return g
}

// Ambiguous sets which occurrence of a repeated wall-clock time is generated.
func (g ZonedDatetimeGenerator) Ambiguous(policy Ambiguous) ZonedDatetimeGenerator {
	g.ambiguous = policy
	return g
}

// The instants an instant bound that was not given defaults to: the first and
// last of the years 1 to 9999 in UTC.
var (
	fullInstantMin = engineTimestamp(time.Date(1, time.January, 1, 0, 0, 0, 0, time.UTC))
	fullInstantMax = engineTimestamp(time.Date(9999, time.December, 31, 23, 59, 59, 999999999, time.UTC))
)

func engineTimestamp(v time.Time) libhegel.Timestamp {
	return libhegel.Timestamp{Second: v.Unix(), Nanosecond: uint32(v.Nanosecond())}
}

// setBounds gives spec the generator's bounds: on the instant when an instant
// bound was set, and on the wall clock otherwise.
func (g ZonedDatetimeGenerator) setBounds(ctx *libhegel.Context, spec *libhegel.ZonedSpec) error {
	instant := g.minInstant != nil || g.maxInstant != nil
	if !instant {
		minVal, maxVal, err := wallClockBounds(g.minWall, g.maxWall)
		if err != nil {
			return err
		}
		return spec.SetCivilBounds(ctx, minVal, maxVal)
	}
	if g.minWall != nil || g.maxWall != nil {
		return errors.New("temporal.ZonedDatetimes: Min and Max bound the instant and MinWall and MaxWall bound the wall clock; set one kind of bound, not both")
	}
	bounds := [2]libhegel.Timestamp{fullInstantMin, fullInstantMax}
	for i, v := range [2]*time.Time{g.minInstant, g.maxInstant} {
		if v != nil {
			bounds[i] = engineTimestamp(*v)
		}
	}
	return spec.SetInstantBounds(ctx, bounds[0], bounds[1])
}

func (g ZonedDatetimeGenerator) spec(ctx *libhegel.Context) (*libhegel.ZonedSpec, error) {
	spec, err := ctx.ZonedSpecNew()
	if err != nil {
		return nil, err
	}
	err = firstError(
		func() error { return g.setBounds(ctx, spec) },
		func() error { return spec.SetPrecision(ctx, g.precision) },
		func() error { return spec.SetAmbiguous(ctx, g.ambiguous) },
		func() error { return spec.SetImaginary(ctx, libhegel.IMAGINARY_AVOID) },
	)
	if err != nil {
		spec.Free()
		return nil, err
	}
	return spec, nil
}

// zonedTime builds the generated time from the instant the engine drew and
// the location. When Go reads that instant at another wall-clock time than
// the engine drew, the zone source and the location disagree about the zone,
// and the test case is rejected rather than handed a time the bounds and
// policies were not applied to.
func zonedTime(value libhegel.ZonedDatetime, loc *time.Location) (time.Time, error) {
	t := time.Unix(value.Timestamp.Second, int64(value.Timestamp.Nanosecond)).In(loc)
	if _, offset := t.Zone(); offset != int(value.OffsetSeconds) {
		return time.Time{}, libhegel.E_ASSUME
	}
	return t, nil
}

func (g ZonedDatetimeGenerator) draw(tc TestCase) (time.Time, error) {
	source := g.defaultSource
	if g.hasSource {
		source = g.source
	}
	zones, builtin := g.zones.(ZoneGenerator)
	switch {
	case g.zones == nil:
		zones, builtin = ZoneGenerator{source: source}, true
	case builtin && g.hasSource:
		return time.Time{}, errors.New("temporal.ZonedDatetimes: Zones was given a temporal.Zones() generator, which has a zone source of its own; " +
			"give the source to it, as temporal.Zones().Source(...), not to temporal.ZonedDatetimes().Source(...)")
	}
	ctx, ltc := tc.engine()
	spec, err := g.spec(ctx)
	if err != nil {
		return time.Time{}, err
	}
	defer spec.Free()

	if builtin {
		tzSpec, done, err := zones.spec(ctx)
		if err != nil {
			return time.Time{}, err
		}
		defer done()
		value, name, err := ltc.GenerateZonedDatetime(ctx, tzSpec, spec)
		if err != nil {
			return time.Time{}, err
		}
		loc, err := zones.source.locate(ctx, name)
		if err != nil {
			return time.Time{}, err
		}
		return zonedTime(value, loc)
	}

	loc, err := draw(tc, g.zones)
	if err != nil {
		return time.Time{}, err
	}
	if loc == nil {
		return time.Time{}, errors.New("temporal.ZonedDatetimes: the zones generator drew a nil *time.Location")
	}
	handle, err := source.engine(ctx)
	if err != nil {
		return time.Time{}, err
	}
	name := loc.String()
	info, err := ctx.TimezoneInfoIn(handle, name)
	if err != nil {
		return time.Time{}, err
	}
	if !info.Known {
		return time.Time{}, fmt.Errorf("temporal.ZonedDatetimes: the zone source does not know the zone %q that the zones generator drew; "+
			"pass a source that has it to Source, or to temporal.SetDefaultZoneSource before the generator is built", name)
	}
	value, err := ltc.GenerateZonedDatetimeForZone(ctx, handle, name, spec)
	if err != nil {
		return time.Time{}, err
	}
	return zonedTime(value, loc)
}
