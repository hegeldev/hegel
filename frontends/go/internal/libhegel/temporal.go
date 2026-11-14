package libhegel

import (
	"runtime"
)

type OffsetGranularity uint32 // Equivalent of hegel_offset_granularity_t (passed as a uint32_t param)

const (
	OFFSET_GRANULARITY_HOURS OffsetGranularity = iota
	OFFSET_GRANULARITY_QUARTER_HOURS
	OFFSET_GRANULARITY_MINUTES
	OFFSET_GRANULARITY_SECONDS
)

type Ambiguous uint32 // Equivalent of hegel_ambiguous_t (passed as a uint32_t param)

const (
	// Either instant of a repeated wall time, decided by a drawn fold.
	AMBIGUOUS_BOTH Ambiguous = iota

	// The earlier instant.
	AMBIGUOUS_EARLIER

	// The later instant.
	AMBIGUOUS_LATER
)

type Imaginary uint32 // Equivalent of hegel_imaginary_t (passed as a uint32_t param)

const (
	// A wall time the clocks skipped is reported as a gap.
	IMAGINARY_ALLOW Imaginary = iota

	// A wall time the clocks skipped is never drawn.
	IMAGINARY_AVOID
)

type ZoneAfter uint32 // Equivalent of hegel_zone_after_t (a uint32_t field of hegel_zone_oracle_t)

const (
	// The last type holds for ever.
	ZONE_AFTER_HOLD ZoneAfter = iota

	// A yearly rule given explicitly; structured zone data only.
	ZONE_AFTER_RULE

	// A yearly rule inferred from the last two years the data covers.
	ZONE_AFTER_INFER
)

type TimezoneKind int32 // Equivalent of hegel_timezone_kind_t

const (
	// The wall time names exactly one instant.
	TIMEZONE_KIND_UNIQUE TimezoneKind = iota

	// The clocks skipped the wall time.
	TIMEZONE_KIND_GAP

	// The clocks repeated the wall time.
	TIMEZONE_KIND_FOLD
)

// Timestamp mirrors hegel_timestamp_t: an instant as seconds and nanoseconds
// since the Unix epoch.
type Timestamp struct {
	Second     int64
	Nanosecond uint32
	_          uint32 // pad to match C abi (purego limitation)
}

// Duration mirrors hegel_duration_t: a signed span of seconds and nanoseconds
// whose two fields never differ in sign.
type Duration struct {
	Seconds     int64
	Nanoseconds int32
	_           int32 // pad to match C abi (purego limitation)
}

// Offset mirrors hegel_offset_t: a UTC offset in seconds east of UT, or the
// unknown offset when Known is false.
type Offset struct {
	Seconds int32
	Known   bool
	_       uint8  // pad to match C abi (purego limitation)
	_       uint16 // pad to match C abi (purego limitation)
}

// ZonedDatetime mirrors hegel_zoned_datetime_t: a wall-clock time in a zone
// and how the zone's data resolves it.
type ZonedDatetime struct {
	Civil         Datetime
	OffsetSeconds int32
	OffsetKnown   bool
	Fold          bool
	_             uint16
	Kind          TimezoneKind
	OffsetBefore  int32
	OffsetAfter   int32
	_             uint32
	Timestamp     Timestamp
}

// timezoneInfoT mirrors hegel_timezone_info_t.
type timezoneInfoT struct {
	known       bool
	primaryName stringT
	isLink      bool
	rank        uint16
	tags        uint32
}

// TimezoneInfo is what a zone source knows about a time zone name.
type TimezoneInfo struct {
	Known       bool
	PrimaryName string
	IsLink      bool
	Rank        uint16
	Tags        uint32
}

// DateOptions wraps a hegel_date_options_t: the weekday and month masks of
// [TestCase.GenerateDate].
type DateOptions struct {
	pointer[dateOptionsT]
}

// DateOptionsNew creates date options allowing every weekday and month.
func (c *Context) DateOptionsNew() (*DateOptions, error) {
	opts := new(DateOptions)
	ok, err := allocateInto(c, &opts.pointer, "hegel_date_options_new", func(ctx ctxT, raw *dateOptionsT) Error {
		return c.syms.DateOptionsNew(ctx, raw)
	}, c.syms.DateOptionsFree)
	if !ok {
		return nil, err
	}
	return opts, nil
}

// SetWeekdays restricts the weekdays drawn to those of mask, whose bit 0 is
// Monday and bit 6 Sunday.
func (o *DateOptions) SetWeekdays(ctx *Context, mask uint32) error {
	return ctx.invoke("hegel_date_options_set_weekdays", func(ctx ctxT) Error {
		e := o.syms.DateOptionsSetWeekdays(ctx, o.raw, mask)
		runtime.KeepAlive(o)
		return e
	})
}

// SetMonths restricts the months drawn to those of mask, whose bit 0 is
// January and bit 11 December.
func (o *DateOptions) SetMonths(ctx *Context, mask uint32) error {
	return ctx.invoke("hegel_date_options_set_months", func(ctx ctxT) Error {
		e := o.syms.DateOptionsSetMonths(ctx, o.raw, mask)
		runtime.KeepAlive(o)
		return e
	})
}

// GenerateDuration draws a duration in [minValue, maxValue] that is a whole
// number of precision units.
func (tc *TestCase) GenerateDuration(ctx *Context, minValue, maxValue Duration, precision Precision) (Duration, error) {
	err := ctx.invoke("hegel_generate_duration", func(ctx ctxT) Error {
		e := tc.syms.GenerateDuration(ctx, tc.raw, minValue, maxValue, precision, &tc.outDuration)
		runtime.KeepAlive(tc)
		return e
	})
	return tc.outDuration, err
}

// GenerateOffset draws a UTC offset in [minSeconds, maxSeconds] that is a whole
// number of granularity units. With allowUnknown the draw may also yield the
// unknown offset.
func (tc *TestCase) GenerateOffset(ctx *Context, minSeconds, maxSeconds int32, granularity OffsetGranularity, allowUnknown bool) (Offset, error) {
	err := ctx.invoke("hegel_generate_offset", func(ctx ctxT) Error {
		e := tc.syms.GenerateOffset(ctx, tc.raw, minSeconds, maxSeconds, granularity, allowUnknown, &tc.outOffset)
		runtime.KeepAlive(tc)
		return e
	})
	return tc.outOffset, err
}

// FormatOffset returns the canonical text of a UTC offset, such as "+05:30".
func (c *Context) FormatOffset(value Offset) (string, error) {
	var res ownedStringT
	err := c.invoke("hegel_format_offset", func(ctx ctxT) Error {
		return c.syms.FormatOffset(ctx, value, &res)
	})
	if err != nil {
		return "", err
	}
	s := goString(stringT(res))
	_ = c.syms.OwnedStringFree(0, &res)
	return s, nil
}

// TimezoneSpec wraps a hegel_timezone_spec_t: which time zone names a draw
// allows and the zone source they come from.
type TimezoneSpec struct {
	pointer[timezoneSpecT]
}

// TimezoneSpecNew creates a spec allowing every name of the process's default
// zone source, links included and legacy names excluded.
func (c *Context) TimezoneSpecNew() (*TimezoneSpec, error) {
	spec := new(TimezoneSpec)
	ok, err := allocateInto(c, &spec.pointer, "hegel_timezone_spec_new", func(ctx ctxT, raw *timezoneSpecT) Error {
		return c.syms.TimezoneSpecNew(ctx, raw)
	}, c.syms.TimezoneSpecFree)
	if !ok {
		return nil, err
	}
	return spec, nil
}

// SetNames restricts the draw to an explicit list of names.
func (s *TimezoneSpec) SetNames(ctx *Context, names []string) error {
	namesPtr, namesLen := cStringArrayArg(names)
	return ctx.invoke("hegel_timezone_spec_set_names", func(ctx ctxT) Error {
		e := s.syms.TimezoneSpecSetNames(ctx, s.raw, namesPtr, namesLen)
		runtime.KeepAlive(s)
		return e
	})
}

// SetLinks sets whether the default set includes link names.
func (s *TimezoneSpec) SetLinks(ctx *Context, links bool) error {
	return ctx.invoke("hegel_timezone_spec_set_links", func(ctx ctxT) Error {
		e := s.syms.TimezoneSpecSetLinks(ctx, s.raw, links)
		runtime.KeepAlive(s)
		return e
	})
}

// SetLegacy sets whether the default set includes legacy names.
func (s *TimezoneSpec) SetLegacy(ctx *Context, legacy bool) error {
	return ctx.invoke("hegel_timezone_spec_set_legacy", func(ctx ctxT) Error {
		e := s.syms.TimezoneSpecSetLegacy(ctx, s.raw, legacy)
		runtime.KeepAlive(s)
		return e
	})
}

// SetSource makes the spec draw from, and evaluate with, source. A nil source
// is the process's default zone source as it is now.
func (s *TimezoneSpec) SetSource(ctx *Context, source *ZoneSource) error {
	var sourceRaw zoneSourceT
	if source != nil {
		sourceRaw = source.raw
	}
	return ctx.invoke("hegel_timezone_spec_set_source", func(ctx ctxT) Error {
		e := s.syms.TimezoneSpecSetSource(ctx, s.raw, sourceRaw)
		runtime.KeepAlive(s)
		runtime.KeepAlive(source)
		return e
	})
}

// GenerateTimezone draws a time zone name allowed by spec.
func (tc *TestCase) GenerateTimezone(ctx *Context, spec *TimezoneSpec) (string, error) {
	err := ctx.invoke("hegel_generate_timezone", func(ctx ctxT) Error {
		e := tc.syms.GenerateTimezone(ctx, tc.raw, spec.raw, &tc.outStringRes)
		runtime.KeepAlive(tc)
		runtime.KeepAlive(spec)
		return e
	})
	if err != nil {
		return "", err
	}
	s := goString(stringT(tc.outStringRes))
	_ = tc.syms.OwnedStringFree(0, &tc.outStringRes)
	return s, nil
}

// ZonedSpec wraps a hegel_zoned_spec_t: the bounds, precision and gap and
// fold policies of a zoned datetime draw.
type ZonedSpec struct {
	pointer[zonedSpecT]
}

// ZonedSpecNew creates a zoned datetime spec with every option at its default.
func (c *Context) ZonedSpecNew() (*ZonedSpec, error) {
	spec := new(ZonedSpec)
	ok, err := allocateInto(c, &spec.pointer, "hegel_zoned_spec_new", func(ctx ctxT, raw *zonedSpecT) Error {
		return c.syms.ZonedSpecNew(ctx, raw)
	}, c.syms.ZonedSpecFree)
	if !ok {
		return nil, err
	}
	return spec, nil
}

// SetCivilBounds bounds the draw on the wall clock of whatever zone is drawn.
func (s *ZonedSpec) SetCivilBounds(ctx *Context, minValue, maxValue Datetime) error {
	return ctx.invoke("hegel_zoned_spec_set_civil_bounds", func(ctx ctxT) Error {
		e := s.syms.ZonedSpecSetCivilBounds(ctx, s.raw, minValue, maxValue)
		runtime.KeepAlive(s)
		return e
	})
}

// SetInstantBounds bounds the draw on the instant.
func (s *ZonedSpec) SetInstantBounds(ctx *Context, minValue, maxValue Timestamp) error {
	return ctx.invoke("hegel_zoned_spec_set_instant_bounds", func(ctx ctxT) Error {
		e := s.syms.ZonedSpecSetInstantBounds(ctx, s.raw, minValue, maxValue)
		runtime.KeepAlive(s)
		return e
	})
}

// SetPrecision sets the unit the time of day is counted in.
func (s *ZonedSpec) SetPrecision(ctx *Context, precision Precision) error {
	return ctx.invoke("hegel_zoned_spec_set_precision", func(ctx ctxT) Error {
		e := s.syms.ZonedSpecSetPrecision(ctx, s.raw, precision)
		runtime.KeepAlive(s)
		return e
	})
}

// SetImaginary sets what the draw does with a wall time the clocks skipped.
func (s *ZonedSpec) SetImaginary(ctx *Context, imaginary Imaginary) error {
	return ctx.invoke("hegel_zoned_spec_set_imaginary", func(ctx ctxT) Error {
		e := s.syms.ZonedSpecSetImaginary(ctx, s.raw, imaginary)
		runtime.KeepAlive(s)
		return e
	})
}

// SetAmbiguous sets which instant the draw means by a wall time the clocks
// repeated.
func (s *ZonedSpec) SetAmbiguous(ctx *Context, ambiguous Ambiguous) error {
	return ctx.invoke("hegel_zoned_spec_set_ambiguous", func(ctx ctxT) Error {
		e := s.syms.ZonedSpecSetAmbiguous(ctx, s.raw, ambiguous)
		runtime.KeepAlive(s)
		return e
	})
}

// GenerateZonedDatetime draws a zoned datetime in a zone allowed by tzSpec
// within zonedSpec's bounds, and returns it with the name of the zone drawn.
func (tc *TestCase) GenerateZonedDatetime(ctx *Context, tzSpec *TimezoneSpec, zonedSpec *ZonedSpec) (ZonedDatetime, string, error) {
	err := ctx.invoke("hegel_generate_zoned_datetime", func(ctx ctxT) Error {
		e := tc.syms.GenerateZonedDatetime(ctx, tc.raw, tzSpec.raw, zonedSpec.raw, &tc.outZoned, &tc.outStringRes)
		runtime.KeepAlive(tc)
		runtime.KeepAlive(tzSpec)
		runtime.KeepAlive(zonedSpec)
		return e
	})
	if err != nil {
		return ZonedDatetime{}, "", err
	}
	zone := goString(stringT(tc.outStringRes))
	_ = tc.syms.OwnedStringFree(0, &tc.outStringRes)
	return tc.outZoned, zone, nil
}

// GenerateZonedDatetimeForZone draws a zoned datetime within zonedSpec's
// bounds in the zone source knows as name, recording no choice of zone. A nil
// source is the process's default zone source as it is at this call.
func (tc *TestCase) GenerateZonedDatetimeForZone(ctx *Context, source *ZoneSource, name string, zonedSpec *ZonedSpec) (ZonedDatetime, error) {
	var sourceRaw zoneSourceT
	if source != nil {
		sourceRaw = source.raw
	}
	err := ctx.invoke("hegel_generate_zoned_datetime_for_zone", func(ctx ctxT) Error {
		e := tc.syms.GenerateZonedDatetimeForZone(ctx, tc.raw, sourceRaw, cString(name), zonedSpec.raw, &tc.outZoned)
		runtime.KeepAlive(tc)
		runtime.KeepAlive(source)
		runtime.KeepAlive(zonedSpec)
		return e
	})
	return tc.outZoned, err
}
