package hegel_test

import (
	"testing"
	"time"

	"hegel.dev/go/hegel"
)

func ExampleZonedDatetimes() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		at := hegel.Draw(ht, hegel.ZonedDatetimes().
			Zones(hegel.Zones().Names("America/New_York", "Europe/London")).
			Precision(hegel.PrecisionSeconds))
		nextDay := at.AddDate(0, 0, 1)
		if nextDay.Sub(at) != 24*time.Hour {
			ht.Fatalf("the day after %v is not 24 hours later", at)
		}
	})
}

var officeZones = hegel.ZoneSourceFromLocations(
	time.FixedZone("Office", 5*60*60),
	time.UTC,
)

func ExampleZoneSourceFromLocations() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		at := hegel.Draw(ht, hegel.ZonedDatetimes().Source(officeZones))
		_ = at // at is a time.Time in Office or UTC
	})
}

func ExampleDurations() {
	t := &testing.T{}
	hegel.Test(t, func(ht *hegel.T) {
		timeout := hegel.Draw(ht, hegel.Durations().
			Min(0).
			Max(time.Hour).
			Precision(hegel.PrecisionMilliseconds))
		_ = timeout // timeout is a time.Duration
	})
}
