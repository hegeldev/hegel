package libhegel

import (
	"runtime"
	"unsafe"

	"github.com/ebitengine/purego"

	"hegel.dev/go/hegel/internal/cgo"
)

// zoneLoaderT is hegel_zone_loader_t and zoneLoaderReleaseT is
// hegel_zone_loader_release_t: C function pointers, which purego represents
// as a uintptr.
type (
	zoneLoaderT        uintptr
	zoneLoaderReleaseT uintptr
)

// zoneTypeT mirrors hegel_zone_type_t as an oracle callback writes it. The
// abbreviation's pointer is a uintptr because the struct lives in memory the
// engine owns: storing a Go pointer there would run a write barrier over
// whatever bytes the engine left in the slot.
type zoneTypeT struct {
	utoff           int32
	isDST           bool
	abbreviation    uintptr
	abbreviationLen uint64
}

// zoneOracleT mirrors hegel_zone_oracle_t.
type zoneOracleT struct {
	offsetAt       uintptr
	nextTransition uintptr
	userData       uintptr
	firstYear      int32
	lastYear       int32
	after          ZoneAfter
}

// ZoneType is the local time type a zone has in force at some instant.
type ZoneType struct {
	// Offset is the UTC offset in seconds east of UT.
	Offset int32
	// IsDST reports whether the type is daylight saving time.
	IsDST bool
	// Abbreviation is the type's name, such as "EST".
	Abbreviation string
}

// ZoneOracle describes a zone by answering lookups, for
// [ZoneSource.AddOracle] to probe into explicit transitions.
type ZoneOracle struct {
	// OffsetAt returns the type in force at instant, in Unix seconds.
	OffsetAt func(instant int64) (ZoneType, error)
	// NextTransition returns the first instant after after at which the type
	// changes, or found false when it never changes again. A nil
	// NextTransition makes the probe sample OffsetAt once a day instead.
	NextTransition func(after int64) (instant int64, found bool, err error)
	// FirstYear and LastYear bound the probe.
	FirstYear, LastYear int32
	// After says what holds after LastYear: [ZONE_AFTER_HOLD] or
	// [ZONE_AFTER_INFER].
	After ZoneAfter
}

// ZoneLoader supplies a zone source's data for name the first time a draw or
// query needs it, by adding the zone to source, or adds nothing when it does
// not know the name. ctx and source are valid only until the loader returns.
// It is called from whichever goroutine draws, possibly concurrently, and
// must not use a test case.
type ZoneLoader func(ctx *Context, source *ZoneSource, name string) error

// ZoneSource wraps a hegel_zone_source_t: a set of time zone names and the
// data they resolve to.
type ZoneSource struct {
	pointer[zoneSourceT]
}

// ZoneSourceNew creates an empty zone source.
func (c *Context) ZoneSourceNew() (*ZoneSource, error) {
	source := new(ZoneSource)
	ok, err := allocateInto(c, &source.pointer, "hegel_zone_source_new", func(ctx ctxT, raw *zoneSourceT) Error {
		return c.syms.ZoneSourceNew(ctx, raw)
	}, c.syms.ZoneSourceFree)
	if !ok {
		return nil, err
	}
	return source, nil
}

// ZoneSourceBundled returns a handle on the engine's bundled time zone
// database, which cannot be changed.
func (c *Context) ZoneSourceBundled() (*ZoneSource, error) {
	source := new(ZoneSource)
	ok, err := allocateInto(c, &source.pointer, "hegel_zone_source_bundled", func(ctx ctxT, raw *zoneSourceT) Error {
		return c.syms.ZoneSourceBundled(ctx, raw)
	}, c.syms.ZoneSourceFree)
	if !ok {
		return nil, err
	}
	return source, nil
}

// Declare makes names known to the source, with their data to come from its
// loader the first time a draw or query needs it.
func (s *ZoneSource) Declare(ctx *Context, names []string) error {
	views := cStringArray(names)
	return ctx.invoke("hegel_zone_source_declare", func(ctx ctxT) Error {
		e := s.syms.ZoneSourceDeclare(ctx, s.raw, slicePtr(views), uint64(len(views)))
		runtime.KeepAlive(s)
		return e
	})
}

// AddOracle adds the zone name to the source by probing oracle, whose
// functions are called during this call only.
func (s *ZoneSource) AddOracle(ctx *Context, name string, oracle ZoneOracle) error {
	state := &zoneOracleState{syms: s.syms, oracle: oracle}
	handle := cgo.NewHandle(state)
	defer handle.Delete()
	desc := &zoneOracleT{
		userData:  uintptr(handle),
		firstYear: oracle.FirstYear,
		lastYear:  oracle.LastYear,
		after:     oracle.After,
	}
	if oracle.OffsetAt != nil {
		desc.offsetAt = zoneOffsetAtPtr
	}
	if oracle.NextTransition != nil {
		desc.nextTransition = zoneNextTransitionPtr
	}
	return ctx.invoke("hegel_zone_source_add_oracle", func(ctx ctxT) Error {
		e := s.syms.ZoneSourceAddOracle(ctx, s.raw, cString(name), desc)
		runtime.KeepAlive(s)
		return e
	})
}

// SetLoader sets the loader the source asks for the data of a name it lacks,
// or with a nil loader removes it. release, when not nil, is called once when
// the engine drops the loader: when another replaces it, when the source's
// last holder goes, or before this call returns when it fails.
func (s *ZoneSource) SetLoader(ctx *Context, loader ZoneLoader, release func()) error {
	var (
		loaderPtr  zoneLoaderT
		releasePtr zoneLoaderReleaseT
		userData   uintptr
	)
	if loader != nil {
		loaderPtr, releasePtr = zoneLoaderPtr, zoneLoaderReleasePtr
		userData = uintptr(cgo.NewHandle(&zoneLoaderState{syms: s.syms, load: loader, release: release}))
	}
	return ctx.invoke("hegel_zone_source_set_loader", func(ctx ctxT) Error {
		e := s.syms.ZoneSourceSetLoader(ctx, s.raw, loaderPtr, userData, releasePtr)
		runtime.KeepAlive(s)
		return e
	})
}

// TimezoneInfoIn reports what source knows about name. A nil source is the
// process's default zone source.
func (c *Context) TimezoneInfoIn(source *ZoneSource, name string) (TimezoneInfo, error) {
	var sourceRaw zoneSourceT
	if source != nil {
		sourceRaw = source.raw
	}
	var info timezoneInfoT
	err := c.invoke("hegel_timezone_info_in", func(ctx ctxT) Error {
		e := c.syms.TimezoneInfoIn(ctx, sourceRaw, cString(name), &info)
		runtime.KeepAlive(source)
		return e
	})
	return TimezoneInfo{
		Known:       info.known,
		PrimaryName: goString(info.primaryName),
		IsLink:      info.isLink,
		Rank:        info.rank,
		Tags:        info.tags,
	}, err
}

type zoneOracleState struct {
	syms   *symbols
	oracle ZoneOracle
	// abbreviation keeps the bytes the last OffsetAt answer points at alive
	// until the next call, as the engine requires.
	abbreviation string
}

type zoneLoaderState struct {
	syms    *symbols
	load    ZoneLoader
	release func()
}

var (
	zoneOffsetAtPtr       = purego.NewCallback(zoneOffsetAtCallback)
	zoneNextTransitionPtr = purego.NewCallback(zoneNextTransitionCallback)
	zoneLoaderPtr         = zoneLoaderT(purego.NewCallback(zoneLoaderCallback))
	zoneLoaderReleasePtr  = zoneLoaderReleaseT(purego.NewCallback(zoneLoaderReleaseCallback))
)

// callbackFailure reports err as the message of a failing callback and
// returns the callback's failure code. The callbacks return a uintptr, though
// C declares an int32_t, because purego.NewCallback maps to
// syscall.NewCallback on Windows, which requires exactly one uintptr result.
func callbackFailure(syms *symbols, ctx ctxT, err error) uintptr {
	_ = syms.ContextSetError(ctx, cString(err.Error()))
	return 1
}

func zoneOffsetAtCallback(userData uintptr, ctx ctxT, instant int64, outType *zoneTypeT) uintptr {
	state := cgo.Handle(userData).Value().(*zoneOracleState)
	t, err := state.oracle.OffsetAt(instant)
	if err != nil {
		return callbackFailure(state.syms, ctx, err)
	}
	state.abbreviation = t.Abbreviation
	*outType = zoneTypeT{
		utoff:           t.Offset,
		isDST:           t.IsDST,
		abbreviation:    uintptr(unsafe.Pointer(unsafe.StringData(state.abbreviation))),
		abbreviationLen: uint64(len(state.abbreviation)),
	}
	return 0
}

func zoneNextTransitionCallback(userData uintptr, ctx ctxT, after int64, outFound *bool, outInstant *int64) uintptr {
	state := cgo.Handle(userData).Value().(*zoneOracleState)
	instant, found, err := state.oracle.NextTransition(after)
	if err != nil {
		return callbackFailure(state.syms, ctx, err)
	}
	*outFound, *outInstant = found, instant
	return 0
}

// loadZone is the body of zoneLoaderCallback, whose C signature takes the
// name as a hegel_string_t by value and so differs by platform.
func loadZone(userData uintptr, ctx ctxT, source zoneSourceT, name stringT) uintptr {
	state := cgo.Handle(userData).Value().(*zoneLoaderState)
	borrowedCtx := &Context{syms: state.syms, raw: ctx}
	borrowedSource := &ZoneSource{pointer[zoneSourceT]{syms: state.syms, raw: source}}
	if err := state.load(borrowedCtx, borrowedSource, goString(name)); err != nil {
		return callbackFailure(state.syms, ctx, err)
	}
	return 0
}

func zoneLoaderReleaseCallback(userData uintptr) uintptr {
	handle := cgo.Handle(userData)
	state := handle.Value().(*zoneLoaderState)
	handle.Delete()
	if state.release != nil {
		state.release()
	}
	return 0
}
