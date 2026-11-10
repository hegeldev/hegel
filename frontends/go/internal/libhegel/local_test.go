package libhegel

import (
	"errors"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestLocalLibraryName(t *testing.T) {
	for goos, want := range map[string]string{
		"linux":   "libhegel_c.so",
		"freebsd": "libhegel_c.so",
		"darwin":  "libhegel_c.dylib",
		"windows": "hegel_c.dll",
	} {
		if got := localLibraryName(goos); got != want {
			t.Errorf("localLibraryName(%q) = %q, want %q", goos, got, want)
		}
	}
}

func TestLocalTargetDirIsTheNearestCheckout(t *testing.T) {
	workspace := t.TempDir()
	near := filepath.Join(workspace, "projects", "hegel", "target")
	far := filepath.Join(workspace, "hegel", "target")
	for _, dir := range []string{near, far} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	workload := filepath.Join(workspace, "projects", "workload", "pkg")

	if got := localTargetDir(workload); got != near {
		t.Errorf("localTargetDir(%q) = %q, want %q", workload, got, near)
	}
	if got := localTargetDir(filepath.Join(workspace, "other")); got != far {
		t.Errorf("localTargetDir from outside projects/ = %q, want %q", got, far)
	}
}

func TestLocalTargetDirIgnoresAFileNamedTarget(t *testing.T) {
	workspace := t.TempDir()
	if err := os.MkdirAll(filepath.Join(workspace, "hegel"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(workspace, "hegel", "target"), nil, 0o644); err != nil {
		t.Fatal(err)
	}

	if got := localTargetDir(workspace); got != "" {
		t.Errorf("localTargetDir(%q) = %q, want none", workspace, got)
	}
}

func writeLocalLibrary(t *testing.T, checkoutParent, profile string, content []byte) string {
	t.Helper()
	path := filepath.Join(checkoutParent, "hegel", "target", profile, localLibraryName(runtime.GOOS))
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, content, 0o755); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestFindLocalLibraryInSiblingCheckout(t *testing.T) {
	workspace := t.TempDir()
	want := writeLocalLibrary(t, workspace, "debug", []byte("debug build"))
	workload := filepath.Join(workspace, "workload", "pkg")
	if err := os.MkdirAll(workload, 0o755); err != nil {
		t.Fatal(err)
	}
	t.Chdir(workload)

	got, err := findLocalLibrary()
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Errorf("findLocalLibrary() = %q, want %q", got, want)
	}
}

func TestFindLocalLibraryPrefersRelease(t *testing.T) {
	workspace := t.TempDir()
	writeLocalLibrary(t, workspace, "debug", []byte("debug build"))
	want := writeLocalLibrary(t, workspace, "release", []byte("release build"))
	t.Chdir(workspace)

	got, err := findLocalLibrary()
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Errorf("findLocalLibrary() = %q, want %q", got, want)
	}
}

func TestFindLocalLibrarySkipsDirectories(t *testing.T) {
	workspace := t.TempDir()
	release := filepath.Join(workspace, "hegel", "target", "release", localLibraryName(runtime.GOOS))
	if err := os.MkdirAll(release, 0o755); err != nil {
		t.Fatal(err)
	}
	want := writeLocalLibrary(t, workspace, "debug", []byte("debug build"))
	t.Chdir(workspace)

	got, err := findLocalLibrary()
	if err != nil {
		t.Fatal(err)
	}
	if got != want {
		t.Errorf("findLocalLibrary() = %q, want %q", got, want)
	}
}

func TestFindLocalLibraryStopsAtTheNearestCheckout(t *testing.T) {
	workspace := t.TempDir()
	writeLocalLibrary(t, workspace, "debug", []byte("a stale build further up"))
	projects := filepath.Join(workspace, "projects")
	unbuilt := filepath.Join(projects, "hegel", "target")
	if err := os.MkdirAll(unbuilt, 0o755); err != nil {
		t.Fatal(err)
	}
	workload := filepath.Join(projects, "workload")
	if err := os.MkdirAll(workload, 0o755); err != nil {
		t.Fatal(err)
	}
	t.Chdir(workload)

	_, err := findLocalLibrary()
	if err == nil {
		t.Fatal("expected an error for an unbuilt nearest checkout")
	}
	for _, want := range []string{unbuilt, "cargo build --release -p hegeltest-c", LibraryPathEnv, localLibraryName(runtime.GOOS)} {
		if !strings.Contains(err.Error(), want) {
			t.Errorf("error should mention %q; got %q", want, err)
		}
	}
}

func TestFindLocalLibraryWithoutACheckout(t *testing.T) {
	dir := t.TempDir()
	t.Chdir(dir)

	_, err := findLocalLibrary()
	if err == nil {
		t.Fatal("expected an error without a hegel checkout")
	}
	for _, want := range []string{dir, "cargo build --release -p hegeltest-c", LibraryPathEnv} {
		if !strings.Contains(err.Error(), want) {
			t.Errorf("error should mention %q; got %q", want, err)
		}
	}
}

func TestFindLocalLibraryGetwdError(t *testing.T) {
	want := errors.New("no working directory")
	stubVar(t, &getwd, func() (string, error) { return "", want })

	_, err := findLocalLibrary()
	if !errors.Is(err, want) {
		t.Fatalf("findLocalLibrary() error = %v, want %v", err, want)
	}
}

func TestLoadFallsBackToLocalBuild(t *testing.T) {
	t.Setenv(LibraryPathEnv, "")
	stubVar(t, &embeddedLib, nil)
	workspace := t.TempDir()
	path := writeLocalLibrary(t, workspace, "release", []byte("not a shared library"))
	t.Chdir(workspace)

	_, err := load()
	if err == nil {
		t.Fatal("expected loading a corrupt local build to fail")
	}
	if !strings.Contains(err.Error(), path) {
		t.Errorf("expected error to name the local build %q; got %q", path, err)
	}
}

func TestLoadWithoutEmbeddedOrLocalBuild(t *testing.T) {
	t.Setenv(LibraryPathEnv, "")
	stubVar(t, &embeddedLib, nil)
	t.Chdir(t.TempDir())

	_, err := load()
	if err == nil {
		t.Fatal("expected load to fail with nothing to load")
	}
	if !strings.Contains(err.Error(), LibraryPathEnv) {
		t.Errorf("expected error to mention %s; got %q", LibraryPathEnv, err)
	}
}

func TestLoadRealLocalBuild(t *testing.T) {
	built := os.Getenv(LibraryPathEnv)
	if built == "" {
		t.Skip(LibraryPathEnv + " names no libhegel build to copy")
	}
	lib, err := os.ReadFile(built)
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv(LibraryPathEnv, "")
	stubVar(t, &embeddedLib, nil)
	workspace := t.TempDir()
	writeLocalLibrary(t, workspace, "release", lib)
	t.Chdir(workspace)

	syms, err := load()
	if err != nil {
		t.Fatal(err)
	}
	if syms == nil {
		t.Fatal("load returned no symbols")
	}
	// Windows cannot delete a loaded DLL, so unload the copy before the temp
	// directory is cleaned up.
	if err := syms.Close(); err != nil {
		t.Fatal(err)
	}
}

func TestLibraryPathMaterializesTheEmbeddedBinary(t *testing.T) {
	t.Setenv(LibraryPathEnv, "")
	cache := testHomeOverride(t)
	stubVar(t, &embeddedLib, []byte("embedded libhegel"))

	path, err := LibraryPath()
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(path, cache) {
		t.Errorf("LibraryPath() = %q, want a path under the cache %q", path, cache)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "embedded libhegel" {
		t.Errorf("materialized library holds %q, want the embedded bytes", got)
	}
}

func TestLibraryPathPrefersTheEnvironment(t *testing.T) {
	t.Setenv(LibraryPathEnv, "/explicit/libhegel.so")
	stubVar(t, &embeddedLib, []byte("embedded libhegel"))

	path, err := LibraryPath()
	if err != nil {
		t.Fatal(err)
	}
	if path != "/explicit/libhegel.so" {
		t.Errorf("LibraryPath() = %q, want the %s value", path, LibraryPathEnv)
	}
}
