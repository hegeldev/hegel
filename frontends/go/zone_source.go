package hegel

import (
	"errors"
	"fmt"
	"slices"
	"sync"
	"sync/atomic"
	"time"

	"hegel.dev/go/hegel/internal/libhegel"
)

//go:generate go run scripts/gen-zone-catalogue.go

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
type ZoneSource struct {
	impl *zoneSource
}

// zoneSource is the state behind a [ZoneSource]. Its engine handle and its
// time zone specs are created the first time a draw needs them, because
// creating them takes a libhegel context and generators are built without one.
type zoneSource struct {
	// open creates the engine's handle on the source.
	open func(ctx *libhegel.Context) (*libhegel.ZoneSource, error)
	// locate returns the location to hand out for a name the engine drew.
	locate func(name string) (*time.Location, error)

	once   sync.Once
	handle *libhegel.ZoneSource
	err    error

	mu    sync.Mutex
	specs map[[2]bool]*libhegel.TimezoneSpec
}

func newZoneSource(open func(ctx *libhegel.Context) (*libhegel.ZoneSource, error), locate func(name string) (*time.Location, error)) *zoneSource {
	return &zoneSource{open: open, locate: locate, specs: make(map[[2]bool]*libhegel.TimezoneSpec)}
}

var (
	errNoZoneSource = errors.New("hegel: no zone source: the zero ZoneSource is not one, and a zero-value zone generator has none; " +
		"use HostZoneSource, BundledZoneSource or ZoneSourceFromLocations, and build generators with Zones and ZonedDatetimes")
	errNoZoneDatabase = errors.New("hegel: Go has no time zone database, so there are no time zones to generate: " +
		"time.LoadLocation loads none of the IANA zones. Import _ \"time/tzdata\" in the test to embed a database " +
		"(or point the ZONEINFO environment variable at one), or pass a zone source built with ZoneSourceFromLocations")
)

// engine returns the engine's handle on the source, creating it on first use.
func (s *zoneSource) engine(ctx *libhegel.Context) (*libhegel.ZoneSource, error) {
	if s == nil {
		return nil, errNoZoneSource
	}
	s.once.Do(func() { s.handle, s.err = s.open(ctx) })
	return s.handle, s.err
}

// newSpec creates a time zone spec that draws from the source.
func (s *zoneSource) newSpec(ctx *libhegel.Context) (*libhegel.TimezoneSpec, error) {
	handle, err := s.engine(ctx)
	if err != nil {
		return nil, err
	}
	spec, err := ctx.TimezoneSpecNew()
	if err != nil {
		return nil, err
	}
	if err := spec.SetSource(ctx, handle); err != nil {
		spec.Free()
		return nil, err
	}
	return spec, nil
}

// sharedSpec returns the source's spec for every name the two flags allow.
// The source keeps one per pair of flags, so the engine reads the source's
// names once rather than at every draw.
func (s *zoneSource) sharedSpec(ctx *libhegel.Context, links, legacy bool) (*libhegel.TimezoneSpec, error) {
	if s == nil {
		return nil, errNoZoneSource
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	key := [2]bool{links, legacy}
	if spec := s.specs[key]; spec != nil {
		return spec, nil
	}
	spec, err := s.newSpec(ctx)
	if err != nil {
		return nil, err
	}
	err = firstError(
		func() error { return spec.SetLinks(ctx, links) },
		func() error { return spec.SetLegacy(ctx, legacy) },
	)
	if err != nil {
		spec.Free()
		return nil, err
	}
	s.specs[key] = spec
	return spec, nil
}

// firstError runs steps in order and returns the first error one reports,
// without running the steps after it.
func firstError(steps ...func() error) error {
	for _, step := range steps {
		if err := step(); err != nil {
			return err
		}
	}
	return nil
}

// goZones is the time zone database behind a location loader such as
// [time.LoadLocation]: the engine's catalogue names the loader can load, and
// every location loaded so far.
type goZones struct {
	load     func(name string) (*time.Location, error)
	cache    sync.Map
	loadable func() []string
}

func newGoZones(load func(name string) (*time.Location, error)) *goZones {
	z := &goZones{load: load}
	z.loadable = sync.OnceValue(func() []string {
		var names []string
		for _, name := range zoneCatalogue {
			if _, err := z.lookup(name); err == nil {
				names = append(names, name)
			}
		}
		return names
	})
	return z
}

func (z *goZones) lookup(name string) (*time.Location, error) {
	if loc, ok := z.cache.Load(name); ok {
		return loc.(*time.Location), nil
	}
	loc, err := z.load(name)
	if err != nil {
		return nil, err
	}
	z.cache.Store(name, loc)
	return loc, nil
}

// hasDatabase reports whether the loader has a time zone database at all.
// UTC does not count: Go loads it without one.
func (z *goZones) hasDatabase() bool {
	return slices.ContainsFunc(z.loadable(), func(name string) bool { return name != "UTC" })
}

// locate loads the location for a name the engine drew. A name the loader
// lacks rejects the test case, since a location that cannot be loaded cannot
// be handed out.
func (z *goZones) locate(name string) (*time.Location, error) {
	loc, err := z.lookup(name)
	if err == nil {
		return loc, nil
	}
	if !z.hasDatabase() {
		return nil, errNoZoneDatabase
	}
	return nil, libhegel.E_ASSUME
}

func (z *goZones) open(ctx *libhegel.Context) (*libhegel.ZoneSource, error) {
	if !z.hasDatabase() {
		return nil, errNoZoneDatabase
	}
	return openLocationSource(ctx, z.loadable(), func(name string) *time.Location {
		loc, _ := z.lookup(name)
		return loc
	})
}

func (z *goZones) hostSource() *zoneSource {
	return newZoneSource(z.open, z.locate)
}

func (z *goZones) bundledSource() *zoneSource {
	return newZoneSource((*libhegel.Context).ZoneSourceBundled, z.locate)
}

// openLocationSource creates an engine zone source that knows names and takes
// each zone's data from the location find returns for it, the first time a
// draw needs the zone. find returns nil for a name it has no location for.
func openLocationSource(ctx *libhegel.Context, names []string, find func(name string) *time.Location) (*libhegel.ZoneSource, error) {
	source, err := ctx.ZoneSourceNew()
	if err != nil {
		return nil, err
	}
	loader := func(ctx *libhegel.Context, source *libhegel.ZoneSource, name string) error {
		loc := find(name)
		if loc == nil {
			return nil
		}
		return source.AddOracle(ctx, name, locationOracle(loc))
	}
	err = firstError(
		func() error { return source.Declare(ctx, names) },
		func() error { return source.SetLoader(ctx, loader, nil) },
	)
	if err != nil {
		source.Free()
		return nil, err
	}
	return source, nil
}

// The years a location is probed over. Before the first, Go and the engine
// both hold the zone's first type; after the last, the engine applies the
// yearly rule the last two years show, as Go applies the zone's own rule.
const (
	oracleFirstYear = 1
	oracleLastYear  = 2100
)

var oracleHorizon = time.Date(oracleLastYear+1, time.January, 1, 0, 0, 0, 0, time.UTC).Unix()

const secondsPerDay = 24 * 60 * 60

// locationOracle describes loc to the engine by answering its lookups.
func locationOracle(loc *time.Location) libhegel.ZoneOracle {
	typeAt := func(instant int64) libhegel.ZoneType {
		t := time.Unix(instant, 0).In(loc)
		name, offset := t.Zone()
		return libhegel.ZoneType{Offset: int32(offset), IsDST: t.IsDST(), Abbreviation: name}
	}
	return libhegel.ZoneOracle{
		OffsetAt: func(instant int64) (libhegel.ZoneType, error) {
			return typeAt(instant), nil
		},
		NextTransition: func(after int64) (int64, bool, error) {
			current := typeAt(after)
			for at := after; at < oracleHorizon; {
				_, end := time.Unix(at, 0).In(loc).ZoneBounds()
				if end.IsZero() {
					break
				}
				next := end.Unix()
				if next <= at {
					// Past a zone's last recorded transition Go computes the
					// year's last period, which runs to the end of the year,
					// as ending 365 days after the year began: in a leap
					// year, at the start of December 31. For an instant on
					// that day the period really ends at the next midnight.
					next = at - at%secondsPerDay + secondsPerDay
				}
				// The same computation reports each new year as a boundary,
				// and a database can record a transition that changes nothing.
				if typeAt(next) != current {
					return next, true, nil
				}
				at = next
			}
			return 0, false, nil
		},
		FirstYear: oracleFirstYear,
		LastYear:  oracleLastYear,
		After:     libhegel.ZONE_AFTER_INFER,
	}
}

var (
	hostZones         = newGoZones(time.LoadLocation)
	hostZoneSource    = hostZones.hostSource()
	bundledZoneSource = hostZones.bundledSource()
	defaultZoneSource atomic.Pointer[zoneSource]
)

func init() {
	defaultZoneSource.Store(hostZoneSource)
}

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
	return ZoneSource{hostZoneSource}
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
	return ZoneSource{bundledZoneSource}
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
	byName := make(map[string]*time.Location, len(locations))
	names := make([]string, 0, len(locations))
	for _, loc := range locations {
		if loc == nil {
			panic("hegel: ZoneSourceFromLocations: nil location")
		}
		name := loc.String()
		if _, ok := byName[name]; ok {
			panic(fmt.Sprintf("hegel: ZoneSourceFromLocations: two locations are named %q", name))
		}
		byName[name] = loc
		names = append(names, name)
	}
	return ZoneSource{newZoneSource(
		func(ctx *libhegel.Context) (*libhegel.ZoneSource, error) {
			return openLocationSource(ctx, names, func(name string) *time.Location { return byName[name] })
		},
		func(name string) (*time.Location, error) { return byName[name], nil },
	)}
}

// DefaultZoneSource returns the zone source generators use when given none.
//
// It is [HostZoneSource] until [SetDefaultZoneSource] changes it.
func DefaultZoneSource() ZoneSource {
	return ZoneSource{defaultZoneSource.Load()}
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
	if source.impl == nil {
		panic("hegel: SetDefaultZoneSource: the zero ZoneSource is not a zone source")
	}
	return ZoneSource{defaultZoneSource.Swap(source.impl)}
}
