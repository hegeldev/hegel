# Hegel for Go

## Build Commands

```bash
just go::build-libhegel                          # Build the in-tree libhegel into the workspace's target/
just go::check-test                              # Run tests with 100% coverage enforcement
just go::check-test-no-coverage -run '^TestLibhegelEndToEnd$' ./internal/libhegel  # Run one named test
just go::format                                  # Auto-format Go code
just go::check-lint                              # Check formatting, the empty binaries, go fix, vet, and staticcheck
just go::check-docs                              # Run go doc over the package
just go::docs                                    # Serve API docs locally; open the printed URL
just go::check                                   # Run lint, check-docs, and tests
```

The tests run against the in-tree libhegel: the test recipes build it (in
release) and point `HEGEL_LIBHEGEL_PATH` at it (an already-set value wins). A
plain `go test` in the checkout also finds a build, since when nothing is
embedded the loader looks in the nearest `hegel/target/` in the working
directory or its parents, taking `release/` over `debug/`: the build
`just go::build-libhegel` (`cargo build --release -p hegeltest-c`) makes. `just go::docs` keeps running
until stopped.

The libhegel binaries under `internal/libhegel/libs/` are empty on main
(`just go::check-libs-empty` enforces it): a C ABI change and the binding
change that follows it land in the same PR, with no pinned release to align
to (see the `go-ffi` skill). Only the commit a release tags embeds binaries.

## What This Is

A Go implementation of the Hegel property-based testing library. Hegel is a
universal property-based testing protocol whose native engine ships as a Rust
shared library, `libhegel`, built from the `hegeltest-c` crate in this
repository's `libhegel/`.

## Architecture

The library is structured in layers, each building on the previous:

1. **FFI loader** (`internal/libhegel`): uses dlopen or similar to load libhegel
   at runtime. Contains a low-level API which is not necessarily idiomatic but
   very close to the exposed C API. All GC integration is handled here.
2. **Test runner**: `testCase` implements
   `TestCase` by routing every operation (generate, span, target, collection,
   mark_complete) to one libhegel C call.
3. **Generator[T]**: type-safe generator abstraction. Generators take a `TestCase`
   and produce typed values.

## Testing Philosophy

- **100% per-file code coverage** is mandatory. `just go::check` fails if any
  file is uncovered. Use `// coverage-ignore` only for genuinely untestable
  code. The annotation count is ratcheted in
  `coverage-ratchet.json` and cannot increase without justification (see the
  `go-coverage` skill).
- **Function-pointer stubs** are the primary way to cover libhegel error
  paths.
- **Use the real libhegel** for integration tests.

### Mutation testing

When adding or changing tests, run the normal Go tests and mutation test the
code they exercise with gomutants. Run:

```sh
go tool gomutants --changed-since origin/main ./...
```

The `--changed-since` flag limits mutations to changed lines. Use the
appropriate base ref if the branch does not target `main`.

Before adding a test for a surviving mutant, state the intended guarantee it
threatens and where that guarantee comes from, such as the protocol, documented
behavior, or an agreed security assumption. An internal invariant is worth
testing when it protects that guarantee. Assert the invariant at a stable
boundary; do not turn incidental implementation choices into requirements
merely to improve the mutation score. Explain survivors that do not reveal a
meaningful test gap.

Report the mutation command, survivors and explanations, and any run you could
not finish. Keep settings at their defaults; tune workers or timeouts only if a
run has resource or timeout problems. Start mutation testing early when a run may
take a while, and let it run in the background while doing independent review work.

## Project Conventions

- **Release notes**: a Go module ships every file under its directory, so every
  PR that changes anything under `frontends/go/` needs a `RELEASE/<slug>.md`
  (see the `changelog` skill), unless it is labelled `skip release` because it
  should not cut a release (a test-only change, say).
- **Releases**: the module's version is its tag, `frontends/go/v<version>`,
  so there is no version manifest; the release automation reads the highest
  such tag. The tag is not on main: ci.yml's `release-go` job puts it on a
  commit of its own whose parent is the release commit and which fills
  `internal/libhegel/libs/` with the libhegel release binaries, and creates a
  `Hegel for Go v<version>` GitHub release. The binaries are those of the
  libhegel version in `libhegel/Cargo.toml`; no engine version is pinned in
  source, and the loader caches the embedded binary under the SHA-256 of its
  bytes.
- **Default generators**: When adding a generator for a stdlib type, also add
  that type to the `defaultKnownTypes` map so `Default()` uses its specialized
  generator. Keep registrations for version-specific types in files with the
  corresponding Go build tags.
- **Zone catalogue**: `zone_catalogue.go` is generated from the engine's
  `libhegel/src/tzdata/tzdata.txt` by `go generate .` (it lists the IANA names
  in shrink order, which the host zone source declares to the engine, since
  the C ABI has no function that lists them). Regenerate it when the engine's
  bundled tzdata changes; `TestZoneCatalogueMatchesTheEngine` fails until then.
- **Module path**: `hegel.dev/go/hegel`
- **Package name**: `hegel` — single package for the library, users import `hegel.dev/go/hegel`
- **File naming**: lowercase, multi-word files use underscores (e.g., `project_root.go`)
- **Test files**: `*_test.go` in the same package (white-box testing for coverage)
- **Error handling**: Unexported functions should return `error` for failable operations. Reserve `panic()` for two cases: (1) truly unreachable code paths (e.g. encoding a fixed-shape CBOR map), and (2) the boundary of an exported API where the caller's contract is misuse — e.g. `Map()` panics on construction with a malformed source generator, and `Draw()` panics to unwind the test body when the underlying `draw` returns a sentinel error. Internally, propagate errors with `fmt.Errorf("...: %w", err)` rather than re-panicking.
- **Doc comments**: Every exported symbol must have a doc comment, usually starting with the symbol name.
  The first paragraph must consist of a single short sentence. Functions should have two more paragraphs: one describing inputs if necessary (for example sentinel values) and one describing outputs (including important sentinel errors).
- **Coverage**: 100% enforced via `go-test-coverage` with `// coverage-ignore` for exclusions; annotation count ratcheted in `coverage-ratchet.json`

## Developer Notes

### purego pitfalls

- Functions returning `const char*` should be typed `func() string` (not `*byte` or `uintptr`).
- C string arguments are passed as `string`. purego ensures that the memory is managed correctly.

## Restating libhegel invariants

Do not duplicate input validation which is done by libhegel. Only perform validation
if it is necessary to make libhegel calls safe, for example when converting int to uint64.
