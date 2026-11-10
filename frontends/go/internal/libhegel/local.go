package libhegel

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
)

var getwd = os.Getwd

// localProfiles are the cargo profiles a local libhegel build is looked for
// in, in order of preference.
var localProfiles = []string{"release", "debug"}

// localLibraryName is the file name `cargo build -p hegeltest-c` gives
// libhegel on goos: the crate's library is hegel_c, so cargo names it
// libhegel_c.<ext>, without the lib prefix on Windows.
func localLibraryName(goos string) string {
	switch goos {
	case "windows":
		return "hegel_c.dll"
	case "darwin":
		return "libhegel_c.dylib"
	default:
		return "libhegel_c.so"
	}
}

// localTargetDir returns the cargo target directory of the nearest hegel
// checkout: hegel/target in dir or in the nearest of its parents that has
// one.
//
// It returns "" when there is none up to the filesystem root.
func localTargetDir(dir string) string {
	for {
		target := filepath.Join(dir, "hegel", "target")
		if info, err := os.Stat(target); err == nil && info.IsDir() {
			return target
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return ""
		}
		dir = parent
	}
}

// findLocalLibrary returns the path of the libhegel built in the nearest
// hegel checkout, found from the working directory: its release build, which
// is what the frontends' just recipes build, or else its debug build.
//
// It is the fallback for a build that embeds no libhegel, which is any build
// of an untagged hegel.dev/go/hegel, such as one a replace directive points at
// a local checkout. Only the nearest checkout counts, so an unbuilt checkout
// never falls through to a stale build further up.
//
// It returns an error naming both ways to provide libhegel when there is no
// checkout or the nearest one has no build.
func findLocalLibrary() (string, error) {
	dir, err := getwd()
	if err != nil {
		return "", fmt.Errorf("find a local libhegel build: %w", err)
	}
	name := localLibraryName(runtime.GOOS)
	target := localTargetDir(dir)
	if target == "" {
		return "", fmt.Errorf("this build of hegel.dev/go/hegel embeds no libhegel for %s/%s, "+
			"and there is no hegel/target in %s or any of its parents: "+
			"run `cargo build --release -p hegeltest-c` in a hegel checkout there, or set %s to a libhegel build",
			runtime.GOOS, runtime.GOARCH, dir, LibraryPathEnv)
	}
	for _, profile := range localProfiles {
		candidate := filepath.Join(target, profile, name)
		if info, err := os.Stat(candidate); err == nil && info.Mode().IsRegular() {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("this build of hegel.dev/go/hegel embeds no libhegel for %s/%s, "+
		"and the nearest hegel checkout's %s has no release/%s or debug/%s: "+
		"run `cargo build --release -p hegeltest-c` in it, or set %s to a libhegel build",
		runtime.GOOS, runtime.GOARCH, target, name, name, LibraryPathEnv)
}
