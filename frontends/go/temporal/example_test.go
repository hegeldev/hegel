package temporal_test

import (
	"testing"
	"time"

	"hegel.dev/go/hegel"
	"hegel.dev/go/hegel/temporal"
)

func Example() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		day := hegel.Draw(ht, temporal.Dates().Weekdays(time.Saturday, time.Sunday))
		start := hegel.Draw(ht, temporal.Datetimes().Precision(temporal.PrecisionMinutes))
		timeout := hegel.Draw(ht, temporal.Durations().Min(0).Max(time.Hour))
		loc := hegel.Draw(ht, temporal.Zones().AllowLinks(false))
		offset := hegel.Draw(ht, temporal.Offsets().Granularity(temporal.GranularityQuarterHours))
		at := hegel.Draw(ht, temporal.ZonedDatetimes().Ambiguous(temporal.AmbiguousEarlier))
		_, _, _, _, _, _ = day, start, timeout, loc, offset, at
	})
}

func ExampleZonedDatetimes() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		at := hegel.Draw(ht, temporal.ZonedDatetimes().
			Zones(temporal.Zones().Names("America/New_York", "Europe/London")).
			Precision(temporal.PrecisionSeconds))
		nextDay := at.AddDate(0, 0, 1)
		if nextDay.Sub(at) != 24*time.Hour {
			ht.Fatalf("the day after %v is not 24 hours later", at)
		}
	})
}

var officeZones = temporal.ZoneSourceFromLocations(
	time.FixedZone("Office", 5*60*60),
	time.UTC,
)

func ExampleZoneSourceFromLocations() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		at := hegel.Draw(ht, temporal.ZonedDatetimes().Source(officeZones))
		_ = at // at is a time.Time in Office or UTC
	})
}

func ExampleDurations() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		timeout := hegel.Draw(ht, temporal.Durations().
			Min(0).
			Max(time.Hour).
			Precision(temporal.PrecisionMilliseconds))
		_ = timeout // timeout is a time.Duration
	})
}
