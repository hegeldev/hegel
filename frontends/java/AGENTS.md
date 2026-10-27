# Java frontend

Hegel for Java: the `dev.hegel:hegel`, `dev.hegel:hegel-jna` and
`dev.hegel:hegel-lowlevel` artifacts on Maven Central. The bindings drive
libhegel, the engine in `../../libhegel`, in-process through its C ABI
(`../../libhegel/include/hegel.h`). Two published frontend artifacts share one
API and one source tree: `dev.hegel:hegel` binds over the Java Foreign Function
& Memory API (FFM, `java.lang.foreign`, Java 22+) and `dev.hegel:hegel-jna`
binds over JNA (Java 17+). Users depend on exactly one. Both depend on a third,
deliberately minimal artifact, `dev.hegel:hegel-lowlevel` (Java 17+): the
binding contract (`Libhegel`, `Abi`, `LibhegelBackend`, `LibraryLoader`) for
people writing their own binding or a from-scratch frontend. It carries no
generators, runner, or printing. Repo-wide conventions are in the root
`AGENTS.md`.

## Commands

```bash
just java::build-libhegel   # cargo-build the engine (release) into the workspace's target/
just java::check-test       # mvn verify: the whole reactor with the 100% coverage gate (JDK 22+)
just java::check-test-jna   # mvn -pl hegel-jna -am verify: what works on JDK 17+ (CI runs 17 and 21)
just java::test             # mvn test, no coverage gate
just java::test-only RunnerTest#happyPathMarksValidAndFreesEverything   # one class or method
just java::check-lint       # spotless (palantir-java-format) check
just java::format           # spotless apply
just java::check-docs       # javadoc with doclint (needs no engine build)
just java::check            # lint + docs + test — what CI runs, less the JNA and other-OS jobs
```

- The test recipes build libhegel first (incremental, so cheap once built).
  Plain `mvn test -Dtest=RunnerTest` works from this directory after
  `just java::build-libhegel`; always run Maven from here, since
  `${maven.multiModuleProjectDirectory}` must be this directory for the
  shared source tree and `.mvn/jvm.config` to be found.
- Building the full reactor needs JDK 22+ (the FFM module);
  `maven.compiler.release` is 22 for `hegel` and 17 for `hegel-lowlevel`,
  `hegel-jna` and the shared tree, so keep shared and lowlevel code
  Java-17-clean. palantir-java-format needs the javac internals
  `.mvn/jvm.config` exports, and does not run on a JDK newer than the one
  CI lints with (25): point `JAVA_HOME` at a JDK 25 for `just java::check-lint`
  if your default is newer.
- `scripts/engine.py` is the one place that knows where the engine is
  (`target/release/libhegel_c.{dylib,so}` or `hegel_c.dll` under the
  workspace's `target/` or `CARGO_TARGET_DIR`; `HEGEL_LIBHEGEL_PATH` overrides
  it) and what its version is (`libhegel/Cargo.toml`).

## Engine distribution

Each frontend jar bundles the libhegel shared library for every supported
platform as the classpath resources `native/<os>-<arch>/libhegel.<ext>`;
`LibraryLoader` unpacks the host's to a per-user cache at runtime (falling back
to a temp dir), after `$HEGEL_LIBHEGEL_PATH` and the OS's shared-library search
path. The bundled native is the only way end users get the engine; nothing is
downloaded at runtime.

Maven stages those resources in each frontend module's `generate-resources`
phase by running `scripts/engine.py stage` (exec-maven-plugin, parent
`pom.xml`):

- `-Dhegel.natives.source=local` (the default) stages the in-tree build for the
  host alone, so development, the tests and a local `mvn package` use the
  engine of the checkout through the same bundled-resource path a user gets.
- `-Dhegel.natives.source=release` (what the `release` profile selects) stages
  every platform's library from the GitHub release of the libhegel version in
  `libhegel/Cargo.toml` (tag `libhegel/<version>`), each verified against its
  `.sha256` sidecar and cached under `native/` here (gitignored). It fails the
  build rather than producing a jar without an engine.
- `-Dhegel.natives.skip=true` skips the staging (javadoc needs no engine).

`hegel-lowlevel` bundles nothing; its build instead generates
`dev.hegel.lowlevel.BuildInfo` (`scripts/engine.py build-info`, into
`target/generated-sources/engine`) from `libhegel/Cargo.toml`, so
`BuildInfo.ENGINE_VERSION`, which `Engine` compares the loaded library's
`hegel_version` against to warn about a mismatched `$HEGEL_LIBHEGEL_PATH`, is
always the engine the bindings were built with. There is no engine version pin
to keep in step in this directory.

## Layout

Multi-module Maven build (`dev.hegel:hegel-parent`, not itself useful to
consumers):

- `hegel-lowlevel/` — a normal Maven module, package `dev.hegel.lowlevel`: the
  public `Libhegel` interface (one method per bound `hegel_*` function; raw
  `long` handles and return codes), `Abi` constants, `LibhegelException`, the
  `LibhegelBackend` SPI and `LibraryLoader`. Tests are fake-only
  (`LibraryLoaderTest`, `LibhegelLoadTest` with a `StubBackend` service
  provider on the test classpath); surefire sets `HEGEL_LIBHEGEL_PATH` to a
  dummy file so `Libhegel.load()` resolves without a bundled native.
- `shared/` — **not a Maven module**: the backend-neutral frontend source tree
  (`src/main/java`, `src/test/java`) compiled into *both* frontend jars via
  build-helper-maven-plugin `add-source`/`add-test-source`. Everything except
  the concrete binding lives here, including `FakeLibhegel` and the whole
  behaviour suite, which therefore runs against both backends.
- `hegel/` — the FFM artifact: `RealLibhegel` + `FfmBackend` (main),
  `RealLibhegelTest` + `FfmCoverageTest` (test), and the JPMS
  `module-consumer` invoker IT under `src/it`.
- `hegel-jna/` — the JNA artifact: `JnaLibhegel` + `JnaBackend` (main),
  `JnaLibhegelTest`.

Each frontend jar registers its backend in
`META-INF/services/dev.hegel.lowlevel.LibhegelBackend`; the shared `Engine`
calls `Libhegel.load(path)`, which finds it via `ServiceLoader` (automatic
modules may use any service, so this also works on the module path — the IT
proves it). The bindings throw `HegelException`, which extends
`dev.hegel.lowlevel.LibhegelException`; the runner aborts on the parent type so
third-party bindings behave the same. IDE caveat: IntelliJ attaches a shared
source root to only one module at a time (duplicate content roots are
unsupported); the Maven CLI is authoritative.

## Architecture

Data crosses the FFI boundary through libhegel's **typed draw functions**
(`hegel_generate_integer`, `hegel_generate_float`, `hegel_generate_string`
through opaque `hegel_string_generator_t` handles, structured
date/time/datetime/uuid/ip draws, `hegel_generate_integer_big` for
`BigInteger`s past `long`, …). Every fallible call takes a per-thread
`hegel_context_t*` first argument and returns a `hegel_result_t`; results come
back through out-parameters, and every handle is caller-owned and explicitly
freed.

Layers live in `dev.hegel`; the concrete generator implementations are in the
`dev.hegel.generators` subpackage (one class per generator: `IntegerGenerator`,
`TextGenerator`, `ListGenerator`, `RegexGenerator`, `EmailGenerator`,
`DateTimeGenerator`, `Derive`/`RecordGenerator`, etc.). The public
`Generator`/`TestCase`/`Generators`/`Hegel`/`Stateful` surface stays in
`dev.hegel`.

- **FFI binding** — `dev.hegel.lowlevel.Libhegel` (a public, fakeable,
  backend-neutral interface: opaque handles cross as raw `long` addresses, 0 =
  NULL, so it never mentions any FFI library's types) with one implementation
  per frontend artifact: `RealLibhegel` (FFM, in `hegel/`) and `JnaLibhegel`
  (JNA, in `hegel-jna/`), each registered as a `LibhegelBackend` service
  provider. `LibraryLoader` (lowlevel) resolves the library (override / OS
  library path / jar-bundled native unpacked to a cache — the natives live in
  the frontend jar, found through the classpath); `Engine` (shared) is the
  process-wide lazy holder of the loaded shared object (with a test hook) — it
  caches only the immutable, thread-safe library, never per-test state. Both
  bindings keep one libhegel error context per thread (read back by
  `lastErrorMessage()`, created on the thread's first call and freed by
  `releaseThreadContext()`, which each concurrent stateful worker calls before
  it exits, or otherwise by a `Cleaner` once the thread has been collected, so
  a user thread drawing through `TestCase.fork` never leaks one), pass date/time/datetime structs by value, pass every
  string as the engine's `hegel_string_t` (`{const char *data; size_t len}`:
  by value for a required string, by nullable pointer where absent is
  meaningful, as a contiguous struct array for name lists; a Java `null`
  crosses as `{NULL, 0}` by value and as NULL by pointer, and strings may
  contain NUL) and read strings back the same way (`readString`/`readStringPtr`
  in FFM, `readString`/`readStringAt` in JNA; `hegel_context_last_error`
  returns the struct by value, so the FFM downcall takes an arena as its
  allocator), copy engine-allocated string/bytes buffers out and free them
  (`hegel_owned_string_free`, `hegel_generate_bytes_result_free`), and bridge
  the per-run output callback (`hegel_output_callback_t`) to a
  `Consumer<String>` — an FFM upcall stub whose arena lives until `runFree`, or
  a JNA `Callback` strongly referenced until `runFree`. In the JNA binding, C
  `bool` crosses as `byte` (JNA's default boolean mapping is a 32-bit int) and
  `size_t` as `long` (the bundled natives are all 64-bit). `Abi` (lowlevel)
  holds the C constants; `Label` (shared) holds the frontend's span labels —
  opaque `u64`s the engine computes (`Label.of` is `hegel_label_from_name`,
  `Label.combine` is `hegel_label_combine`, both through `Engine.real()` so a
  test's fake binding never changes them; the `Label.*` constants are literal
  values, checked against `Label.of` by `LabelTest`, so that nothing touches
  the engine during class initialisation and a missing engine fails the run,
  not the first generator). Every generator has a `label()`:
  leaves use the other frontends' `hegel.<kind>` names (`hegel.integers`,
  `hegel.text`, ...), and a composite combines its kind (`hegel.lists`,
  `hegel.optional`, ...) with its components' labels, computed once per
  instance (`CachedLabel`) and used for the span it draws in. Filter attempts
  open `hegel.filter.attempt` spans and pool draws
  `hegel.pool.values_reusable`/`values_consumed` spans, as in Rust and
  Python. `hegel_settings_set_from_string` backs `Settings.set(key, value)`,
  Java's way to every setting by its `hegel.toml` key (Java has no CLI to pass
  flags through). The engine's
  `hegel_set_unicode_version` has no Java surface (Java reports no Unicode
  version). The grammar functions are bound except `hegel_grammar_placeholder`
  and `hegel_generate_grammar`: the frontend draws every grammar string through
  a `hegel_grammar_draw_t`, which splices in explicit terminals' values engine-side.
- **Per-case primitives** — `DataSource` is the abstraction generators draw
  against; `LiveDataSource` wraps the engine, translating return codes
  (`StopTest` → OVERRUN, `AssumeRejected` → INVALID, `INVALID_ARG` →
  `InvalidArgument`, an `IllegalArgumentException` with the engine's
  diagnostic, other negatives → `HegelException`) and short-circuiting once a
  case is aborted. `InvalidArgument` is the usage error of a running test: the
  runner rethrows it, failing the run at once as Rust's `InvalidArgument` does,
  while any other `IllegalArgumentException` the body throws is a failure of
  the property. `TestCase` also carries `event`/`eventValue`, `reject`,
  `drawSilent` (a draw that is neither printed nor recorded), `fork(worker)`
  (a `hegel_test_case_clone` + `hegel_test_case_set_worker` handle sharing the
  draw-naming scope of the handle it forks, freed when the case ends) and
  `repeat(body)` (a `hegel.collection` of silent draws; each repetition is a
  block noted `// Repetition #N`, with a fresh naming scope, freed when the
  repetition ends; an `assume` failure skips the repetition, and any other
  failure draws a silent boolean before propagating, so the shrinker can tell
  repetitions apart).
- **Output** — all of a case's output goes through the engine's per-case
  document: `TestCase.note` is `hegel_note`, and a top-level `draw` prints
  `var name = ` through the handle's `hegel_test_case_printer`, then has the
  generator print the value as it draws it, then `;`, all inside a
  speculative region (committed once the value is drawn, aborted if the draw
  throws; the engine holds back notes made mid-draw until the region closes).
  Printing is the generator's job, modelled on the Rust frontend (`pretty.rs`):
  `PrintableGenerator.doDrawAndPrint(tc, printer)` draws and prints in one step,
  and `TestCase.draw` and `drawAndPrint` take only a `PrintableGenerator`, so
  printability is checked at compile time (there is no runtime check), while
  `drawSilent` takes any `Generator` and calls its silent `doDraw`. A
  non-emitting or nested `draw` still runs the printing path, into
  `Printer.noop()`. Structural generators (`dev.hegel.generators`) are
  printable exactly when their parts are, as Rust's conditional
  `PrintableGenerator` impls are: each is a plain `Generator` class (`ListGenerator`,
  `SetGenerator`, `MapGenerator`, `ArrayGenerator`, `TupleGenerator`,
  `OptionalGenerator`, `OneOfGenerator`, `FilteredGenerator`,
  `RecursiveGenerator`) with a nested `Printable` subclass whose constructor
  takes printable parts and whose builder methods return `Printable`
  covariantly; the `Generators` factories are overloaded on `Generator` and
  `PrintableGenerator` arguments (implicitly typed lambdas pick by arity:
  `tc -> ...` is a `Generator`, `(tc, p) -> ...` a `PrintableGenerator`), and
  `PrintableGenerator` overrides `map` (to `MappedGenerator.Printable`, which
  has `printAsCall`) and `filter`. `map` is printable either way (it prints
  `repr`). Where printability would depend on a lambda's return type,
  which Java cannot overload on, the printable form is the only one:
  `flatMap`'s function, `Deferred.set` and `RecordGenerator.with` take
  `PrintableGenerator`s, and `recursive` over a printable base needs `extend`
  to return printable generators (a silent `RecursiveGenerator` still hands
  `extend` a printable sub-value generator, which prints with `repr`). Each
  structural generator has one body taking a nullable printer: `null` on the
  silent path, where each part is drawn with `drawSilent`, and the printer on
  the printing path, which only the `Printable` subclass takes, where each
  part is drawn with `TestCase.drawAndPrint` (which raises `drawDepth`, so a
  `draw` inside a generator prints nothing) and so prints itself in place
  (`Components`, whose cast to `PrintableGenerator` is safe by construction).
  `PrintabilityCompilationTest` compiles snippets with `javax.tools` to check
  that unprintable draws are rejected by javac.
  Collections, filter attempts and recursion attempts print each attempt in a
  `Printer.speculate()` region and commit only the accepted one. Leaf and
  value-producing generators (`ValueGenerator`: the leaves, `map`, `just`,
  `sampledFrom`, the one-argument `composite`; `PoolGenerator` for pool draws)
  draw silently and print the value with `Printer.repr`, which renders Java
  syntax — quoted, escaped strings and chars, arrays and collections as
  `[a, b]`, maps as `{k: v}`, records as `new Name(x)`, optionals as `Optional.of(x)` — as groups
  with breakables, and hands anything else's `toString()` to
  `hegel_printer_reflow`. `printWith`/`printAsRepr` (`PrintedGenerator`) and
  `MappedGenerator.Printable.printAsCall` are the value-printing escape hatches.
  `Printer` is public and binds the whole printer ABI (`if_break`, `comment`,
  `shift_indent`, `deferred`, `is_live`, speculation); a write to a dead
  region (`E_INVALID_HANDLE`) or through the no-op or a freed printer is
  ignored. Deferred regions are registered in a `Printer.Regions` owned by the
  case (`TestCase.Shared`) or `Document` and freed after the document is read.
  `Document` is a standalone `hegel_printer_new` document with
  `hegel_printer_options_*` for its width. Draw names (`x`, `x_2`, `draw_N`)
  are allocated before the draw and numbered Java-side, per naming scope as Rust's `child()` scopes them
  (the engine has no naming); `Failure.draws()` keys stay unique across scopes
  by suffixing a taken name. A draw label containing a newline is an
  `InvalidArgument` before anything is drawn, and an error abandoning a draw's
  speculative region, or reading a failing case's document, is attached to the
  body's exception as suppressed rather than replacing it. Indented sections
  use `hegel_test_case_block` (`TestCase.block`, a fresh naming scope, freed by
  `endBlock()` when the rule ends unless `pin()`ned by a `Pool` made through
  it, which keeps it until the case ends; a released source rejects further
  use with an `InvalidArgument`), worker attribution is
  `hegel_test_case_set_worker`, and after the body the runner reads the
  document (`hegel_printer_resolve` + `hegel_printer_value`) into lines for
  the `Reporter` (`caseOutput`) and `Failure.output()`. Nothing is buffered
  Java-side.
- **Run loop** — `Runner` builds the settings handle and drives
  `hegel_run_start` → `hegel_next_test_case` → `hegel_mark_complete`. The
  engine owns the *whole* run — generation, shrinking, the replays that confirm
  a failure, and the final replay of every failure it is about to report — so
  the runner never replays anything itself. It reads
  `hegel_test_case_should_capture` once per case: a *stamped* case's `TestCase`
  (or any case of a verbose run) prints its top-level draws and notes into the
  engine's per-case document (see **Output**; `isFinal()` exposes the
  stamp), a stamped case also records its draws as Java values, and
  every interesting case's exception is kept per origin in a capture map (a
  stamped capture outranks an unstamped one, newest wins at equal rank — the
  unstamped discovery of a failure that never fails again is all there is for
  an *unconfirmed* nondeterministic failure). After the loop drains,
  `hegel_run_result` yields PASSED, FAILED, or ERROR. On FAILED each distinct
  failure is built from its origin's capture: the rendered lines go to the
  reporter (`caseStarted(true)`, `caseOutput(line, true)` …
  `caseFinished(true)`) and become `Failure.output()`, and
  the `Failure` carries the body's own exception, the reproduce blob
  (`hegel_failure_reproduction_blob`, null for an unconfirmed failure and for a
  blob replay) and the **caveat** (`hegel_failure_caveat`, non-null for a
  failure the run saw fail to reproduce — `Failure.nondeterministic()`; the
  printing reporter prints it as `note: …`). A failure whose origin has no
  capture is an internal error. `Settings.reproduceFailure(String...)` drives
  the same loop over `hegel_run_start_blob` once per blob (the engine replays
  each until a replay fails, under its budget) and reports every distinct
  failure: none → `DidNotReproduceException` (`Runner.STALE_BLOB`), ERROR →
  the same typed error report as any run, after which the remaining blobs are
  not replayed; a blob the engine cannot
  decode fails `hegel_run_start_blob` itself and surfaces as the binding's
  `HegelException` with the engine's "could not be decoded" message. There is
  no Java-side flaky detection: the engine handles nondeterminism per
  `nondeterminismStrictness` (`NondeterminismStrictness.QUIET`/`WARN`
  confirm-by-replay and report with a caveat; `ERROR` aborts the run with a
  `FlakyTestException`). The runner
  returns a `RunReport` (status, client-side `RunStatistics` counted per
  `mark_complete` — the C ABI has no counter accessor — engine error,
  failures) and never prints: everything goes through the run's `Reporter`
  (`Reporter.printing(System.err)` is the default and reproduces the classic
  output; `Reporter.silent()` for library use). `Hegel.run` returns the
  report; `Hegel.test` = `run` + `RunReport.throwIfFailed()`, which rethrows a
  single failure as-is (checked exceptions included), aggregates several into
  an `AssertionError`, and maps ERROR by `hegel_run_result_error_kind`
  (`RunReport.errorKind()`) to a `HegelRunException`: `HealthCheckFailure`
  (carrying the failed `HealthCheck`), `FlakyTestException`,
  `UnsatisfiableException`, or the base type for usage and internal errors.
  `HegelException` (binding/engine errors) is always thrown, never reported.
  `Settings` is the immutable config; its closed-state setting types live
  alongside it — `Backend` (auto = leave it to the engine's profile, which
  picks urandom inside Antithesis / default / urandom), `Database`,
  `OptBoolean`, `NondeterminismStrictness` and `Verbosity` (`DEFAULT` = leave
  it to the engine), `TestLocation` — plus `printBlob` (print a
  copy-pasteable reproducer per failure) and `reproduceFailure` (replay a
  stored blob instead of running the property). Every engine setting is
  unset unless the user sets it: `Runner.applySettings` sends only what the
  user set, so the engine's resolution — its profile (`hegel.toml`, a
  registered profile, the shipped `development`/`ci`/`workload` profiles, or
  the one `Settings.fromProfile` names, resolved by
  `hegel_settings_new_for_profile`) with the `HEGEL_*` settings variables
  applied over it — stands for the rest, and explicit Java settings win over
  both. `Settings.registerProfile` snapshots a resolved handle with
  `hegel_settings_register_profile`; `Settings.setDefaultProfile` calls
  `hegel_set_default_profile`. `@HegelTest` names a test (its database key)
  `<fully.qualified.Class>.<method>`, the class being the test class running
  it (a subclass, for an inherited test), and locates it at the method's file
  and the first line of its body;
  `Hegel.test`/`Hegel.run` locate a test at their caller's frame
  (`Runner.callerLocation`) and send it with `hegel_settings_set_test_location`. There is no Java-side CI detection (the engine's `ci` profile
  covers it). `Libhegel.settingsNew` returns a raw result code because a
  malformed variable or `hegel.toml` fails it with `E_INVALID_ARG` (→
  `IllegalArgumentException`); after applying, the runner reads
  `settingsGetTestCases`/`settingsGetPrintBlob`/`settingsGetNondeterminismStrictness`/
  `settingsGetVerbosity` back and hands the *effective* `Settings` to the
  reporter and the run. `EnvironmentTest` covers the variables and `hegel.toml` through a child
  JVM (`EnvironmentFixture`), since the engine reads its own process
  environment. There is no single-test-case mode; `testCases(1)` is the
  one-case configuration.
- **Generators** — `Generator<T>` (public; `doDraw` is the silent draw) with
  `map`/`filter`/`flatMap`/`printWith`/`printAsRepr`, and
  `PrintableGenerator<T>` (see **Output**). Leaf
  generators call the typed draw bridges on `TestCase` (`generateInteger`,
  `generateFloat`, `generateString`, …); composite generators (collections via
  the engine's `new_collection`/`collection_more` protocol, each collection
  freed with `collectionFree` when its draw ends, `oneOf`, tuples,
  `map`/`filter`/`flatMap`) draw their parts through the test case inside
  labeled spans, mirroring the Rust frontend. String-shaped generators
  (`text`/`characters`/regex/email/url/domain) build a validated
  `hegel_string_generator_t` handle once per configuration and cache it
  (`HandleCache`, rebuilt if a test swaps the `Engine` binding);
  `StringGeneratorHandle` frees the engine allocation via a `Cleaner` when
  unreachable. `Generators.fromLark` (`LarkGenerator`) compiles its grammar once per
  configuration (`TestCase.grammar`: `hegel_grammar_builder_*`, reading each
  `%import`ed file from its `importPath`, then `hegel_grammar_new`; a
  `GrammarHandle` frees it via a `Cleaner`) and draws each string through a
  `hegel_grammar_draw_t` (`LiveDataSource.generateGrammar`): the engine asks for
  each explicit terminal's value in turn, the generator draws it silently from
  the user's generator, and the engine splices the values in, keeps the string
  under the size cap and retries or rejects; nothing about the splicing lives in
  Java. Text and binary generators with no `maxSize` send
  `Abi.MAX_SIZE_DEFAULT` for the engine to apply its default, and `filter` tries
  `Abi.FILTER_ATTEMPTS` (`HEGEL_FILTER_ATTEMPTS`) values. `Generators.recursive` (`RecursiveGenerator`) drives
  `hegel_new_recursion`: the engine decides each node's `recursionBranch`,
  and a leaf over budget (`recursionLeaf` → `E_RETRY`) unwinds the attempt
  with `OverBudget` and retries it. The engine closes a retried attempt's
  open spans itself, so `LiveDataSource` suppresses `stopSpan` while
  unwinding; `recursionFinish` → `E_RETRY` redraws a mispriced value, and
  `recursionRetry` → `E_ASSUME` rejects the case. A filtered consuming pool
  draw (`PoolDraws.drawFiltered`) consumes one value per attempt and gives
  every rejected one back with `hegel_pool_restore`. `Generators` is the
  factory facade.
- **Explicit examples** — `Settings.examples(Example...)` (or
  `@HegelTest(examples = {"method"})`, naming methods that return an
  `Example` or an `Iterable` of them) runs the body once per example before
  the engine's run, when the resolved phases (`settingsGetPhases`) include
  `EXPLICIT` and no blob is being replayed. The engine is not involved: the
  `TestCase` draws from `ExplicitDraws` (a draw takes the value named for it,
  else the next positional one; a silent draw takes the next positional one)
  over an `ExplicitDraws.noEngine()` data source that ignores `target`/events
  and rejects the engine primitives (spans, pools, state machines) as a usage
  error, and prints into a standalone `hegel_printer_new` document. `fork`
  shares the example's values and `repeat` runs its body once, as Rust's
  `ExplicitTestCase` does. Misfitting examples (a missing value, an unused
  one, a failed assumption, inside `repeat` or not) are
  `IllegalArgumentException`s. Without `reportMultipleFailures`
  (`settingsGetReportMultipleFailures`) the first failing example ends the
  run; with it, every example runs, failures are deduplicated by origin, the
  engine runs too, and its failures with an explicit failure's origin are
  dropped. An explicit failure makes an otherwise PASSED or ERROR run FAILED
  (an ERROR's message and kind stay on the report).
- **Public API** — `Hegel.test`/`Hegel.run`, `TestCase`
  (`draw`/`drawSilent`/`drawAndPrint`/`assume`/`reject`/`note`/`target`/`event`/`eventValue`/`fork`/`repeat`/`isFinal`/`span`),
  `Example`, `Generator`/`PrintableGenerator`, `Printer`/`Document` (the
  printer surface), `Label` (span labels;
  `Label.of(name)` mints labels through the engine), `Reporter`, `RunReport`,
  `Settings.infrastructurePackages` (extra class-name prefixes skipped by
  `Runner.originOf`), the `@HegelTest` annotation + `HegelTestExtension` (a
  JUnit 5 `TestTemplateInvocationContextProvider` that drives the engine loop
  and invokes the user method per case).
- **Stateful testing** — `Stateful.run(machine, tc[, options])` reflects
  `@Rule`/`@Invariant` methods (sorted by name for determinism) and registers
  them via `hegel_new_state_machine`: `@Rule(group = "g")` names become
  `rule_groups` ids (0.. in first-appearance order; rules that name none share
  `Stateful.ANONYMOUS_GROUP`, so a plain machine is all-zeros),
  `@Rule(weight = w)` becomes the `rule_weights` array (validated finite and
  positive in Java; `null` — the engine's all-equal default — when every rule
  is at 1; the engine samples proportionally among the *enabled* rules, so
  weights are hints, and tests check them per case, not in aggregate), and
  `Stateful.Options` carries the per-machine `step_count`
  (`Stateful.DEFAULT_STEP_COUNT` = 50; the engine has no default) and the
  concurrency bounds (default `1, 1`). The engine draws the concurrency level
  and the driver runs exactly that many workers. Two drivers share the
  registration and the round protocol — `hegel_state_machine_next_group` opens
  each round on the root handle (or reports `HEGEL_STATE_MACHINE_DONE`, which
  is `INT64_MIN`), `hegel_state_machine_next_rule(worker)` hands out the
  round's rules until the worker's join point, a rule that fails its own
  assumption is reported with `hegel_state_machine_rule_rejected(worker)`, and
  at each join point `hegel_state_machine_should_check_invariant` (root
  handle) decides which invariants run — always for
  `@Invariant(alwaysRun = true)`, sampled otherwise; the initial and final
  checks run every invariant unconditionally:
  - `maxConcurrency == 1` → the **sequential** driver: everything on the
    calling thread and the root handle as worker 0, one `STATEFUL_RULE` span
    per round (discarded on rejection), each rule's body under a `Step N: rule {`
    note in a `hegel_test_case_block` (indent 2) and closed by a `}` note.
  - `maxConcurrency > 1` → the **concurrent** driver, modelled on the Rust
    frontend's `run_concurrent_machine` (even when the drawn level is 1, so a
    run's output format is uniform). Persistent daemon worker threads
    (`hegel-worker-N`) live for the test case; per round the root thread notes
    the header (`---------------- Round k: group "name" ----------------`),
    makes a **fresh clone per worker** (`hegel_test_case_clone` +
    `hegel_test_case_set_worker`; cloning is a draw and can return
    `E_STOP_TEST` on a replay, hence the raw-rc convention for
    `Libhegel.testCaseClone`), hands each worker its clone through a queue
    (a worker notes `Rule: name {` on it, runs the rule in a block and notes
    `}`), waits for one event per worker (the join point), frees the clones (their
    output stays in the document, anchored where each clone was made and
    stamped `[worker N +X.XXXms]` by the engine; each rule's block names its
    draws in a fresh scope), and resolves the round in the
    Rust frontend's precedence: a control error
    (`LibhegelException`/`InvalidArgument`, or a worker that exited
    without reporting) is rethrown first; then an overrun or an engine-level
    `E_ASSUME` concludes the case, dropping any failures found alongside with
    a `Dropped concurrent failure from worker N` note; otherwise the lowest
    worker's failure is rethrown as-is (its stack trace is the worker's, so
    `originOf` still works) and the rest are noted as dropped. There is no
    cancellation: a failing worker ends its own round and the others finish
    theirs (an overrun aborts the family engine-side anyway). No spans are
    opened in the concurrent path (none of the bindings do; the engine owns
    rule structure). The `Reporter` is only ever called from the driving
    thread: a case's output, worker lines included, is read once its body has
    finished. Workers
    are stopped and joined in a `finally` before any leftover clone is freed.
  The machine handle is freed in a `finally`. Pools are family-wide: every pool
  created on a test case or its clones is freed by the runner when the case
  ends (`DataSource.releasePools`). `Pool<T>` (sequential; bound to
  its creating handle, unsynchronised) and `ConcurrentPool<T>` (one monitor
  across the engine call and the map update; `add(tc, value)` draws through
  the calling rule's handle) track previously generated values over the
  engine's pool primitives so rules can reuse or consume them.
  `LiveDataSource` maps `E_CONCURRENT_USE` to a `HegelException` naming the
  fix (draw through the rule's own `TestCase`, use `ConcurrentPool`).
  `FakeLibhegel.concurrentRounds` scripts per-worker rule queues so the
  driver's join-point layout and every resolution path are tested
  deterministically (`ConcurrentStatefulDriverTest`); `ConcurrentStatefulTest`
  covers the real engine (parallelism, group exclusion, a found lost-update
  race, pools, blobs).
- **Derivation** — `dev.hegel.generators.Derive` + `RecordGenerator` build
  generators from records, enums, scalars, and generic
  `List`/`Set`/`Optional`/`Map` by reflection.

## Rules

- **Coverage is 100%** instruction and branch coverage, enforced per module by
  JaCoCo in `mvn verify`. Each backend jar carries its own binding-edge tests
  (`RealLibhegelTest` + `FfmCoverageTest` for FFM, `JnaLibhegelTest` for JNA)
  on top of the shared suites. A few genuinely-unreachable defensive blocks
  are excluded via `@Generated` (JaCoCo ignores `*Generated*`-named
  annotations): the `NoSuchAlgorithmException` for SHA-256, the lookup of the
  output-callback bridge method, and the `IllegalAccessException` after
  `setAccessible(true)` succeeded in `Stateful`. Adding one is the Java
  equivalent of `// nocov` and needs the same explicit human permission.
  Everything else is covered by real-engine integration tests plus
  `FakeLibhegel`-driven error-path tests. `scripts/uncovered.py` lists the
  lines a failed gate left uncovered (CI runs it on failure).
- **Never mock the engine for behaviour.** The behaviour suite runs against
  the real libhegel through both backends; `FakeLibhegel` exists only for the
  result-code and error paths the engine cannot be driven into.
- **Engine divergences are fixed in the engine.** When libhegel misbehaves,
  fix it in `../../libhegel` (with a `libhegel/RELEASE.md`) in the same PR
  rather than working around it in the bindings.
- **Every PR that changes what ships needs a `RELEASE.md` here.** That means
  any change to `shared/src/main/`, a module's `src/main/` or a `pom.xml`. See
  the root `CONTRIBUTING.md` for the format and the semver rules (on 0.x,
  breaking changes are `minor`, everything else `patch`). CI checks this
  (`.github/scripts/release.py check`); on merge to main the release job bumps
  the version in the parent `pom.xml` and each module pom's `<parent>` and the
  README's install snippet (which must name the current version, or the
  release fails), moves the text into `CHANGELOG.md`, tags `frontends/java/<version>`, and the
  `release-java` job publishes the three artifacts to Maven Central
  (`.github/scripts/publish_java.py`: `mvn -P release deploy`, signed with
  the Hegel release key, bundling the libhegel GitHub release's binaries). A
  libhegel release re-releases this frontend too, with an auto-generated
  engine-bump entry unless the PR writes a `RELEASE.md` here. Never edit
  `CHANGELOG.md` by hand.

## Conventions

- Formatting is palantir-java-format through spotless; `just java::format`
  before committing.
- Both frontend modules' tests run with `--enable-native-access=ALL-UNNAMED`
  (the `hegel.argLine` property — FFM needs it on 22+, JNA on 24+ under JEP
  472; the flag is accepted on every supported JDK).
- JaCoCo gotcha: a call to a helper that *always* throws leaves the call
  site's own instructions uncovered (they are attributed to the next probe,
  which never runs), and `throw helper(x)` leaves a dead `athrow`. Make such
  helpers return on some tested path (see `RunReport.unchecked`). Also note
  the test suite lives in `dev.hegel`, so `Runner.originOf` treats test frames
  as infrastructure — origin assertions cannot name a test file.
- The `ConcurrentCounterExample` demonstration is skipped unless
  `-Dhegel.examples=true` is passed; see the README for how to run it.
