package hegel

import (
	"errors"
	"fmt"
	"runtime"
	"slices"
	"strings"
	"testing"
	"time"
	"weak"

	"hegel.dev/go/hegel/internal/libhegel"
)

// TestZoneCatalogueMatchesTheEngine checks zone_catalogue.go against the
// engine the tests run on: every name has the rank that is its index, and a
// name outside the catalogue has the rank after all of them. A failure means
// the engine's database changed: run `go generate .`.
func TestZoneCatalogueMatchesTheEngine(t *testing.T) {
	t.Parallel()
	ctx := libhegel.NewContext()
	bundled, err := ctx.ZoneSourceBundled()
	if err != nil {
		t.Fatal(err)
	}
	for rank, name := range zoneCatalogue {
		info, err := ctx.TimezoneInfoIn(bundled, name)
		if err != nil || !info.Known || int(info.Rank) != rank {
			t.Fatalf("%s: info %+v, %v; want a known name of rank %d", name, info, err, rank)
		}
	}
	source, err := ctx.ZoneSourceNew()
	if err != nil {
		t.Fatal(err)
	}
	if err := source.Declare(ctx, []string{"Test/Uncatalogued"}); err != nil {
		t.Fatal(err)
	}
	info, err := ctx.TimezoneInfoIn(source, "Test/Uncatalogued")
	if err != nil || int(info.Rank) != len(zoneCatalogue) {
		t.Fatalf("an uncatalogued name has rank %d, %v; the catalogue has %d names", info.Rank, err, len(zoneCatalogue))
	}
}

// TestDefaultZoneSource is not parallel: it changes the process-wide default.
func TestDefaultZoneSource(t *testing.T) {
	host, bundled := HostZoneSource(), BundledZoneSource()
	if DefaultZoneSource() != host || host != HostZoneSource() || host == bundled || bundled != BundledZoneSource() {
		t.Fatal("the default zone source is not the host source, or the built-in sources are not stable")
	}
	only := time.FixedZone("Test/Only", 3600)
	custom := ZoneSourceFromLocations(only)
	if custom == ZoneSourceFromLocations(only) {
		t.Fatal("two sources built from the same locations compare equal")
	}

	builtBefore := []Generator[*time.Location]{Zones(), Map(ZonedDatetimes(), time.Time.Location)}
	if previous := SetDefaultZoneSource(custom); previous != host {
		t.Fatal("SetDefaultZoneSource did not return the host source it replaced")
	}
	if DefaultZoneSource() != custom {
		t.Fatal("DefaultZoneSource does not return the source that was set")
	}
	builtDuring := []Generator[*time.Location]{
		Zones(),
		Map(ZonedDatetimes(), time.Time.Location),
		Map(ZonedDatetimes().Zones(Just(only)), time.Time.Location),
		Default[*time.Location](),
	}
	for i, g := range builtDuring {
		if got := drawnNames(t, g); !slices.Equal(got, []string{"Test/Only"}) {
			t.Errorf("generator %d built under the custom default drew %v", i, got)
		}
	}
	if previous := SetDefaultZoneSource(host); previous != custom {
		t.Fatal("SetDefaultZoneSource did not return the custom source it replaced")
	}

	for i, g := range builtDuring[:3] {
		if got := drawnNames(t, g); !slices.Equal(got, []string{"Test/Only"}) {
			t.Errorf("generator %d built under the custom default drew %v once the default was restored", i, got)
		}
	}
	for i, g := range append(builtBefore, Zones(), Default[*time.Location]()) {
		if got := drawnNames(t, g); len(got) < 20 || slices.Contains(got, "Test/Only") {
			t.Errorf("generator %d built under the host default drew %v", i, got)
		}
	}

	assertPanicsWithMessage(t, "SetDefaultZoneSource: the zero ZoneSource is not a zone source", func() {
		SetDefaultZoneSource(ZoneSource{})
	})
	if DefaultZoneSource() != host {
		t.Fatal("a rejected SetDefaultZoneSource changed the default")
	}
}

func TestZoneSourceFromLocationsRejectsBadLocations(t *testing.T) {
	t.Parallel()
	assertPanicsWithMessage(t, "ZoneSourceFromLocations: nil location", func() {
		ZoneSourceFromLocations(time.UTC, nil)
	})
	assertPanicsWithMessage(t, `ZoneSourceFromLocations: two locations are named "Test/Twice"`, func() {
		ZoneSourceFromLocations(time.FixedZone("Test/Twice", 0), time.FixedZone("Test/Twice", 3600))
	})
}

// TestZoneSourceFromLocationsWithAFixedZone draws times in a source of one
// zone with daylight saving and one fixed zone, and checks each against what
// the location itself says.
func TestZoneSourceFromLocationsWithAFixedZone(t *testing.T) {
	t.Parallel()
	eastern, fixed := testEastern(t), time.FixedZone("Test/Fixed", -(3*3600+30*60))
	source := ZoneSourceFromLocations(eastern, fixed)
	offsets := map[string]map[int]bool{"Test/Eastern": {}, "Test/Fixed": {}}
	Test(t, func(ht *T) {
		got := Draw(ht, ZonedDatetimes().Source(source).MinWall(wall(2000, time.January, 1, 0, 0)).MaxWall(wall(2030, time.January, 1, 0, 0)))
		name, offset := got.Zone()
		offsets[got.Location().String()][offset] = true
		if got.Location() == fixed && (offset != -12600 || name != "Test/Fixed") {
			ht.Fatalf("drew %v in the fixed zone", got)
		}
	}, WithTestCases(200))
	if got := offsets["Test/Fixed"]; len(got) != 1 {
		t.Errorf("the fixed zone was drawn at the offsets %v", got)
	}
	if got := offsets["Test/Eastern"]; !got[-18000] || !got[-14400] || len(got) != 2 {
		t.Errorf("the zone with daylight saving was drawn at the offsets %v", got)
	}
}

// TestZoneSourceLoaderIsReleased checks that dropping a zone source frees what
// its loader holds. The loader's state is reachable from the handle table
// until the engine calls the release hook, so a location only the source
// refers to is collected exactly when the hook has run.
func TestZoneSourceLoaderIsReleased(t *testing.T) {
	t.Parallel()
	held := func() weak.Pointer[time.Location] {
		loc := time.FixedZone("Test/Released", 3600)
		source := ZoneSourceFromLocations(loc)
		got, err := ZonedDatetimes().Source(source).draw(newRealTestCase(t))
		if err != nil || got.Location() != loc {
			t.Fatalf("drew %v, %v", got, err)
		}
		return weak.Make(loc)
	}()
	for range 100 {
		runtime.GC()
		if held.Value() == nil {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("the location of a dropped zone source was never collected: its loader was not released")
}

// partialZones is a location loader that lacks some zones Go has.
func partialZones(missing ...string) *goZones {
	return newGoZones(func(name string) (*time.Location, error) {
		if slices.Contains(missing, name) {
			return nil, fmt.Errorf("no zone %s here", name)
		}
		return time.LoadLocation(name)
	})
}

// noZones is the location loader of a machine with no time zone database: as
// with time.LoadLocation, only UTC loads.
func noZones() *goZones {
	return newGoZones(func(name string) (*time.Location, error) {
		if name == "UTC" {
			return time.UTC, nil
		}
		return nil, errors.New("unknown time zone " + name)
	})
}

func TestHostZoneSourceLeavesOutWhatGoCannotLoad(t *testing.T) {
	t.Parallel()
	zones := partialZones("Europe/Paris", "Asia/Tokyo")
	if got, want := len(zones.loadable()), len(zoneCatalogue)-2; got != want {
		t.Fatalf("%d loadable names, want %d", got, want)
	}
	host := ZoneSource{zones.hostSource()}
	names := map[string]bool{}
	Test(t, func(ht *T) {
		names[Draw(ht, Zones().Source(host).Names("Europe/Berlin", "Asia/Seoul")).String()] = true
	}, WithTestCases(30))
	if !names["Europe/Berlin"] || !names["Asia/Seoul"] {
		t.Fatalf("drew %v", names)
	}
	_, err := Zones().Source(host).Names("Europe/Paris").draw(newRealTestCase(t))
	assertErrorContains(t, "Europe/Paris", err)
	_, err = ZonedDatetimes().Zones(Just(time.FixedZone("Asia/Tokyo", 9*3600))).Source(host).draw(newRealTestCase(t))
	assertErrorContains(t, `does not know the zone "Asia/Tokyo"`, err)

	bundled := ZoneSource{zones.bundledSource()}
	_, err = Zones().Source(bundled).Names("Europe/Paris").draw(newRealTestCase(t))
	if !errors.Is(err, libhegel.E_ASSUME) {
		t.Fatalf("a bundled zone Go cannot load: %v, want a rejected test case", err)
	}
	_, err = ZonedDatetimes().Zones(Zones().Source(bundled).Names("Asia/Tokyo")).draw(newRealTestCase(t))
	if !errors.Is(err, libhegel.E_ASSUME) {
		t.Fatalf("a time in a bundled zone Go cannot load: %v, want a rejected test case", err)
	}
	berlin, err := Zones().Source(bundled).Names("Europe/Berlin").draw(newRealTestCase(t))
	if err != nil || berlin.String() != "Europe/Berlin" {
		t.Fatalf("a bundled zone Go can load: %v, %v", berlin, err)
	}
}

func TestZoneSourcesWithoutATimeZoneDatabase(t *testing.T) {
	t.Parallel()
	zones := noZones()
	if got := zones.loadable(); !slices.Equal(got, []string{"UTC"}) {
		t.Fatalf("loadable names = %v", got)
	}
	host, bundled := ZoneSource{zones.hostSource()}, ZoneSource{zones.bundledSource()}
	for label, g := range map[string]interface {
		draw(TestCase) (*time.Location, error)
	}{
		"Zones":                            Zones().Source(host),
		"Zones with names":                 Zones().Source(host).Names("UTC"),
		"ZonedDatetimes":                   Map(ZonedDatetimes().Source(host), time.Time.Location),
		"ZonedDatetimes of a location":     Map(ZonedDatetimes().Zones(Just(time.UTC)).Source(host), time.Time.Location),
		"bundled Zones":                    Zones().Source(bundled).Names("Europe/Paris"),
		"bundled ZonedDatetimes of a zone": Map(ZonedDatetimes().Zones(Zones().Source(bundled).Names("Asia/Tokyo")), time.Time.Location),
	} {
		_, err := g.draw(newRealTestCase(t))
		if err == nil || !strings.Contains(err.Error(), `Import _ "time/tzdata"`) {
			t.Errorf("%s: %v, want an error that says to import time/tzdata", label, err)
		}
	}
	utc, err := Zones().Source(bundled).Names("UTC").draw(newRealTestCase(t))
	if err != nil || utc != time.UTC {
		t.Fatalf("bundled UTC without a database: %v, %v", utc, err)
	}
}

// transitions walks loc's oracle from start to end and returns every
// transition it reports.
func transitions(t *testing.T, loc *time.Location, start, end time.Time) []int64 {
	t.Helper()
	oracle := locationOracle(loc)
	var found []int64
	for at := start.Unix(); ; {
		next, ok, err := oracle.NextTransition(at)
		if err != nil {
			t.Fatal(err)
		}
		if !ok || next > end.Unix() {
			return found
		}
		if next <= at {
			t.Fatalf("%v: the transition after %d is %d, which is not later", loc, at, next)
		}
		found = append(found, next)
		at = next
	}
}

// TestLocationOracleFindsEveryTransition compares the transitions the oracle
// reports with the changes a scan of the same years finds by looking up every
// quarter of an hour, and checks each is the exact second the type changes.
func TestLocationOracleFindsEveryTransition(t *testing.T) {
	t.Parallel()
	const step = 15 * 60
	for _, loc := range []*time.Location{
		mustLoad(t, "America/New_York"), mustLoad(t, "Australia/Lord_Howe"), mustLoad(t, "Africa/Casablanca"),
		mustLoad(t, "Europe/Dublin"), mustLoad(t, "America/Sao_Paulo"), testEastern(t),
		tzifLocation(t, "Test/YearEnd", []tzifType{{0, false, "AAA"}, {3600, true, "BBB"}}, "AAA0BBB,J1/0,J365/12"),
	} {
		oracle := locationOracle(loc)
		// The hand-built zones are scanned from 1971 only: Go miscomputes a
		// POSIX TZ rule for instants before 1970, which no real zone asks of
		// it, since every real zone's recorded transitions run past 1970.
		spans := [][2]int{{1971, 1977}, {2018, 2026}, {2038, 2046}, {2094, 2101}}
		if !strings.HasPrefix(loc.String(), "Test/") {
			spans = append(spans, [2]int{1915, 1921})
		}
		for _, years := range spans {
			start := time.Date(years[0], time.January, 1, 0, 0, 0, 0, time.UTC)
			end := time.Date(years[1], time.January, 1, 0, 0, 0, 0, time.UTC)
			found := transitions(t, loc, start, end)
			for _, at := range found {
				before, _ := oracle.OffsetAt(at - 1)
				after, _ := oracle.OffsetAt(at)
				if before == after {
					t.Errorf("%v: nothing changes at the transition %v", loc, time.Unix(at, 0).UTC())
				}
			}
			changes := 0
			previous, _ := oracle.OffsetAt(start.Unix())
			for at := start.Unix() + step; at <= end.Unix(); at += step {
				current, _ := oracle.OffsetAt(at)
				if current != previous {
					changes++
					if !slices.ContainsFunc(found, func(tr int64) bool { return tr > at-step && tr <= at }) {
						t.Errorf("%v: the oracle reports no transition for the change before %v", loc, time.Unix(at, 0).UTC())
					}
				}
				previous = current
			}
			if changes != len(found) {
				t.Errorf("%v from %d to %d: the oracle reports %d transitions, a scan finds %d changes", loc, years[0], years[1], len(found), changes)
			}
		}
	}
}

func TestLocationOracleAnswers(t *testing.T) {
	t.Parallel()
	oracle := locationOracle(mustLoad(t, "America/New_York"))
	summer, _ := oracle.OffsetAt(time.Date(2024, time.July, 1, 0, 0, 0, 0, time.UTC).Unix())
	if want := (libhegel.ZoneType{Offset: -4 * 3600, IsDST: true, Abbreviation: "EDT"}); summer != want {
		t.Fatalf("New York in July = %+v, want %+v", summer, want)
	}
	winter, _ := oracle.OffsetAt(time.Date(2024, time.January, 1, 0, 0, 0, 0, time.UTC).Unix())
	if want := (libhegel.ZoneType{Offset: -5 * 3600, Abbreviation: "EST"}); winter != want {
		t.Fatalf("New York in January = %+v, want %+v", winter, want)
	}
	next, found, err := oracle.NextTransition(time.Date(2020, time.December, 31, 12, 0, 0, 0, time.UTC).Unix())
	if want := time.Date(2021, time.March, 14, 7, 0, 0, 0, time.UTC).Unix(); err != nil || !found || next != want {
		t.Fatalf("the transition after the last day of 2020 = %d, %v, %v; want %d", next, found, err, want)
	}
	next, found, err = oracle.NextTransition(time.Date(2040, time.December, 31, 12, 0, 0, 0, time.UTC).Unix())
	if want := time.Date(2041, time.March, 10, 7, 0, 0, 0, time.UTC).Unix(); err != nil || !found || next != want {
		t.Fatalf("the transition after the last day of 2040 = %d, %v, %v; want %d", next, found, err, want)
	}
	if next, found, err := oracle.NextTransition(oracleHorizon - 1); found || err != nil {
		t.Fatalf("a transition past the probed years: %d, %v, %v", next, found, err)
	}
	if oracle.FirstYear != 1 || oracle.LastYear != 2100 || oracle.After != libhegel.ZONE_AFTER_INFER {
		t.Fatalf("probe range %d to %d, after %v", oracle.FirstYear, oracle.LastYear, oracle.After)
	}
	if oracleHorizon != time.Date(2101, time.January, 1, 0, 0, 0, 0, time.UTC).Unix() {
		t.Fatalf("the probe stops at %v", time.Unix(oracleHorizon, 0).UTC())
	}

	fixed := locationOracle(time.FixedZone("Test/Fixed", 3600))
	if next, found, err := fixed.NextTransition(0); found || err != nil {
		t.Fatalf("a transition in a fixed zone: %d, %v, %v", next, found, err)
	}
	if got, _ := fixed.OffsetAt(0); got != (libhegel.ZoneType{Offset: 3600, Abbreviation: "Test/Fixed"}) {
		t.Fatalf("a fixed zone's type = %+v", got)
	}
}

func TestDrawReportPrintsALocationByName(t *testing.T) {
	t.Parallel()
	for value, want := range map[any]string{
		time.FixedZone("+05:30", 19800): `loc := hegel.Draw(tc, g) = time.Location("+05:30")`,
		time.UTC:                        `loc := hegel.Draw(tc, g) = time.Location("UTC")`,
		(*time.Location)(nil):           `loc := hegel.Draw(tc, g) = (*time.Location)(nil)`,
		90 * time.Minute:                `loc := hegel.Draw(tc, g) = 5400000000000`,
		time.Date(2024, 5, 6, 7, 8, 9, 0, time.UTC).Unix(): `loc := hegel.Draw(tc, g) = 1714979289`,
	} {
		if got := formatDrawLine("loc := hegel.Draw(tc, g)", value); got != want {
			t.Errorf("got %q, want %q", got, want)
		}
	}
	if got, want := formatDrawLine("", time.UTC), `hegel.Draw[*time.Location](...) = time.Location("UTC")`; got != want {
		t.Errorf("got %q, want %q", got, want)
	}
	at := time.Date(2021, time.November, 7, 1, 30, 0, 0, mustLoad(t, "America/New_York"))
	if got, want := formatDrawLine("at := hegel.Draw(tc, g)", at), `at := hegel.Draw(tc, g) = time.Date(2021, time.November, 7, 1, 30, 0, 0, time.Location("America/New_York"))`; got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

func TestDefaultDurationsAndLocations(t *testing.T) {
	t.Parallel()
	type record struct {
		Timeout  time.Duration
		Zone     *time.Location
		Zones    []*time.Location
		Timeouts map[string]time.Duration
	}
	nonzero := false
	Test(t, func(ht *T) {
		v := Draw(ht, Default[record]())
		if v.Zone == nil || slices.Contains(v.Zones, nil) {
			ht.Fatalf("drew a nil location: %+v", v)
		}
		if _, err := time.LoadLocation(v.Zone.String()); err != nil {
			ht.Fatalf("drew the location %v, which Go cannot load", v.Zone)
		}
		nonzero = nonzero || v.Timeout%time.Second != 0
	}, WithTestCases(50))
	if !nonzero {
		t.Fatal("every default duration was a whole number of seconds")
	}

	want := libhegel.Duration{Seconds: -3, Nanoseconds: -5}
	got, err := Default[time.Duration]().draw(newStubTestCase(t, libhegel.OK, want, libhegel.OK, libhegel.OK))
	if err != nil || got != -3*time.Second-5 {
		t.Fatalf("got %v, %v", got, err)
	}
}
