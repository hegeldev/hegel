// Package temporalbridge hands package hegel's temporal constructors to
// package temporal.
//
// A generator can only be implemented in package hegel, which seals its
// Generator interface, so the generators of package temporal are built there.
// Package hegel cannot export the constructors without making them part of
// its own API, and cannot import package temporal, which imports it. It
// stores them here when it is initialized, and package temporal, initialized
// after it, reads them.
package temporalbridge

import "time"

// Constructors is the set of constructors package temporal exports.
//
// Each type parameter stands for the type of package hegel it is named
// after, which this package cannot name without importing package hegel.
type Constructors[DateGenerator, DatetimeGenerator, DurationGenerator, OffsetGenerator, ZoneGenerator, ZonedDatetimeGenerator, ZoneSource any] struct {
	Dates                   func() DateGenerator
	Datetimes               func() DatetimeGenerator
	Durations               func() DurationGenerator
	Offsets                 func() OffsetGenerator
	Zones                   func() ZoneGenerator
	ZonedDatetimes          func() ZonedDatetimeGenerator
	HostZoneSource          func() ZoneSource
	BundledZoneSource       func() ZoneSource
	ZoneSourceFromLocations func(locations ...*time.Location) ZoneSource
	DefaultZoneSource       func() ZoneSource
	SetDefaultZoneSource    func(source ZoneSource) ZoneSource
}

// Registered is the Constructors value package hegel stores.
//
// It is typed any because its type arguments are types of package hegel.
// Package temporal asserts it to the instantiation it expects, once, when it
// is initialized.
var Registered any
