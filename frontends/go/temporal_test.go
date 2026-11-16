package hegel

import (
	"encoding/binary"
	"errors"
	"math"
	"reflect"
	"regexp"
	"slices"
	"testing"
	"time"
	_ "time/tzdata"

	"hegel.dev/go/hegel/internal/libhegel"
	"hegel.dev/go/hegel/internal/temporalbridge"
)

// TestTemporalBridge checks what package temporal relies on: that package
// hegel registers every constructor, under the type arguments temporal
// asserts.
func TestTemporalBridge(t *testing.T) {
	t.Parallel()
	registered, ok := temporalbridge.Registered.(temporalbridge.Constructors[
		DateGenerator, DatetimeGenerator, DurationGenerator, OffsetGenerator,
		ZoneGenerator, ZonedDatetimeGenerator, ZoneSource,
	])
	if !ok {
		t.Fatalf("temporalbridge.Registered is a %T", temporalbridge.Registered)
	}
	v := reflect.ValueOf(registered)
	for i := range v.NumField() {
		if v.Field(i).IsNil() {
			t.Errorf("no %s constructor is registered", v.Type().Field(i).Name)
		}
	}
}

// tzifType is one local time type of a hand-built TZif file.
type tzifType struct {
	offset int32
	isDST  bool
	name   string
}

// tzifLocation builds a location from a TZif file with the given local time
// types, no recorded transitions and the POSIX TZ rule, under a name no
// database has. Go then computes every lookup from the rule, the path it
// takes for a real zone after its last recorded transition.
func tzifLocation(t testing.TB, name string, types []tzifType, rule string) *time.Location {
	t.Helper()
	header := func(counts [6]uint32) []byte {
		b := append([]byte("TZif2"), make([]byte, 15)...)
		for _, n := range counts {
			b = binary.BigEndian.AppendUint32(b, n)
		}
		return b
	}
	var body, abbreviations []byte
	for _, typ := range types {
		body = binary.BigEndian.AppendUint32(body, uint32(typ.offset))
		dst := byte(0)
		if typ.isDST {
			dst = 1
		}
		body = append(body, dst, byte(len(abbreviations)))
		abbreviations = append(append(abbreviations, typ.name...), 0)
	}
	data := header([6]uint32{})
	data = append(data, header([6]uint32{0, 0, 0, 0, uint32(len(types)), uint32(len(abbreviations))})...)
	data = append(data, body...)
	data = append(data, abbreviations...)
	data = append(data, "\n"+rule+"\n"...)
	loc, err := time.LoadLocationFromTZData(name, data)
	if err != nil {
		t.Fatal(err)
	}
	return loc
}

// testEastern is a zone with New York's current rule under a name only a
// custom zone source knows.
func testEastern(t testing.TB) *time.Location {
	return tzifLocation(t, "Test/Eastern", []tzifType{{-18000, false, "EST"}, {-14400, true, "EDT"}}, "EST5EDT,M3.2.0,M11.1.0")
}

func mustLoad(t testing.TB, name string) *time.Location {
	t.Helper()
	loc, err := time.LoadLocation(name)
	if err != nil {
		t.Fatal(err)
	}
	return loc
}

func wall(year int, month time.Month, day, hour, minute int) time.Time {
	return time.Date(year, month, day, hour, minute, 0, 0, time.UTC)
}

// wallOf returns what t's own clock shows, as a UTC time that can be compared
// with a wall-clock bound.
func wallOf(t time.Time) time.Time {
	return time.Date(t.Year(), t.Month(), t.Day(), t.Hour(), t.Minute(), t.Second(), t.Nanosecond(), time.UTC)
}

func TestDatesWeekdays(t *testing.T) {
	t.Parallel()
	for day := time.Sunday; day <= time.Saturday; day++ {
		Test(t, func(ht *T) {
			if got := Draw(ht, newDates().Weekdays(day)); got.Weekday() != day {
				ht.Fatalf("drew %v, a %v, want a %v", got, got.Weekday(), day)
			}
		}, WithTestCases(15))
	}
	seen := map[time.Weekday]bool{}
	Test(t, func(ht *T) {
		seen[Draw(ht, newDates().Weekdays(time.Saturday, time.Sunday)).Weekday()] = true
	}, WithTestCases(100))
	if len(seen) != 2 || !seen[time.Saturday] || !seen[time.Sunday] {
		t.Fatalf("weekend dates fell on %v", seen)
	}
}

func TestDatesMonths(t *testing.T) {
	t.Parallel()
	for month := time.January; month <= time.December; month++ {
		Test(t, func(ht *T) {
			if got := Draw(ht, newDates().Months(month)); got.Month() != month {
				ht.Fatalf("drew %v, want a date in %v", got, month)
			}
		}, WithTestCases(10))
	}
	lo, hi := wall(2020, time.January, 1, 0, 0), wall(2030, time.December, 31, 0, 0)
	Test(t, func(ht *T) {
		got := Draw(ht, newDates().Min(lo).Max(hi).Months(time.February, time.July).Weekdays(time.Monday))
		if (got.Month() != time.February && got.Month() != time.July) || got.Weekday() != time.Monday || got.Before(lo) || got.After(hi) {
			ht.Fatalf("drew %v", got)
		}
	}, WithTestCases(50))
}

func TestDatesMaskErrors(t *testing.T) {
	t.Parallel()
	for want, g := range map[string]DateGenerator{
		"temporal.Dates: 7 is not a weekday":  newDates().Weekdays(time.Monday, time.Weekday(7)),
		"temporal.Dates: -1 is not a weekday": newDates().Weekdays(time.Weekday(-1)),
		"temporal.Dates: 0 is not a month":    newDates().Months(time.Month(0)),
		"temporal.Dates: 13 is not a month":   newDates().Months(time.December, time.Month(13)),
		"hegel_date_options_set_weekdays":     newDates().Weekdays(),
		"hegel_date_options_set_months":       newDates().Months(),
	} {
		_, err := g.draw(newRealTestCase(t))
		assertErrorContains(t, want, err)
	}
}

func TestDatetimesPrecision(t *testing.T) {
	t.Parallel()
	Test(t, func(ht *T) {
		got := Draw(ht, newDatetimes().Precision(libhegel.PRECISION_HOURS))
		if got.Minute() != 0 || got.Second() != 0 || got.Nanosecond() != 0 {
			ht.Fatalf("drew %v, want a whole hour", got)
		}
	}, WithTestCases(30))
	Test(t, func(ht *T) {
		if got := Draw(ht, newDatetimes().Precision(libhegel.PRECISION_MILLISECONDS)); got.Nanosecond()%1_000_000 != 0 {
			ht.Fatalf("drew %v, want whole milliseconds", got)
		}
	}, WithTestCases(30))
	_, err := newDatetimes().Precision(Precision(99)).draw(newRealTestCase(t))
	assertErrorContains(t, "hegel_generate_datetime", err)
}

func TestDurationsBounds(t *testing.T) {
	t.Parallel()
	Test(t, func(ht *T) {
		Draw(ht, newDurations())
	}, WithTestCases(50))
	lo, hi := -90*time.Minute, 36*time.Hour
	Test(t, func(ht *T) {
		if got := Draw(ht, newDurations().Min(lo).Max(hi)); got < lo || got > hi {
			ht.Fatalf("drew %v outside [%v, %v]", got, lo, hi)
		}
	}, WithTestCases(100))
	Test(t, func(ht *T) {
		if got := Draw(ht, newDurations().Min(0)); got < 0 {
			ht.Fatalf("drew %v below a minimum of zero", got)
		}
		if got := Draw(ht, newDurations().Max(0)); got > 0 {
			ht.Fatalf("drew %v above a maximum of zero", got)
		}
	}, WithTestCases(100))
	Test(t, func(ht *T) {
		if got := Draw(ht, newDurations().Precision(libhegel.PRECISION_SECONDS)); got%time.Second != 0 {
			ht.Fatalf("drew %v, want whole seconds", got)
		}
	}, WithTestCases(50))
}

func TestDurationsExactValues(t *testing.T) {
	t.Parallel()
	tc := newRealTestCase(t)
	for _, want := range []time.Duration{
		0, 1, -1, -1500 * time.Millisecond, 90*time.Minute + 7, time.Duration(math.MinInt64), time.Duration(math.MaxInt64),
	} {
		got, err := newDurations().Min(want).Max(want).draw(tc)
		if err != nil || got != want {
			t.Fatalf("a duration pinned to %d = %d, %v", want, got, err)
		}
	}
	_, err := newDurations().Min(time.Second).Max(0).draw(tc)
	assertErrorContains(t, "hegel_generate_duration", err)
}

var offsetName = regexp.MustCompile(`^[+-]\d\d:\d\d(:\d\d)?$`)

func TestOffsets(t *testing.T) {
	t.Parallel()
	Test(t, func(ht *T) {
		loc := Draw(ht, newOffsets())
		name, offset := time.Unix(0, 0).In(loc).Zone()
		if name != loc.String() || !offsetName.MatchString(name) || offset < -93599 || offset > 93599 {
			ht.Fatalf("drew %q at %d seconds", name, offset)
		}
	}, WithTestCases(100))
	lo, hi := -12*time.Hour, 14*time.Hour
	Test(t, func(ht *T) {
		loc := Draw(ht, newOffsets().Min(lo).Max(hi).Granularity(libhegel.OFFSET_GRANULARITY_QUARTER_HOURS))
		_, offset := time.Unix(0, 0).In(loc).Zone()
		if got := time.Duration(offset) * time.Second; got < lo || got > hi || got%(15*time.Minute) != 0 {
			ht.Fatalf("drew an offset of %v", got)
		}
	}, WithTestCases(100))
	Test(t, func(ht *T) {
		_, offset := time.Unix(0, 0).In(Draw(ht, newOffsets().Granularity(libhegel.OFFSET_GRANULARITY_HOURS))).Zone()
		if offset%3600 != 0 {
			ht.Fatalf("drew an offset of %d seconds, want whole hours", offset)
		}
	}, WithTestCases(50))
}

func TestOffsetsExactValues(t *testing.T) {
	t.Parallel()
	tc := newRealTestCase(t)
	for offset, want := range map[time.Duration]string{
		5*time.Hour + 30*time.Minute:       "+05:30",
		-8 * time.Hour:                     "-08:00",
		0:                                  "+00:00",
		-(44*time.Minute + 30*time.Second): "-00:44:30",
		93599 * time.Second:                "+25:59:59",
	} {
		loc, err := newOffsets().Min(offset).Max(offset).draw(tc)
		if err != nil {
			t.Fatal(err)
		}
		name, seconds := time.Unix(0, 0).In(loc).Zone()
		if name != want || loc.String() != want || time.Duration(seconds)*time.Second != offset {
			t.Fatalf("an offset pinned to %v is %q at %d seconds, want %q", offset, name, seconds, want)
		}
	}
}

func TestOffsetsDefaultToTheEngineLimits(t *testing.T) {
	t.Parallel()
	tc := newRealTestCase(t)
	for want, g := range map[string]OffsetGenerator{
		"+25:59:59": newOffsets().Min(93599 * time.Second),
		"-25:59:59": newOffsets().Max(-93599 * time.Second),
	} {
		loc, err := g.draw(tc)
		if err != nil || loc.String() != want {
			t.Fatalf("an offset with one bound at the limit is %v, %v; want %s", loc, err, want)
		}
	}
}

func TestOffsetsErrors(t *testing.T) {
	t.Parallel()
	// 2^32 seconds more than an hour: truncated to an int32 it would be an
	// hour, a bound the engine accepts.
	wrapsToAnHour := (1<<32 + 3600) * time.Second
	for want, g := range map[string]OffsetGenerator{
		"temporal.Offsets: Min must be a whole number of seconds, got 1.5s": newOffsets().Min(1500 * time.Millisecond),
		"temporal.Offsets: Max must be a whole number of seconds, got -1ns": newOffsets().Max(-1),
		"hegel_generate_offset: E_INVALID_ARG":                              newOffsets().Min(time.Hour).Max(0),
		"max_seconds":                                                       newOffsets().Max(wrapsToAnHour),
		"min_seconds":                                                       newOffsets().Min(-wrapsToAnHour),
	} {
		_, err := g.draw(newRealTestCase(t))
		assertErrorContains(t, want, err)
	}
}

// classifiedSource holds four locations whose names the IANA catalogue
// classifies differently: a zone, UTC, a link and a legacy name.
func classifiedSource() ZoneSource {
	return zoneSourceFromLocations(
		time.UTC,
		time.FixedZone("Europe/Paris", 3600),
		time.FixedZone("Asia/Calcutta", 19800),
		time.FixedZone("EST5EDT", -18000),
	)
}

func drawnNames(t *testing.T, g Generator[*time.Location]) []string {
	t.Helper()
	seen := map[string]bool{}
	Test(t, func(ht *T) {
		seen[Draw(ht, g).String()] = true
	}, WithTestCases(200))
	var names []string
	for name := range seen {
		names = append(names, name)
	}
	slices.Sort(names)
	return names
}

func TestZonesLinkAndLegacyFlags(t *testing.T) {
	t.Parallel()
	source := classifiedSource()
	for _, tt := range []struct {
		label string
		g     ZoneGenerator
		want  []string
	}{
		{"default", newZones().Source(source), []string{"Asia/Calcutta", "Europe/Paris", "UTC"}},
		{"without links", newZones().Source(source).AllowLinks(false), []string{"Europe/Paris", "UTC"}},
		{"with links again", newZones().Source(source).AllowLinks(false).AllowLinks(true), []string{"Asia/Calcutta", "Europe/Paris", "UTC"}},
		{"with legacy", newZones().Source(source).AllowLegacy(true), []string{"Asia/Calcutta", "EST5EDT", "Europe/Paris", "UTC"}},
		{"legacy without links", newZones().Source(source).AllowLegacy(true).AllowLinks(false), []string{"EST5EDT", "Europe/Paris", "UTC"}},
		{"names", newZones().Source(source).Names("EST5EDT", "UTC"), []string{"EST5EDT", "UTC"}},
		{"names ignore the flags", newZones().Source(source).AllowLinks(false).Names("Asia/Calcutta"), []string{"Asia/Calcutta"}},
	} {
		if got := drawnNames(t, tt.g); !slices.Equal(got, tt.want) {
			t.Errorf("%s: drew %v, want %v", tt.label, got, tt.want)
		}
	}
}

func TestZonesReturnsTheSourcesLocations(t *testing.T) {
	t.Parallel()
	paris := time.FixedZone("Europe/Paris", 3600)
	source := zoneSourceFromLocations(paris)
	Test(t, func(ht *T) {
		if got := Draw(ht, newZones().Source(source)); got != paris {
			ht.Fatalf("drew %v, want the location the source was given", got)
		}
	}, WithTestCases(5))
}

func TestZonesFromTheHostAndBundledSources(t *testing.T) {
	t.Parallel()
	for label, g := range map[string]ZoneGenerator{
		"default": newZones(),
		"host":    newZones().Source(hostZoneSource()),
		"bundled": newZones().Source(bundledZoneSource()),
	} {
		names := drawnNames(t, g)
		if len(names) < 20 {
			t.Errorf("%s: 200 draws gave only the zones %v", label, names)
		}
		for _, name := range names {
			if _, err := time.LoadLocation(name); err != nil {
				t.Errorf("%s: drew %q, which Go cannot load: %v", label, name, err)
			}
		}
	}
	paris, err := newZones().Names("Europe/Paris").draw(newRealTestCase(t))
	if err != nil || paris.String() != "Europe/Paris" {
		t.Fatalf("Zones().Names(Europe/Paris) = %v, %v", paris, err)
	}
	if _, offset := time.Date(2024, time.July, 1, 0, 0, 0, 0, paris).Zone(); offset != 7200 {
		t.Fatalf("Europe/Paris is %d seconds from UTC in July, want 7200", offset)
	}
	local, err := newZones().Names("Local").draw(newRealTestCase(t))
	if err != nil || local != time.Local {
		t.Fatalf("Zones().Names(Local) = %v, %v", local, err)
	}
}

func TestZonesErrors(t *testing.T) {
	t.Parallel()
	source := classifiedSource()
	for want, g := range map[string]Generator[*time.Location]{
		"Nowhere/Special":               newZones().Source(source).Names("UTC", "Nowhere/Special"),
		"Not/AZone":                     newZones().Names("Not/AZone"),
		"hegel_timezone_spec_set_names": newZones().Names(),
		"no zone source: the zero ZoneSource is not one": newZones().Source(ZoneSource{}),
		"a zero-value zone generator has none":           ZoneGenerator{}.Names("UTC"),
		"hegel_generate_timezone":                        newZones().Source(zoneSourceFromLocations()),
	} {
		_, err := g.draw(newRealTestCase(t))
		assertErrorContains(t, want, err)
	}
}

func TestZonedDatetimesDefaults(t *testing.T) {
	t.Parallel()
	zones := map[string]bool{}
	Test(t, func(ht *T) {
		got := Draw(ht, newZonedDatetimes())
		if got.Year() < 1 || got.Year() > 9999 || got.Location() == nil {
			ht.Fatalf("drew %v", got)
		}
		zones[got.Location().String()] = true
	}, WithTestCases(200))
	if len(zones) < 20 {
		t.Fatalf("200 draws used only the zones %v", zones)
	}
}

func TestZonedDatetimesWallClockBounds(t *testing.T) {
	t.Parallel()
	lo, hi := wall(2021, time.March, 13, 22, 0), wall(2021, time.March, 15, 2, 0)
	Test(t, func(ht *T) {
		got := wallOf(Draw(ht, newZonedDatetimes().MinWall(lo).MaxWall(hi)))
		if got.Before(lo) || got.After(hi) {
			ht.Fatalf("drew the wall-clock time %v outside [%v, %v]", got, lo, hi)
		}
	}, WithTestCases(200))
	Test(t, func(ht *T) {
		if got := Draw(ht, newZonedDatetimes().MinWall(wall(9999, time.December, 31, 0, 0))); got.Year() != 9999 || got.Month() != time.December || got.Day() != 31 {
			ht.Fatalf("drew %v with only a minimum of the last day", got)
		}
		if got := Draw(ht, newZonedDatetimes().MaxWall(wall(1, time.January, 1, 23, 59))); got.Year() != 1 || got.YearDay() != 1 {
			ht.Fatalf("drew %v with only a maximum of the first day", got)
		}
	}, WithTestCases(50))
	ignored := time.Date(2024, time.May, 5, 12, 0, 0, 0, time.FixedZone("ignored", -7*3600))
	got, err := newZonedDatetimes().Zones(Just(mustLoad(t, "Asia/Tokyo"))).MinWall(ignored).MaxWall(ignored).draw(newRealTestCase(t))
	if err != nil || !got.Equal(time.Date(2024, time.May, 5, 3, 0, 0, 0, time.UTC)) {
		t.Fatalf("noon on the Tokyo wall clock = %v, %v", got, err)
	}
}

func TestZonedDatetimesInstantBounds(t *testing.T) {
	t.Parallel()
	lo := time.Date(2021, time.November, 6, 0, 0, 0, 0, time.UTC)
	hi := time.Date(2021, time.November, 8, 0, 0, 0, 0, time.FixedZone("west", -5*3600))
	Test(t, func(ht *T) {
		if got := Draw(ht, newZonedDatetimes().Min(lo).Max(hi)); got.Before(lo) || got.After(hi) {
			ht.Fatalf("drew the instant %v outside [%v, %v]", got, lo, hi)
		}
	}, WithTestCases(200))
	Test(t, func(ht *T) {
		late := time.Date(9999, time.December, 31, 0, 0, 0, 0, time.UTC)
		if got := Draw(ht, newZonedDatetimes().Min(late)); got.Before(late) {
			ht.Fatalf("drew %v before the minimum %v", got, late)
		}
		early := time.Date(1, time.January, 2, 0, 0, 0, 0, time.UTC)
		if got := Draw(ht, newZonedDatetimes().Max(early)); got.After(early) {
			ht.Fatalf("drew %v after the maximum %v", got, early)
		}
	}, WithTestCases(50))
	at := time.Date(2024, time.May, 5, 12, 0, 0, 123456789, time.UTC)
	got, err := newZonedDatetimes().Zones(Just(mustLoad(t, "Asia/Tokyo"))).Min(at).Max(at).draw(newRealTestCase(t))
	if err != nil || !got.Equal(at) || got.Hour() != 21 || got.Location().String() != "Asia/Tokyo" {
		t.Fatalf("an instant pinned to %v = %v, %v", at, got, err)
	}
}

func TestZonedDatetimesInstantBoundsDefaultToTheYears1To9999(t *testing.T) {
	t.Parallel()
	first := time.Date(1, time.January, 1, 0, 0, 0, 0, time.UTC)
	last := time.Date(9999, time.December, 31, 23, 59, 59, 999999999, time.UTC)
	Test(t, func(ht *T) {
		if got := Draw(ht, newZonedDatetimes().Max(first)); !got.Equal(first) {
			ht.Fatalf("drew %v with only a maximum of %v", got, first)
		}
		if got := Draw(ht, newZonedDatetimes().Min(last)); !got.Equal(last) {
			ht.Fatalf("drew %v with only a minimum of %v", got, last)
		}
	}, WithTestCases(100))
}

func TestZonedDatetimesMixedBounds(t *testing.T) {
	t.Parallel()
	at := wall(2024, time.May, 5, 12, 0)
	for label, g := range map[string]ZonedDatetimeGenerator{
		"Min and MinWall": newZonedDatetimes().Min(at).MinWall(at),
		"Min and MaxWall": newZonedDatetimes().Min(at).MaxWall(at),
		"Max and MinWall": newZonedDatetimes().MinWall(at).Max(at),
		"Max and MaxWall": newZonedDatetimes().MaxWall(at).Max(at),
	} {
		_, err := g.draw(newRealTestCase(t))
		if err == nil || err.Error() != "temporal.ZonedDatetimes: Min and Max bound the instant and MinWall and MaxWall bound the wall clock; set one kind of bound, not both" {
			t.Errorf("%s: %v", label, err)
		}
	}
}

func TestZonedDatetimesBoundErrors(t *testing.T) {
	t.Parallel()
	at := wall(2024, time.May, 5, 12, 0)
	for want, g := range map[string]ZonedDatetimeGenerator{
		"hegel_zoned_spec_set_civil_bounds":   newZonedDatetimes().MinWall(at).MaxWall(at.Add(-time.Hour)),
		"hegel_zoned_spec_set_instant_bounds": newZonedDatetimes().Min(at).Max(at.Add(-time.Hour)),
		"date year must be between":           newZonedDatetimes().MaxWall(wall(1_000_000, time.January, 1, 0, 0)),
		"hegel_zoned_spec_set_precision":      newZonedDatetimes().Precision(Precision(99)),
		"hegel_zoned_spec_set_ambiguous":      newZonedDatetimes().Ambiguous(Ambiguous(99)),
	} {
		_, err := g.draw(newRealTestCase(t))
		assertErrorContains(t, want, err)
	}
}

func TestZonedDatetimesPrecision(t *testing.T) {
	t.Parallel()
	Test(t, func(ht *T) {
		if got := Draw(ht, newZonedDatetimes().Precision(libhegel.PRECISION_MINUTES)); got.Second() != 0 || got.Nanosecond() != 0 {
			ht.Fatalf("drew %v, want a whole minute on the wall clock", got)
		}
	}, WithTestCases(100))
}

// TestZonedDatetimesReachBothSidesOfAFold draws inside the hour New York
// repeated on 2021-11-07, first on daylight time and then on standard time.
func TestZonedDatetimesReachBothSidesOfAFold(t *testing.T) {
	t.Parallel()
	const daylight, standard = -4 * 3600, -5 * 3600
	for _, loc := range []*time.Location{mustLoad(t, "America/New_York"), testEastern(t)} {
		source := zoneSourceFromLocations(loc)
		inTheFold := newZonedDatetimes().Zones(Just(loc)).Source(source).
			MinWall(wall(2021, time.November, 7, 1, 0)).MaxWall(wall(2021, time.November, 7, 1, 59))
		for _, tt := range []struct {
			policy Ambiguous
			want   []int
		}{
			{libhegel.AMBIGUOUS_BOTH, []int{standard, daylight}},
			{libhegel.AMBIGUOUS_EARLIER, []int{daylight}},
			{libhegel.AMBIGUOUS_LATER, []int{standard}},
		} {
			seen := map[int]bool{}
			Test(t, func(ht *T) {
				got := Draw(ht, inTheFold.Ambiguous(tt.policy))
				if got.Hour() != 1 || got.Day() != 7 {
					ht.Fatalf("drew %v, outside the repeated hour", got)
				}
				_, offset := got.Zone()
				seen[offset] = true
			}, WithTestCases(100))
			var offsets []int
			for offset := range seen {
				offsets = append(offsets, offset)
			}
			slices.Sort(offsets)
			if !slices.Equal(offsets, tt.want) {
				t.Errorf("%v under %v: offsets %v, want %v", loc, tt.policy, offsets, tt.want)
			}
		}
	}
}

// TestZonedDatetimesNeverInAGap draws around the hour New York skipped on
// 2021-03-14: every value is a time the wall clock showed, on either side.
func TestZonedDatetimesNeverInAGap(t *testing.T) {
	t.Parallel()
	lo, hi := wall(2021, time.March, 14, 1, 30), wall(2021, time.March, 14, 3, 30)
	before, after := 0, 0
	Test(t, func(ht *T) {
		got := Draw(ht, newZonedDatetimes().Zones(Just(mustLoad(t, "America/New_York"))).MinWall(lo).MaxWall(hi))
		shown := wallOf(got)
		if shown.Before(lo) || shown.After(hi) || got.Hour() == 2 {
			ht.Fatalf("drew %v", got)
		}
		if got.Hour() < 2 {
			before++
		} else {
			after++
		}
	}, WithTestCases(100))
	if before == 0 || after == 0 {
		t.Fatalf("%d draws before the gap and %d after it, want some of each", before, after)
	}
}

func TestZonedDatetimesZoneSourceRules(t *testing.T) {
	t.Parallel()
	eastern, fixed := testEastern(t), time.FixedZone("Test/Fixed", 9*3600)
	source := zoneSourceFromLocations(eastern, fixed)
	inSource := func(label string, g ZonedDatetimeGenerator, want ...string) {
		t.Helper()
		seen := map[string]bool{}
		Test(t, func(ht *T) {
			got := Draw(ht, g)
			if got.Location() != eastern && got.Location() != fixed {
				ht.Fatalf("%s: drew %v, in a location the source does not hold", label, got)
			}
			seen[got.Location().String()] = true
		}, WithTestCases(100))
		for _, name := range want {
			if !seen[name] {
				t.Errorf("%s: never drew a time in %s", label, name)
			}
		}
	}
	both := []string{"Test/Eastern", "Test/Fixed"}
	always := func(*time.Location) bool { return true }
	inSource("Source alone", newZonedDatetimes().Source(source), both...)
	inSource("Zones with a source", newZonedDatetimes().Zones(newZones().Source(source)), both...)
	inSource("Zones with names", newZonedDatetimes().Zones(newZones().Source(source).Names("Test/Fixed")), "Test/Fixed")
	inSource("wrapped Zones and Source", newZonedDatetimes().Zones(Filter(newZones().Source(source), always)).Source(source), both...)
	inSource("another generator and Source", newZonedDatetimes().Zones(SampledFrom([]*time.Location{eastern, fixed})).Source(source), both...)

	for want, g := range map[string]ZonedDatetimeGenerator{
		"give the source to it, as temporal.Zones().Source(...)": newZonedDatetimes().Zones(newZones().Source(source)).Source(source),
		`does not know the zone "Test/Eastern"`:                  newZonedDatetimes().Zones(Filter(newZones().Source(source), always)),
		`does not know the zone "Test/Fixed"`:                    newZonedDatetimes().Zones(Just(fixed)).Source(bundledZoneSource()),
		`does not know the zone "+05:00"`:                        newZonedDatetimes().Zones(newOffsets().Min(5 * time.Hour).Max(5 * time.Hour)),
		"pass a source that has it to Source":                    newZonedDatetimes().Zones(Just(fixed)),
		"the zones generator drew a nil *time.Location":          newZonedDatetimes().Zones(Just[*time.Location](nil)),
		"no zone source: the zero ZoneSource is not one":         newZonedDatetimes().Source(ZoneSource{}),
		"a zero-value zone generator has none":                   newZonedDatetimes().Zones(Just(fixed)).Source(ZoneSource{}),
		"hegel: no zone source":                                  ZonedDatetimeGenerator{},
		"hegel_generate_zoned_datetime: E_INVALID_ARG":           newZonedDatetimes().Source(zoneSourceFromLocations()),
		"hegel_timezone_spec_set_names":                          newZonedDatetimes().Zones(newZones().Source(source).Names()),
		"SampledFrom requires at least one element (via Map)":    newZonedDatetimes().Zones(failingZones{}),
	} {
		_, err := g.draw(newRealTestCase(t))
		assertErrorContains(t, want, err)
	}
}

// failingZones is a zones generator whose draw fails.
type failingZones struct{}

func (failingZones) draw(TestCase) (*time.Location, error) {
	return nil, errors.New("SampledFrom requires at least one element (via Map)")
}

// TestZonedDatetimesRejectWhenGoDisagrees gives the generator a location that
// shares its name with a zone of the source but not its offset: the engine
// draws against the source's zone, Go reads the instant in the other, and
// the value is rejected rather than returned at a wall-clock time the bounds
// were not applied to.
func TestZonedDatetimesRejectWhenGoDisagrees(t *testing.T) {
	t.Parallel()
	source := zoneSourceFromLocations(time.FixedZone("Test/Fixed", 3600))
	impostor := time.FixedZone("Test/Fixed", 7200)
	_, err := newZonedDatetimes().Zones(Just(impostor)).Source(source).draw(newRealTestCase(t))
	if !errors.Is(err, libhegel.E_ASSUME) {
		t.Fatalf("err = %v, want a rejected test case", err)
	}
}

// TestZonedDatetimesAgreeWithGo pins the instant in zones with every kind of
// history, inside and beyond the years the host source probes: the engine
// has to read each at the offset Go does, or the draw would be rejected.
func TestZonedDatetimesAgreeWithGo(t *testing.T) {
	t.Parallel()
	tc := newRealTestCase(t)
	for _, name := range []string{
		"UTC", "America/New_York", "Australia/Lord_Howe", "Africa/Casablanca", "Europe/Dublin",
		"Pacific/Apia", "America/Sao_Paulo", "Asia/Kolkata", "Africa/Monrovia", "Antarctica/Troll",
	} {
		loc := mustLoad(t, name)
		for _, at := range []time.Time{
			time.Date(1, time.January, 2, 0, 0, 0, 0, time.UTC),
			time.Date(1850, time.June, 1, 0, 0, 0, 0, time.UTC),
			time.Date(1945, time.August, 15, 12, 0, 0, 0, time.UTC),
			time.Date(1972, time.January, 7, 0, 30, 0, 0, time.UTC),
			time.Date(2011, time.December, 30, 12, 0, 0, 0, time.UTC),
			time.Date(2024, time.January, 15, 0, 0, 0, 0, time.UTC),
			time.Date(2024, time.July, 15, 0, 0, 0, 0, time.UTC),
			time.Date(2040, time.December, 31, 12, 0, 0, 0, time.UTC),
			time.Date(2041, time.July, 1, 12, 0, 0, 0, time.UTC),
			time.Date(2100, time.July, 1, 0, 0, 0, 0, time.UTC),
			time.Date(2101, time.January, 1, 0, 0, 0, 0, time.UTC),
			time.Date(2500, time.July, 1, 0, 0, 0, 0, time.UTC),
			time.Date(9999, time.December, 30, 0, 0, 0, 0, time.UTC),
		} {
			got, err := newZonedDatetimes().Zones(Just(loc)).Min(at).Max(at).draw(tc)
			if err != nil || !got.Equal(at) || got.Location() != loc {
				t.Errorf("%s at %v: %v, %v", name, at, got, err)
			}
		}
	}
}

func TestZonedDatetimesFromTheBundledSource(t *testing.T) {
	t.Parallel()
	bundled := bundledZoneSource()
	for label, g := range map[string]ZonedDatetimeGenerator{
		"Source":            newZonedDatetimes().Source(bundled),
		"Zones":             newZonedDatetimes().Zones(newZones().Source(bundled)),
		"another generator": newZonedDatetimes().Zones(Just(mustLoad(t, "Europe/London"))).Source(bundled),
	} {
		lo, hi := wall(1990, time.January, 1, 0, 0), wall(2030, time.January, 1, 0, 0)
		Test(t, func(ht *T) {
			got := wallOf(Draw(ht, g.MinWall(lo).MaxWall(hi)))
			if got.Before(lo) || got.After(hi) {
				ht.Fatalf("%s: drew %v", label, got)
			}
		}, WithTestCases(50))
	}
}
