# .NET frontend

Hegel for C# and F#: the `Hegel`, `Hegel.FSharp` and `Hegel.Xunit` NuGet
packages. The bindings drive libhegel, the engine in `../../libhegel`, in-process through
its C ABI (`../../libhegel/include/hegel.h`) with source-generated P/Invoke
(`LibraryImport`). Two library projects, both targeting .NET 8: `src/Hegel/`
(C#, everything the engine offers) and `src/Hegel.FSharp/` (F#, a surface
over `Hegel` that draws and prints F# values), each with an xunit test
project (`tests/Hegel.Tests/`, `tests/Hegel.FSharp.Tests/`; .NET 10 by
default, .NET 8 with `HEGEL_TEST_FRAMEWORK=net8.0`), plus `src/Hegel.Xunit/`
(C#, the `[HegelTest]` attribute for xUnit.net v2, tested by the C# suite).
The three packages share one version (`Directory.Build.props`) and release
together. Repo-wide
conventions are in the root `AGENTS.md`.

## Commands

```bash
just csharp::build-libhegel   # cargo-build the engine (release) into the workspace's target/
just csharp::check-test       # both suites, each with its 100% line coverage gate (coverlet)
just csharp::test             # dotnet test of the solution, no coverage gate
just csharp::test-only RunnerTests          # one class (a FullyQualifiedName~ filter)
just csharp::uncovered        # the library lines the last check-test left uncovered
just csharp::check-lint       # dotnet format + fantomas --check, then a build with -warnaserror
just csharp::format           # dotnet format (C#) and fantomas (F#)
just csharp::pack             # both NuGet packages into dist/, with the local engine
just csharp::check            # lint + test — what CI runs, less the other-OS and net8.0 jobs
```

- The recipes build libhegel first (incremental, so cheap once built). Plain
  `dotnet test Hegel.sln` works from this directory after
  `just csharp::build-libhegel`. The test projects' `HegelTargetFramework`
  property picks their target framework. fantomas comes from the tool
  manifest (`.config/dotnet-tools.json`; the recipes run `dotnet tool
  restore`).
- `scripts/engine.py` is the one place that knows where the engine is
  (`target/release/libhegel_c.{dylib,so}` or `hegel_c.dll` under the
  workspace's `target/` or `CARGO_TARGET_DIR`; `HEGEL_LIBHEGEL_PATH` overrides
  it) and what its version is (`libhegel/Cargo.toml`).
- MSBuild runs the engine script through the `HegelPython` property
  (`python3`, or `python` on Windows); the `.NET` SDK needed is 10 (the
  library's own `net8.0` target still needs the .NET 8 runtime only to run
  the tests under it).

## Engine distribution

The package bundles the libhegel shared library for every supported platform
under `runtimes/<rid>/native/`, the layout the .NET host probes for native
libraries, so `LibraryImport` finds it with no loader code of its own.
`LibraryLoader` only adds `$HEGEL_LIBHEGEL_PATH` in front of that probing and
warns when the loaded library's `hegel_version` differs from the version the
bindings were built against (`BuildInfo.EngineVersion`, generated from
`libhegel/Cargo.toml` by `scripts/engine.py build-info`). The bundled native
is the only way end users get the engine; nothing is downloaded at runtime.

The `HegelEngine` target in `Hegel.csproj` stages those files into
`native/` (gitignored) by running `scripts/engine.py stage`:

- `-p:HegelNativesSource=local` (the default) stages the in-tree build for
  the host alone, so development, the tests and a local `dotnet pack` use
  the engine of the checkout through the same bundled-library path a user
  gets.
- `-p:HegelNativesSource=release` (what `.github/scripts/publish_csharp.py`
  passes) stages every platform's library from the GitHub release of the
  libhegel version in `libhegel/Cargo.toml` (tag `libhegel/<version>`), each
  verified against its `.sha256` sidecar and cached under `native/downloads/`.
  It fails the build rather than producing a package without an engine.
- `-p:HegelNativesSource=skip` skips the staging (a build that will only load
  the engine through `HEGEL_LIBHEGEL_PATH`).

There is no engine version pin to keep in step in this directory.

## Layout

- `src/Hegel/Native/` — the binding edge. `NativeMethods` is the
  `LibraryImport` declaration of every `hegel_*` function; `Abi` the
  constants, result codes and enum codes of `hegel.h`; `ILibhegel` the
  internal interface the rest of the frontend calls (one method per ABI
  function, raw handles and result codes, the fakes in the tests implement
  it) and `Libhegel` its real implementation: one engine context per thread,
  UTF-8 marshalling, the output callback (`[UnmanagedCallersOnly]`) that
  forwards engine output and stores an exception it raised for rethrowing
  once the engine call returns, and `Require`, which turns a non-OK result
  code into a `HegelException` naming the operation. `Engine.Get()` is the
  process-wide instance; `Engine.SetForTesting` swaps in a fake.
- `Settings`, `Database`, `Example`, `TestLocation`, `Enums` — the settings
  surface. `Settings` is an immutable record whose defaults are read from
  the engine's resolved profile at construction (`Settings()` and
  `Settings.FromProfile`), so a `with` expression overrides exactly one
  setting on top of `hegel.toml` and the `HEGEL_*` environment variables;
  `Set(key, value)` is the string form for the ones the engine validates.
- `Hegel`, `Runner`, `RunReport`, `Reporter`, `CallerLocation` — the run.
  `Property.Test`/`Property.Run` fill in the test's identity from the compiler's
  caller attributes, `Runner` drives the engine's run loop (explicit
  examples, `ReproduceFailure` blobs, generation, shrinking, the final
  replay), builds the `Failure`s (origin `<ExceptionType> at <file>:<line>`
  of the first non-infrastructure frame, blob, caveat, output, draws) and
  the `RunReport`, whose `ThrowIfFailed` rethrows the minimal failure's own
  exception with its original stack (`ExceptionDispatchInfo`), or a
  `MultipleFailuresException`, `HealthCheckFailure`, `FlakyTestException`,
  `UnsatisfiableException`, `DidNotReproduceException` or `HegelRunException`
  as the run's outcome says. `Reporter` is the output hook; `PrintingReporter`
  writes what the other frontends write, with the same wording.
- `TestCase`, `LiveDataSource`, `ExplicitDraws`, `DataSource` — the draw
  surface. `TestCase` names draws (`label_1`, `draw_1` per naming scope),
  prints them through the generator (speculatively, so a draw that fails
  prints nothing), and forwards the engine primitives to its `IDataSource`:
  `LiveDataSource` over an engine test case handle (every result code
  translated: `E_STOP_TEST`/`E_ASSUME` abort the case, `E_INVALID_ARG` is a
  `HegelUsageException`, `E_CONCURRENT_USE` names `ConcurrentPool`) or
  `ExplicitDataSource` for an explicit example, where anything but a draw
  that the example supplies is an `InvalidExampleException`.
- `Printer`, `Document`, `Label`, `Dialect` — the engine's pretty-printer
  (regions, groups, breakables, speculation) with `Printer.Repr(value,
  staticType)`, which hands a value to the current `Dialect`, and the
  generator labels (`Label.Of`, `Label.Combine`, the engine's constants).
  `Dialect` is the language draws print in: `Dialect.CSharp` prints
  `var name = <C# literal>;` through `Printer.Render`; another frontend
  installs its own with `Dialect.Use` (an `AsyncLocal` scope) around a run,
  which is how a C# generator drawn from F# prints as F#. The static type
  reaches the dialect because `ValueGenerator<T>` and `PoolGenerator<T>`
  pass `typeof(T)` (F# needs it to print a `null` as `None`).
- `HegelInfrastructureAttribute` — an assembly-level marker;
  `Runner.IsUserFrame` skips the frames of an assembly carrying it (and of
  `Hegel` itself) when working out a failure's origin, so a frontend layered
  on this package (`Hegel.FSharp` applies it) reports the user's frame.
- `Generator`, `Generators`, `Generators/` — `Generator<T>` (`DoDraw`,
  `Label`, `Map`/`Filter`/`FlatMap`/`PrintWith`) and `PrintableGenerator<T>`
  (`DoDrawAndPrint`, which prints as it draws); `Generators` is the facade;
  `Primitives`, `Strings`, `Temporal`, `Combinators`, `Derive` (reflection
  over scalars, enums, nullables, collections, tuples and records,
  `RecordGenerator<T>`) and `Internal` (`ValueGenerator<T>`, handle caches,
  size validation) hold the implementations.
- `src/Hegel.Xunit/` — the `Hegel.Xunit` package: `HegelTestAttribute`, a
  `FactAttribute` whose constructor takes `[CallerFilePath]`/`[CallerLineNumber]`
  (so the method's own file and line are the test location, with no help
  from the runner) and whose `Profile`/`TestCases` are the settings surface;
  `HegelTestDiscoverer` (named by the attribute's `[XunitTestCaseDiscoverer]`)
  checks the method's shape (one `TestCase` parameter, `void`, non-generic)
  and yields a `HegelTestCase`, or xunit's `ExecutionErrorTestCase` with the
  problem; `HegelTestCase` → `HegelTestCaseRunner` → `HegelTestRunner` →
  `HegelTestInvoker` is xunit's own chain with the invoker's `CallTestMethod`
  replaced by `Property.Test` over the method as an `Action<TestCase>`
  delegate (so the failure is the assertion's own exception and the class
  name is the user's), the engine's report going into the test's xunit
  output. The assembly is `[assembly: HegelInfrastructure]`, so its frames
  never count as a failure's origin. The package depends on
  `xunit.extensibility.execution` and on `Hegel`; the core package keeps no
  test-framework dependency.
- `Stateful`, `Pool` — `Stateful.Run` drives a machine's `[Rule]` and
  `[Invariant]` methods (each taking one `TestCase`) through the engine's
  state machine primitives, sequentially or with worker threads for
  `MaxConcurrency > 1`, noting steps, rules and invariants with the same
  strings as the Java frontend; `Pool<T>`/`ConcurrentPool<T>` track values
  over the engine's pool primitives.

- `src/Hegel.FSharp/` — the F# package, compiled in this order: `Repr.fs`
  (`Repr.render`, F# literals for every value by runtime type with the
  static type deciding how `null` prints, and `Repr.dialect`, the
  `Dialect` that prints `let name = value`), `Generators.fs` (the F#
  generator classes: `ListGen`, `ArrayGen`, `SetGen`, `MapGen`,
  `OptionGen`, `ValueOptionGen`, `TupleGen`, `RecordGen`, `UnionGen`, each
  a `PrintableGenerator<T>` printing F# syntax; `Collections.draw` mirrors
  the C# `ListGenerator`'s speculation and duplicate rejection),
  `Auto.fs` (derivation: a plan per type, recursive unions and records
  through `Generators.Recursive` with the cases that avoid the type as
  base cases, mutual recursion through the `Selves` of the scope, a
  `HegelUsageException` for a type with no base case; non-F# leaves go to
  the C# `Derive.FromType`), `Gen.fs` (the `Gen` module and the `gen { }`
  builder) and `Hegel.fs` (the `Hegel` static type: `test`/`run` with
  caller-info parameters, installing `Repr.dialect` around
  `Property.Run`). It reads `Hegel`'s internals through
  `InternalsVisibleTo`.

## Rules

- **Coverage is 100%** line coverage of `src/Hegel`, `src/Hegel.Xunit` and
  `src/Hegel.FSharp`, each enforced by coverlet in `just csharp::check-test`
  (generated `*.g.cs` files are excluded). Everything is covered by real-engine tests plus
  `DelegatingLibhegel`-driven error-path tests (`tests/Hegel.Tests/Support/`):
  a fake subclasses the forwarding wrapper, overrides one method to return
  the result code the real engine never would, and is installed with
  `Engines.Override`. `scripts/uncovered.py` lists the lines a failed gate
  left uncovered (CI runs it on failure). There is no C# equivalent of
  `// nocov`; a line that cannot be covered is a design problem to fix.
  The F# gate measures the optimised build, so `Hegel.FSharp.fsproj` turns
  cross-assembly optimisation off (`--crossoptimize-`, so the tests run the
  library's own IL rather than inlined copies) and the few one-line helpers
  the F# optimiser would inline within the assembly carry
  `[<MethodImpl(MethodImplOptions.NoInlining)>]`. `inline` functions have no
  runnable body, so the package has none.
- **Never mock the engine for behaviour.** The behaviour suite runs against
  the real libhegel; fakes exist only for the result-code and error paths
  the engine cannot be driven into.
- **Engine divergences are fixed in the engine.** When libhegel misbehaves,
  fix it in `../../libhegel` (with a `libhegel/RELEASE.md`) in the same PR
  rather than working around it in the bindings.
- **Every PR that changes what ships needs a `RELEASE.md` here.** That means
  any change to `src/` or `Directory.Build.props`. See the root
  `CONTRIBUTING.md` for the format and the semver rules (on 0.x, breaking
  changes are `minor`, everything else `patch`). CI checks this
  (`.github/scripts/release.py check`); on merge to main the release job
  bumps `<Version>` in `Directory.Build.props`, moves the text into
  `CHANGELOG.md`, tags `frontends/csharp/<version>`, and the `release-csharp`
  job publishes both packages to nuget.org (`.github/scripts/publish_csharp.py`,
  `Hegel` first, through NuGet trusted publishing, bundling the libhegel
  GitHub release's binaries in `Hegel`). A libhegel release re-releases this frontend too, with an
  auto-generated engine-bump entry unless the PR writes a `RELEASE.md` here.
  Never edit `CHANGELOG.md` by hand.

## Conventions

- Formatting is `dotnet format` for C# (the `.editorconfig` here) and
  fantomas for F#; `just csharp::format` before committing. The build treats
  warnings as errors, with the code-style analyzers on and XML documentation
  required for every public member.
- The F# library is idiomatic F# over the C# one, not a port of it: it
  reuses the C# scalars, `TestCase`, `Settings` and `Stateful`, and adds
  only what F# values need (its collections, options, tuples, records and
  unions, and printing). Its tests are xunit `[<Fact>]`s in modules, one per
  source file, with `Support.fs` holding the quiet settings and the
  `show`/`printed` helpers.
- Tests are xunit `[Fact]`s with assembly-wide parallelization off (the
  engine keeps one context per thread and the fakes are process-global).
  Test settings start from `Engines.Quiet` (no database, quiet, no blob) so
  a test leaves no `.hegel/` directory behind and prints nothing.
- The test suites are the `Hegel.Tests` and `Hegel.FSharp.Tests`
  assemblies, which `Runner.OriginOf` treats as user code, so origin
  assertions can name a test file.
- `Hegel.Xunit` is tested in `XunitTests.cs` by driving `HegelTestDiscoverer`
  and the test case's `RunAsync` by hand over a private nested sample class
  (xunit discovers only public classes, so its failing and malformed samples
  never run as tests) with a recording `IMessageBus`; the two `[HegelTest]`
  methods on the public test class prove xunit's own discovery wiring.
- Tests never write `Console.Error` noisily: pass `Reporter.Silent` or a
  `Reporter.Printing(new StringWriter())` to assert on output.
