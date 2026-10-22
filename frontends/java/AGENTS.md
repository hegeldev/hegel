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
  public `Libhegel` interface (one method per `hegel_*` function; raw `long`
  handles and return codes), `Abi` constants, `LibhegelException`, the
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
date/time/datetime/uuid/ip draws, …). Every fallible call takes a per-thread
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
  `lastErrorMessage()`), pass date/time/datetime structs by value, pass every
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
  opaque `u64`s derived from `dev.hegel.<kind>` names with the same FNV-1a hash
  as the engine's `hegel_label_from_name` (`Label.of`) and
  `hegel_label_combine` (`Label.combine`); the engine has no predefined label
  enum. The engine's `hegel_set_unicode_version`, `stop_on_first_failure`
  setting and `hegel_run_result_error_kind` have no Java surface yet.
- **Per-case primitives** — `DataSource` is the abstraction generators draw
  against; `LiveDataSource` wraps the engine, translating return codes
  (`StopTest` → OVERRUN, `AssumeRejected` → INVALID, `INVALID_ARG` →
  `IllegalArgumentException` with the engine's diagnostic, other negatives →
  `HegelException`) and short-circuiting once a case is aborted.
- **Run loop** — `Runner` builds the settings handle and drives
  `hegel_run_start` → `hegel_next_test_case` → `hegel_mark_complete`. The
  engine owns the *whole* run — generation, shrinking, the replays that confirm
  a failure, and the final replay of every failure it is about to report — so
  the runner never replays anything itself. It reads
  `hegel_test_case_should_capture` once per case: a *stamped* case's `TestCase`
  records its top-level draws and notes (`isFinal()` exposes the stamp), and
  every interesting case's exception is kept per origin in a capture map (a
  stamped capture outranks an unstamped one, newest wins at equal rank — the
  unstamped discovery of a failure that never fails again is all there is for
  an *unconfirmed* nondeterministic failure). After the loop drains,
  `hegel_run_result` yields PASSED, FAILED, or ERROR. On FAILED each distinct
  failure is built from its origin's capture: the recorded draws/notes are
  replayed to the reporter (`caseStarted(true)` … `caseFinished(true)`), and
  the `Failure` carries the body's own exception, the reproduce blob
  (`hegel_failure_reproduction_blob`, null for an unconfirmed failure and for a
  blob replay) and the **caveat** (`hegel_failure_caveat`, non-null for a
  failure the run saw fail to reproduce — `Failure.nondeterministic()`; the
  printing reporter prints it as `note: …`). A failure whose origin has no
  capture is an internal error. `Settings.reproduceFailure` drives the same
  loop over `hegel_run_start_blob` (the engine replays until a replay fails,
  under its budget): PASSED → `Runner.STALE_BLOB` `HegelException`, ERROR →
  "blob replay ended in an error" `HegelException`; a blob the engine cannot
  decode fails `hegel_run_start_blob` itself and surfaces as the binding's
  `HegelException` with the engine's "could not be decoded" message. There is
  no Java-side flaky detection: the engine handles nondeterminism per
  `nondeterminismStrictness` (`NondeterminismStrictness.QUIET`/`WARN`
  confirm-by-replay and report with a caveat; `ERROR` aborts the run,
  surfacing as the engine's own `Flaky test detected` message). The runner
  returns a `RunReport` (status, client-side `RunStatistics` counted per
  `mark_complete` — the C ABI has no counter accessor — engine error,
  failures) and never prints: everything goes through the run's `Reporter`
  (`Reporter.printing(System.err)` is the default and reproduces the classic
  output; `Reporter.silent()` for library use). `Hegel.run` returns the
  report; `Hegel.test` = `run` + `RunReport.throwIfFailed()`, which rethrows a
  single failure as-is (checked exceptions included), aggregates several into
  an `AssertionError`, and maps ERROR to `HealthCheckFailure`/`HegelException`.
  `HegelException` (binding/engine errors) is always thrown, never reported.
  `Settings` is the immutable config; its closed-state setting types live
  alongside it — `Backend` (auto = leave it to the engine's profile, which
  picks urandom inside Antithesis / default / urandom), `Database`,
  `OptBoolean`, `NondeterminismStrictness` (`DEFAULT` = leave it to the
  engine) — plus `printBlob` (print a copy-pasteable reproducer per failure)
  and `reproduceFailure` (replay a stored blob instead of running the
  property). `testCases`, `printBlob`, `derandomize`, `seed`,
  `nondeterminismStrictness` and `database` are nullable ("unset"):
  `Runner.applySettings` sends only what the user set, so the engine's
  resolution — its profile (`hegel.toml`, the shipped
  `development`/`ci`/`workload` profiles) with the
  `HEGEL_TEST_CASES`/`HEGEL_DATABASE`/`HEGEL_SEED`/`HEGEL_DERANDOMIZE`/
  `HEGEL_PRINT_BLOB`/`HEGEL_NONDETERMINISM_STRICTNESS` variables applied over
  it in `hegel_settings_new` — stands for the rest, and explicit Java settings
  win over both. There is no Java-side CI detection (the engine's `ci` profile
  covers it). `Libhegel.settingsNew` returns a raw result code because a
  malformed variable or `hegel.toml` fails it with `E_INVALID_ARG` (→
  `IllegalArgumentException`); after applying, the runner reads
  `settingsGetTestCases`/`settingsGetPrintBlob`/`settingsGetNondeterminismStrictness`
  back and hands the *effective* `Settings` to the reporter and the run.
  Verbosity and `reportMultipleFailures` keep Java-side defaults and are always
  sent. `EnvironmentTest` covers the variables and `hegel.toml` through a child
  JVM (`EnvironmentFixture`), since the engine reads its own process
  environment. There is no single-test-case mode; `testCases(1)` is the
  one-case configuration.
- **Generators** — `Generator<T>` (public) with `map`/`filter`/`flatMap`. Leaf
  generators call the typed draw bridges on `TestCase` (`generateInteger`,
  `generateFloat`, `generateString`, …); composite generators (collections via
  the engine's `new_collection`/`collection_more` protocol, `oneOf`, tuples,
  `map`/`filter`/`flatMap`) compose other generators' `doDraw` inside labeled
  spans, mirroring the Rust frontend. String-shaped generators
  (`text`/`characters`/regex/email/url/domain) build a validated
  `hegel_string_generator_t` handle once per configuration and cache it
  (`HandleCache`, rebuilt if a test swaps the `Engine` binding);
  `StringGeneratorHandle` frees the engine allocation via a `Cleaner` when
  unreachable. `Generators` is the factory facade.
- **Public API** — `Hegel.test`/`Hegel.run`, `TestCase`
  (`draw`/`assume`/`note`/`target`/`isFinal`/`span`), `Label` (span labels;
  `Label.of(name)` mints FNV-1a labels), `Reporter`, `RunReport`,
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
    per round (discarded on rejection), `Step N: rule` notes.
  - `maxConcurrency > 1` → the **concurrent** driver, modelled on the Rust
    frontend's `run_concurrent_machine` (even when the drawn level is 1, so a
    run's output format is uniform). Persistent daemon worker threads
    (`hegel-worker-N`) live for the test case; per round the root thread notes
    the header (`---------------- Round k: group "name" ----------------`),
    makes a **fresh clone per worker** (`hegel_test_case_clone` +
    `hegel_test_case_set_worker`; cloning is a draw and can return
    `E_STOP_TEST` on a replay, hence the raw-rc convention for
    `Libhegel.testCaseClone`), hands each worker its clone through a queue,
    waits for one event per worker (the join point), absorbs the workers'
    buffered draws/notes into the root in worker order (each line stamped
    `[worker N +X.XXXms]` client-side — Java notes never go through
    `hegel_note`, so the engine's own attribution is invisible here; draw
    names stay unique family-wide through the root's synchronized name counter
    and are recorded plain), frees the clones, and resolves the round in the
    Rust frontend's precedence: a control error
    (`LibhegelException`/`IllegalArgumentException`, or a worker that exited
    without reporting) is rethrown first; then an overrun or an engine-level
    `E_ASSUME` concludes the case, dropping any failures found alongside with
    a `Dropped concurrent failure from worker N` note; otherwise the lowest
    worker's failure is rethrown as-is (its stack trace is the worker's, so
    `originOf` still works) and the rest are noted as dropped. There is no
    cancellation: a failing worker ends its own round and the others finish
    theirs (an overrun aborts the family engine-side anyway). No spans are
    opened in the concurrent path (none of the bindings do; the engine owns
    rule structure). The `Reporter` is only ever called from the driving
    thread: verbose output of worker lines is delivered at the join. Workers
    are stopped and joined in a `finally` before any leftover clone is freed.
  The machine handle is freed in a `finally`. `Pool<T>` (sequential; bound to
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
  `FakeLibhegel`-driven error-path tests.
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
  the version in the parent `pom.xml` and each module pom's `<parent>`, moves
  the text into `CHANGELOG.md`, tags `frontends/java/<version>`, and the
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
