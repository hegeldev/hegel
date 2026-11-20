package temporal_test

import (
	"slices"
	"testing"
	"time"
	_ "time/tzdata"

	"hegel.dev/go/hegel"
	"hegel.dev/go/hegel/temporal"
)

var (
	_ hegel.Generator[time.Time]      = temporal.Dates()
	_ hegel.Generator[time.Time]      = temporal.Datetimes()
	_ hegel.Generator[time.Duration]  = temporal.Durations()
	_ hegel.Generator[*time.Location] = temporal.Offsets()
	_ hegel.Generator[*time.Location] = temporal.Zones()
	_ hegel.Generator[time.Time]      = temporal.ZonedDatetimes()
)

func mustLoad(t *testing.T, name string) *time.Location {
	t.Helper()
	loc, err := time.LoadLocation(name)
	if err != nil {
		t.Fatal(err)
	}
	return loc
}

func TestDates(t *testing.T) {
	t.Parallel()
	first := time.Date(2024, time.February, 1, 0, 0, 0, 0, time.UTC)
	last := time.Date(2024, time.March, 31, 0, 0, 0, 0, time.UTC)
	hegel.Test(t, func(ht *hegel.T) {
		got := hegel.Draw(ht, temporal.Dates())
		if got.Location() != time.UTC || !got.Equal(got.Truncate(24*time.Hour)) || got.Year() < 1 || got.Year() > 9999 {
			ht.Fatalf("Dates() drew %v, want midnight UTC in the years 1 to 9999", got)
		}
		leapDay := hegel.Draw(ht, temporal.Dates().Min(first).Max(last).Months(time.February).Weekdays(time.Thursday))
		if leapDay.Before(first) || leapDay.Month() != time.February || leapDay.Weekday() != time.Thursday {
			ht.Fatalf("drew %v, want a Thursday in February 2024", leapDay)
		}
	}, hegel.WithTestCases(100))
}

func TestDatetimes(t *testing.T) {
	t.Parallel()
	first := time.Date(2024, time.May, 5, 12, 0, 0, 0, time.UTC)
	last := first.Add(48 * time.Hour)
	hegel.Test(t, func(ht *hegel.T) {
		got := hegel.Draw(ht, temporal.Datetimes())
		if got.Location() != time.UTC || got.Year() < 1 || got.Year() > 9999 {
			ht.Fatalf("Datetimes() drew %v, want a UTC time in the years 1 to 9999", got)
		}
		bounded := hegel.Draw(ht, temporal.Datetimes().Min(first).Max(last).Precision(temporal.PrecisionHours))
		if bounded.Before(first) || bounded.After(last) || !bounded.Equal(bounded.Truncate(time.Hour)) {
			ht.Fatalf("drew %v, want a whole hour from %v to %v", bounded, first, last)
		}
	}, hegel.WithTestCases(100))
}

func TestPrecisions(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		precision temporal.Precision
		unit      time.Duration
	}{
		{temporal.PrecisionNanoseconds, time.Nanosecond},
		{temporal.PrecisionMicroseconds, time.Microsecond},
		{temporal.PrecisionMilliseconds, time.Millisecond},
		{temporal.PrecisionSeconds, time.Second},
		{temporal.PrecisionMinutes, time.Minute},
		{temporal.PrecisionHours, time.Hour},
	} {
		hegel.Test(t, func(ht *hegel.T) {
			g := temporal.Durations().Precision(tt.precision)
			if got := hegel.Draw(ht, g.Min(tt.unit).Max(tt.unit)); got != tt.unit {
				ht.Fatalf("precision %v: drew %v from a range holding only %v", tt.unit, got, tt.unit)
			}
			if got := hegel.Draw(ht, g.Min(-999*tt.unit).Max(999*tt.unit)); got%tt.unit != 0 || got < -999*tt.unit || got > 999*tt.unit {
				ht.Fatalf("precision %v: drew %v", tt.unit, got)
			}
		}, hegel.WithTestCases(100))
	}
	var zero temporal.Precision
	if zero != temporal.PrecisionNanoseconds {
		t.Fatal("the zero Precision is not PrecisionNanoseconds")
	}
}

func TestGranularities(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		granularity temporal.Granularity
		unit        time.Duration
	}{
		{temporal.GranularityHours, time.Hour},
		{temporal.GranularityQuarterHours, 15 * time.Minute},
		{temporal.GranularityMinutes, time.Minute},
		{temporal.GranularitySeconds, time.Second},
	} {
		hegel.Test(t, func(ht *hegel.T) {
			g := temporal.Offsets().Granularity(tt.granularity)
			only := hegel.Draw(ht, g.Min(tt.unit).Max(tt.unit))
			if _, offset := time.Unix(0, 0).In(only).Zone(); time.Duration(offset)*time.Second != tt.unit {
				ht.Fatalf("granularity %v: drew %v from a range holding only %v", tt.unit, only, tt.unit)
			}
			loc := hegel.Draw(ht, g.Min(-12*time.Hour).Max(14*time.Hour))
			_, seconds := time.Unix(0, 0).In(loc).Zone()
			offset := time.Duration(seconds) * time.Second
			if offset%tt.unit != 0 || offset < -12*time.Hour || offset > 14*time.Hour {
				ht.Fatalf("granularity %v: drew the offset %v", tt.unit, offset)
			}
		}, hegel.WithTestCases(100))
	}
}

func TestOffsetsAreNamedAfterTheOffset(t *testing.T) {
	t.Parallel()
	hegel.Test(t, func(ht *hegel.T) {
		loc := hegel.Draw(ht, temporal.Offsets().Min(330*time.Minute).Max(330*time.Minute))
		if loc.String() != "+05:30" {
			ht.Fatalf("the offset 5h30m is named %q", loc)
		}
	}, hegel.WithTestCases(10))
}

func TestZones(t *testing.T) {
	t.Parallel()
	seen := map[string]bool{}
	hegel.Test(t, func(ht *hegel.T) {
		seen[hegel.Draw(ht, temporal.Zones()).String()] = true
		named := hegel.Draw(ht, temporal.Zones().Names("Europe/Paris", "Asia/Tokyo"))
		if name := named.String(); name != "Europe/Paris" && name != "Asia/Tokyo" {
			ht.Fatalf("Names drew %q", name)
		}
	}, hegel.WithTestCases(100))
	if len(seen) < 20 {
		t.Fatalf("100 draws of Zones() gave only %v", seen)
	}
}

// TestAmbiguousPolicies draws inside the hour New York repeated on
// 2021-11-07, first on daylight time and then on standard time.
func TestAmbiguousPolicies(t *testing.T) {
	t.Parallel()
	const daylight, standard = -4 * 3600, -5 * 3600
	newYork := mustLoad(t, "America/New_York")
	inTheFold := temporal.ZonedDatetimes().
		Zones(temporal.Zones().Names("America/New_York")).
		MinWall(time.Date(2021, time.November, 7, 1, 0, 0, 0, time.UTC)).
		MaxWall(time.Date(2021, time.November, 7, 1, 59, 0, 0, time.UTC)).
		Precision(temporal.PrecisionMinutes)
	for _, tt := range []struct {
		name   string
		g      temporal.ZonedDatetimeGenerator
		want   []int
		policy temporal.Ambiguous
	}{
		{name: "default", g: inTheFold, want: []int{standard, daylight}},
		{name: "both", g: inTheFold.Ambiguous(temporal.AmbiguousBoth), want: []int{standard, daylight}},
		{name: "earlier", g: inTheFold.Ambiguous(temporal.AmbiguousEarlier), want: []int{daylight}},
		{name: "later", g: inTheFold.Ambiguous(temporal.AmbiguousLater), want: []int{standard}},
	} {
		seen := map[int]bool{}
		hegel.Test(t, func(ht *hegel.T) {
			got := hegel.Draw(ht, tt.g)
			if got.Location().String() != newYork.String() || got.Day() != 7 || got.Hour() != 1 || got.Second() != 0 {
				ht.Fatalf("%s: drew %v, outside the repeated hour in New York", tt.name, got)
			}
			_, offset := got.Zone()
			seen[offset] = true
		}, hegel.WithTestCases(100))
		var offsets []int
		for offset := range seen {
			offsets = append(offsets, offset)
		}
		slices.Sort(offsets)
		if !slices.Equal(offsets, tt.want) {
			t.Errorf("%s: offsets %v, want %v", tt.name, offsets, tt.want)
		}
	}
}

func TestZonedDatetimesInstantBounds(t *testing.T) {
	t.Parallel()
	first := time.Date(2024, time.May, 5, 12, 0, 0, 0, time.UTC)
	last := first.Add(time.Hour)
	hegel.Test(t, func(ht *hegel.T) {
		got := hegel.Draw(ht, temporal.ZonedDatetimes().Min(first).Max(last))
		if got.Before(first) || got.After(last) {
			ht.Fatalf("drew %v, want an instant from %v to %v", got, first, last)
		}
	}, hegel.WithTestCases(100))
}

func drawnZones(t *testing.T, g hegel.Generator[time.Time]) []string {
	t.Helper()
	seen := map[string]bool{}
	hegel.Test(t, func(ht *hegel.T) {
		seen[hegel.Draw(ht, g).Location().String()] = true
	}, hegel.WithTestCases(100))
	var names []string
	for name := range seen {
		names = append(names, name)
	}
	slices.Sort(names)
	return names
}

func TestBuiltInZoneSources(t *testing.T) {
	t.Parallel()
	host, bundled := temporal.HostZoneSource(), temporal.BundledZoneSource()
	if host != temporal.HostZoneSource() || bundled != temporal.BundledZoneSource() || host == bundled {
		t.Fatal("the host and bundled zone sources are not two stable, distinct sources")
	}
	if host == (temporal.ZoneSource{}) || bundled == (temporal.ZoneSource{}) {
		t.Fatal("a built-in zone source is the zero ZoneSource")
	}
	for name, source := range map[string]temporal.ZoneSource{"host": host, "bundled": bundled} {
		paris := temporal.ZonedDatetimes().Zones(temporal.Zones().Source(source).Names("Europe/Paris"))
		if got := drawnZones(t, paris); !slices.Equal(got, []string{"Europe/Paris"}) {
			t.Errorf("%s source: drew the zones %v", name, got)
		}
		if got := drawnZones(t, temporal.ZonedDatetimes().Source(source)); len(got) < 20 {
			t.Errorf("%s source: 100 draws used only the zones %v", name, got)
		}
	}
}

func TestZoneSourceFromLocations(t *testing.T) {
	t.Parallel()
	office := time.FixedZone("Office", 5*60*60)
	source := temporal.ZoneSourceFromLocations(office, time.UTC)
	if source == temporal.ZoneSourceFromLocations(office, time.UTC) {
		t.Fatal("two sources built from the same locations compare equal")
	}
	if got := drawnZones(t, temporal.ZonedDatetimes().Source(source)); !slices.Equal(got, []string{"Office", "UTC"}) {
		t.Fatalf("drew the zones %v, want Office and UTC", got)
	}
	hegel.Test(t, func(ht *hegel.T) {
		loc := hegel.Draw(ht, temporal.Zones().Source(source).Names("Office"))
		if loc != office {
			ht.Fatalf("drew %v, want the location the source was given", loc)
		}
	}, hegel.WithTestCases(10))
}

// TestDefaultZoneSource is not parallel: it changes the process-wide default.
func TestDefaultZoneSource(t *testing.T) {
	host := temporal.HostZoneSource()
	if temporal.DefaultZoneSource() != host {
		t.Fatal("the default zone source is not the host source")
	}
	only := time.FixedZone("Test/Only", 3600)
	custom := temporal.ZoneSourceFromLocations(only)
	builtBefore := temporal.ZonedDatetimes()

	if previous := temporal.SetDefaultZoneSource(custom); previous != host {
		t.Fatal("SetDefaultZoneSource did not return the host source it replaced")
	}
	t.Cleanup(func() { temporal.SetDefaultZoneSource(host) })
	if temporal.DefaultZoneSource() != custom {
		t.Fatal("DefaultZoneSource does not return the source that was set")
	}
	builtDuring := []hegel.Generator[time.Time]{
		temporal.ZonedDatetimes(),
		temporal.ZonedDatetimes().Zones(temporal.Zones()),
	}
	if previous := temporal.SetDefaultZoneSource(host); previous != custom {
		t.Fatal("SetDefaultZoneSource did not return the custom source it replaced")
	}

	for i, g := range builtDuring {
		if got := drawnZones(t, g); !slices.Equal(got, []string{"Test/Only"}) {
			t.Errorf("generator %d built under the custom default drew the zones %v", i, got)
		}
	}
	if got := drawnZones(t, builtBefore); len(got) < 20 || slices.Contains(got, "Test/Only") {
		t.Errorf("a generator built under the host default drew the zones %v", got)
	}
}
