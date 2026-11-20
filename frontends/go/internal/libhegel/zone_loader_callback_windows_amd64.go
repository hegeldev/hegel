//go:build windows && amd64

package libhegel

// zoneLoaderCallback is the hegel_zone_loader_t every zone source's loader
// goes through. The Windows x64 calling convention passes the name, a
// 16-byte hegel_string_t, as a pointer to a copy the caller owns.
func zoneLoaderCallback(userData uintptr, ctx ctxT, source zoneSourceT, name *stringT) uintptr {
	return loadZone(userData, ctx, source, *name)
}
