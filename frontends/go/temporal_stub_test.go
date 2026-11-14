package hegel

// temporal_stub_test.go drives the temporal generators against a scripted
// libhegel, for the engine failures the real engine does not produce on
// demand: a constructor or setter that fails after the calls before it
// succeeded. Each case scripts the calls up to the failing one, and the error
// has to be the failing call's.

import (
	"testing"
	"time"

	"hegel.dev/go/hegel/internal/libhegel"
)

// Scripted results of the calls the generators make, in the stub's terms: a
// produced handle or value, then the call's result.
var (
	stubHandle = []any{uintptr(9), libhegel.OK}
	stubOK     = []any{libhegel.OK}
	// A zone source built from locations opens with a new source, the
	// declared names and the loader.
	stubOpenSource = script(stubHandle, stubOK, stubOK)
	// A zoned datetime spec takes its bounds, precision and two policies.
	stubZonedSpec = script(stubHandle, stubOK, stubOK, stubOK, stubOK)
	// A draw from another generator is wrapped in a span.
	stubSpan = script(stubOK, stubOK)
)

func script(parts ...[]any) []any {
	var all []any
	for _, part := range parts {
		all = append(all, part...)
	}
	return all
}

func stubFailure(produced ...any) []any {
	return append(produced, libhegel.E_BACKEND, "boom")
}

// drawError returns a function that draws from g and reports only the error.
func drawError[T any](g Generator[T]) func(TestCase) error {
	return func(tc TestCase) error {
		_, err := g.draw(tc)
		return err
	}
}

func TestTemporalEngineFailures(t *testing.T) {
	t.Parallel()
	utcOnly := func() ZoneSource { return ZoneSourceFromLocations(time.UTC) }
	for _, tt := range []struct {
		op     string
		draw   func(TestCase) error
		script []any
	}{
		{
			"hegel_date_options_new",
			drawError(Dates().Weekdays(time.Monday)),
			stubFailure(uintptr(0)),
		},
		{
			"hegel_format_offset",
			drawError(Offsets()),
			script([]any{libhegel.Offset{Seconds: 3600, Known: true}, libhegel.OK}, stubFailure("")),
		},
		{
			"hegel_zone_source_new",
			drawError(Zones().Source(utcOnly())),
			stubFailure(uintptr(0)),
		},
		{
			"hegel_zone_source_declare",
			drawError(Zones().Source(utcOnly())),
			script(stubHandle, stubFailure()),
		},
		{
			"hegel_zone_source_set_loader",
			drawError(Zones().Source(utcOnly())),
			script(stubHandle, stubOK, stubFailure()),
		},
		{
			"hegel_timezone_spec_new",
			drawError(Zones().Source(utcOnly())),
			script(stubOpenSource, stubFailure(uintptr(0))),
		},
		{
			"hegel_timezone_spec_set_source",
			drawError(Zones().Source(utcOnly())),
			script(stubOpenSource, stubHandle, stubFailure()),
		},
		{
			"hegel_timezone_spec_set_links",
			drawError(Zones().Source(utcOnly())),
			script(stubOpenSource, stubHandle, stubOK, stubFailure()),
		},
		{
			"hegel_timezone_spec_set_legacy",
			drawError(Zones().Source(utcOnly())),
			script(stubOpenSource, stubHandle, stubOK, stubOK, stubFailure()),
		},
		{
			"hegel_zoned_spec_new",
			drawError(ZonedDatetimes().Source(utcOnly())),
			stubFailure(uintptr(0)),
		},
		{
			"hegel_zoned_spec_set_imaginary",
			drawError(ZonedDatetimes().Source(utcOnly())),
			script(stubHandle, stubOK, stubOK, stubOK, stubFailure()),
		},
		{
			"hegel_timezone_info_in",
			drawError(ZonedDatetimes().Zones(Just(time.UTC)).Source(utcOnly())),
			script(stubZonedSpec, stubSpan, stubOpenSource, stubFailure(libhegel.TimezoneInfo{})),
		},
		{
			"hegel_generate_zoned_datetime_for_zone",
			drawError(ZonedDatetimes().Zones(Just(time.UTC)).Source(utcOnly())),
			script(stubZonedSpec, stubSpan, stubOpenSource, []any{libhegel.TimezoneInfo{Known: true}, libhegel.OK}, stubFailure(libhegel.ZonedDatetime{})),
		},
	} {
		assertErrorContains(t, tt.op+": E_BACKEND: boom", tt.draw(newStubTestCase(t, tt.script...)))
	}
}

// TestZonedDatetimesStubbedDraw scripts a whole zoned draw, checking the
// value is built from the instant the engine reports and the location the
// source holds, and that no handle is used once it is freed.
func TestZonedDatetimesStubbedDraw(t *testing.T) {
	t.Parallel()
	tokyo := time.FixedZone("Test/Tokyo", 9*3600)
	source := ZoneSourceFromLocations(tokyo)
	drawn := libhegel.ZonedDatetime{OffsetSeconds: 9 * 3600, OffsetKnown: true, Timestamp: libhegel.Timestamp{Second: 86400, Nanosecond: 7}}
	tc := newStubTestCase(t, script(
		stubZonedSpec,
		stubOpenSource,
		stubHandle, stubOK, stubOK, stubOK,
		[]any{drawn, "Test/Tokyo", libhegel.OK},
		stubZonedSpec,
		[]any{drawn, "Test/Tokyo", libhegel.OK},
	)...)
	for range 2 {
		got, err := ZonedDatetimes().Source(source).draw(tc)
		if err != nil || got.Location() != tokyo || !got.Equal(time.Unix(86400, 7)) || got.Hour() != 9 {
			t.Fatalf("drew %v, %v", got, err)
		}
	}
}

// TestHostZoneNamesEngineFailures fails each call that lists the bundled
// database's names, which is how the host zone source finds its own.
func TestHostZoneNamesEngineFailures(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		op     string
		script []any
	}{
		{"hegel_zone_source_bundled", stubFailure(uintptr(0))},
		{"hegel_zone_source_name_count", script(stubHandle, stubFailure(uint64(0)))},
		{"hegel_zone_source_name", script(stubHandle, []any{uint64(1), libhegel.OK}, stubFailure(""))},
	} {
		zones := newGoZones(time.LoadLocation)
		_, err := Zones().Source(ZoneSource{zones.hostSource()}).draw(newStubTestCase(t, tt.script...))
		assertErrorContains(t, tt.op+": E_BACKEND: boom", err)
	}
}
