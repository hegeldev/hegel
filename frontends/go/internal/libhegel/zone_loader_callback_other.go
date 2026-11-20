//go:build !(windows && amd64)

package libhegel

// zoneLoaderCallback is the hegel_zone_loader_t every zone source's loader
// goes through. The name, a two-word hegel_string_t passed by value, arrives
// in two integer registers under the System V and AAPCS64 calling
// conventions, so it is declared as its two words.
func zoneLoaderCallback(userData uintptr, ctx ctxT, source zoneSourceT, nameData *byte, nameLen uintptr) uintptr {
	return loadZone(userData, ctx, source, stringT{data: nameData, len: uint64(nameLen)})
}
