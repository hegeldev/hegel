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

// ZoneSource is a time zone database that zone generators draw from.
//
// It decides which names a [ZoneGenerator] draws and which daylight-saving and
// offset transitions a [ZonedDatetimeGenerator] aims at. Sources are made with
// package [hegel.dev/go/hegel/temporal]: temporal.HostZoneSource, the default,
// is the database Go's own time package reads; temporal.BundledZoneSource is
// the one built into Hegel; temporal.ZoneSourceFromLocations holds locations
// of your choosing. A ZoneSource is safe for concurrent use and is meant to be
// built once, in a package-level variable, not inside a test body. Values
// compare equal when they are the same source. The zero value is not a source:
// generators given it fail when drawn from.
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
	locate func(ctx *libhegel.Context, name string) (*time.Location, error)

	once   sync.Once
	handle *libhegel.ZoneSource
	err    error

	mu    sync.Mutex
	specs map[[2]bool]*libhegel.TimezoneSpec
}

func newZoneSource(open func(ctx *libhegel.Context) (*libhegel.ZoneSource, error), locate func(ctx *libhegel.Context, name string) (*time.Location, error)) *zoneSource {
	return &zoneSource{open: open, locate: locate, specs: make(map[[2]bool]*libhegel.TimezoneSpec)}
}

var (
	errNoZoneSource = errors.New("hegel: no zone source: the zero ZoneSource is not one, and a zero-value zone generator has none; " +
		"use temporal.HostZoneSource, temporal.BundledZoneSource or temporal.ZoneSourceFromLocations, " +
		"and build generators with temporal.Zones and temporal.ZonedDatetimes")
	errNoZoneDatabase = errors.New("hegel: Go has no time zone database, so there are no time zones to generate: " +
		"time.LoadLocation loads none of the IANA zones. Import _ \"time/tzdata\" in the test to embed a database " +
		"(or point the ZONEINFO environment variable at one), or pass a zone source built with temporal.ZoneSourceFromLocations")
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
// [time.LoadLocation]: the names it can load, and every location loaded so
// far.
type goZones struct {
	load  func(name string) (*time.Location, error)
	cache sync.Map

	once     sync.Once
	loadable []string
	err      error
}

func newGoZones(load func(name string) (*time.Location, error)) *goZones {
	return &goZones{load: load}
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

// names returns the names the loader can load, found on first use. A loader
// cannot list its names, so they are the names of the engine's bundled
// database that it loads.
func (z *goZones) names(ctx *libhegel.Context) ([]string, error) {
	z.once.Do(func() {
		var bundled []string
		bundled, z.err = bundledZoneNames(ctx)
		for _, name := range bundled {
			if _, err := z.lookup(name); err == nil {
				z.loadable = append(z.loadable, name)
			}
		}
	})
	return z.loadable, z.err
}

// bundledZoneNames returns every name of the engine's bundled database.
func bundledZoneNames(ctx *libhegel.Context) ([]string, error) {
	bundled, err := ctx.ZoneSourceBundled()
	if err != nil {
		return nil, err
	}
	defer bundled.Free()
	count, err := bundled.NameCount(ctx)
	if err != nil {
		return nil, err
	}
	names := make([]string, 0, count)
	for index := range count {
		name, err := bundled.Name(ctx, index)
		if err != nil {
			return nil, err
		}
		names = append(names, name)
	}
	return names, nil
}

// database returns the names the loader can load, or [errNoZoneDatabase]
// when it has no time zone database at all. UTC does not count: Go loads it
// without one.
func (z *goZones) database(ctx *libhegel.Context) ([]string, error) {
	names, err := z.names(ctx)
	if err != nil {
		return nil, err
	}
	if !slices.ContainsFunc(names, func(name string) bool { return name != "UTC" }) {
		return nil, errNoZoneDatabase
	}
	return names, nil
}

// locate loads the location for a name the engine drew. A name the loader
// lacks rejects the test case, since a location that cannot be loaded cannot
// be handed out.
func (z *goZones) locate(ctx *libhegel.Context, name string) (*time.Location, error) {
	loc, err := z.lookup(name)
	if err == nil {
		return loc, nil
	}
	if _, err := z.database(ctx); err != nil {
		return nil, err
	}
	return nil, libhegel.E_ASSUME
}

func (z *goZones) open(ctx *libhegel.Context) (*libhegel.ZoneSource, error) {
	names, err := z.database(ctx)
	if err != nil {
		return nil, err
	}
	return openLocationSource(ctx, names, func(name string) *time.Location {
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
	hostZones             = newGoZones(time.LoadLocation)
	hostZoneSourceImpl    = hostZones.hostSource()
	bundledZoneSourceImpl = hostZones.bundledSource()
	defaultZoneSourceImpl atomic.Pointer[zoneSource]
)

func init() {
	defaultZoneSourceImpl.Store(hostZoneSourceImpl)
}

func hostZoneSource() ZoneSource {
	return ZoneSource{hostZoneSourceImpl}
}

func bundledZoneSource() ZoneSource {
	return ZoneSource{bundledZoneSourceImpl}
}

func zoneSourceFromLocations(locations ...*time.Location) ZoneSource {
	byName := make(map[string]*time.Location, len(locations))
	names := make([]string, 0, len(locations))
	for _, loc := range locations {
		if loc == nil {
			panic("hegel: temporal.ZoneSourceFromLocations: nil location")
		}
		name := loc.String()
		if _, ok := byName[name]; ok {
			panic(fmt.Sprintf("hegel: temporal.ZoneSourceFromLocations: two locations are named %q", name))
		}
		byName[name] = loc
		names = append(names, name)
	}
	return ZoneSource{newZoneSource(
		func(ctx *libhegel.Context) (*libhegel.ZoneSource, error) {
			return openLocationSource(ctx, names, func(name string) *time.Location { return byName[name] })
		},
		func(_ *libhegel.Context, name string) (*time.Location, error) { return byName[name], nil },
	)}
}

func defaultZoneSource() ZoneSource {
	return ZoneSource{defaultZoneSourceImpl.Load()}
}

func setDefaultZoneSource(source ZoneSource) ZoneSource {
	if source.impl == nil {
		panic("hegel: temporal.SetDefaultZoneSource: the zero ZoneSource is not a zone source")
	}
	return ZoneSource{defaultZoneSourceImpl.Swap(source.impl)}
}
