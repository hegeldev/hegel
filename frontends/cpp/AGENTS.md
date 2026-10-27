# C++ frontend

Hegel for C++: the `hegel` CMake library. It drives libhegel, the engine in
`../../libhegel`, **in-process** through its C ABI (`../../libhegel/include/hegel.h`)
to generate random test data and perform shrinking. There is no server
process and no download: `cmake/libhegel.cmake` builds the in-tree engine
with `cargo build --release -p hegeltest-c` and links the shared library
from the Cargo workspace's `target/` directory. Repo-wide conventions are in
the root `AGENTS.md`.

## Commands

```bash
just cpp::build-libhegel   # cargo-build the engine (the CMake build also does this itself)
just cpp::test             # alias for check-tests: cmake -B build && cmake --build build && ctest --test-dir build/tests (twice: with and without the throw-site hook)
just cpp::check            # check-lint check-tests check-docs check-coverage (the required CI checks)
just cpp::format           # clang-format (through uvx) on all .cpp/.h files
just cpp::docs             # build the Doxygen documentation and open it
```

Run a single test:
```bash
cd frontends/cpp && cmake -B build && cmake --build build
ctest --test-dir build/tests -R test_name
```

Tools: CMake 3.14+, a C++20 compiler, a Rust toolchain (for the engine),
`uv` (clang-format runs as `uvx clang-format`), `clang-tidy` (set
`CLANG_TIDY` to its path; Homebrew's `/opt/homebrew/opt/llvm/bin/clang-tidy`
is found automatically), `doxygen` 1.10+ with graphviz for the docs, and
`llvm-profdata`/`llvm-cov` matching the compiler for coverage (`xcrun` finds
Apple's).

Other recipes worth knowing before touching build/CI files:
- `just cpp::check-sanitizers` — separate `address,undefined` and `thread` builds under `build/san-*`; LSan suppressions at `tests/lsan-suppressions.txt`. The `thread` build links a TSan-instrumented libhegel (`just cpp::build-libhegel-tsan`: Linux only, needs the nightly that `just cpp::tsan-toolchain` names, with `rust-src`); off Linux the recipe skips the `thread` build with a message.
- `just cpp::check-tidy` — clang-tidy over `src/`, warnings-as-errors.
- `just cpp::check-consumer MODE` (`subdirectory` / `fetchcontent` / `install` / `tests_on`) and `just cpp::check-consumer-all` — build the library the way a downstream project would, not part of `just cpp::check`. The `fetchcontent` mode clones the monorepo at `HEAD` (`SOURCE_SUBDIR frontends/cpp`), so it only sees committed changes, and links the engine this checkout already built.
- `just cpp::check-cxx17` — builds and installs with `HEGEL_REFLECTION=OFF`, then builds/runs a standalone C++17 consumer.
- `just cpp::check-nix` — builds and runs the Nix consumer project in `tests/nix` with `nix-build` against the flake's package. `nix/flake.nix` exports `lib.mkHegelCppProject` for downstream Nix users and builds the engine from the in-tree libhegel through the root flake's `lib.mkLibhegel`; `nix/default.nix` is its flake-compat entry point. Needs nix; not part of `just cpp::check`.
- CMake options: `HEGEL_BUILD_TESTS`, `HEGEL_BUILD_DOCS` (OFF), `HEGEL_COVERAGE` (OFF), `HEGEL_REFLECTION` (ON), `HEGEL_THROW_SITE` (ON; OFF for statically linked C++ runtimes), plus `HEGEL_SANITIZE`, `HEGEL_LIBHEGEL_LIBRARY` (link a libhegel built elsewhere; cargo is then not needed), `HEGEL_LIBHEGEL_INCLUDE_DIR` (that library's `hegel.h`), and `tests/CMakeLists.txt`'s `HEGEL_APPROVAL_TESTS` (ON).

## VERY IMPORTANT: Comments

- Adhere to ASD-STE100 Simplified Technical English
- After making a change, do not describe what code was there previously and why
the code was changed.
- Do not mention Hypothesis, or any other Hegel library, even when the user prompts
you to port a feature from there.
- Comments should not duplicate the code.
- Adhere to ASD-STE100 Simplified Technical English

## Dependencies

- C++20 compiler by default. The only hard C++20 dependency is reflect-cpp (used by `default_generator`). Configure with `-DHEGEL_REFLECTION=OFF` to drop reflect-cpp and build/consume at C++17 — `default_generator` and automatic struct parsing become unavailable, but everything else works. The feature is gated by the `HEGEL_HAS_REFLECTION` macro (set from the CMake option; see `include/hegel/config.h`). Designated-initializer params (`integers<int>({.min_value = 0})`) then rely on a GCC/Clang C++17 extension.
- CMake 3.14+
- libhegel (Hegel's native engine) — the in-tree `hegeltest-c` crate. `cmake/libhegel.cmake` adds a custom target that runs `cargo build --release -p hegeltest-c` against the workspace `Cargo.toml` (`CARGO_TARGET_DIR` moves the output, as for cargo) and links `target/release/libhegel_c.{dylib,so}` as the imported target `hegel::libhegel`. The C ABI header is `../../libhegel/include/hegel.h`; the wrappers in `src/engine.{h,cpp}` must track it when the ABI changes (a PR changing the ABI changes this frontend in the same PR). `install()` ships the library next to `libhegel.a` (on macOS with its install name rewritten to `@rpath`).
- reflect-cpp v0.22.0 (type-directed generator derivation via reflection). Version pinned in `CMakeLists.txt`.
- Google Test v1.14.0 (for unit tests)
- ApprovalTests.cpp v.10.13.0 (test-only; snapshot tests in `tests/approvals/`). Fetched at configure time. `-DHEGEL_APPROVAL_TESTS=OFF` skips the fetch and the snapshot suites for offline builds. A mismatch writes a sibling `*.received.txt`; accept it by rerunning the test binary with `APPROVAL_TESTS_USE_REPORTER=AutoApproveReporter`.
- Coverage gate (`scripts/check-coverage.py`, run by `just cpp::check-coverage`): every non-excluded line in `src/` and `include/hegel/` must be covered, and lines hidden via `// GCOVR_EXCL_*` markers must not exceed the ratchet in `coverage-ratchet.json`. Only a human raises the ratchet; it auto-tightens when the excluded count drops.
- Releases: a PR touching `src/`, `include/`, `cmake/` or `CMakeLists.txt` needs a `RELEASE.md` in this directory (`RELEASE_TYPE: major|minor|patch` + changelog body, see the `changelog` skill), or `.github/scripts/release.py check` fails the PR. On merge the release job bumps the `project()` version in `CMakeLists.txt`, moves the entry into `CHANGELOG.md`, tags `frontends/cpp/<version>` (the tag is the release: consumers fetch it with `FetchContent` and `SOURCE_SUBDIR frontends/cpp`) and pushes the Doxygen output to hegel.dev/cpp (`.github/scripts/push_cpp_docs.py`). A libhegel release patch-bumps this frontend too.

## Architecture

### Execution Model

The library calls libhegel's C ABI (`hegel_*` functions) directly, in-process — no subprocess, no socket. `hegel::test()` (`src/hegel.cpp`) drives the run:
1. Get this thread's error-reporting context (`impl::thread_context()`, one `hegel_context_t` per thread, mirroring hegel-rust) and a fresh settings handle (`hegel_settings_new`), then call every `hegel_settings_set_*` with the fields of the `Settings`. A `Settings` holds a value for every field: creating one resolves a profile through the engine (`hegel_settings_new`, or `hegel_settings_new_for_profile` for `Settings::from_profile`), which applies profiles, `hegel.toml` and the `HEGEL_*` variables, and copies every value back through `hegel_settings_get_*` (`detail::resolve_settings`, the default member initializer of the hidden first field `resolved_`, which the other fields' initializers copy from). The frontend never sets a field to a default of its own and never detects CI. What the run needs back from the handle (verbosity, `print_blob`, the phases) it reads with `hegel_settings_get_*`.
2. Unless failure blobs are given, the explicit test cases (`hegel::ExplicitTestCase`, from `HEGEL_EXPLICIT_TEST_CASE` or the `explicit_cases` argument) run first when the resolved phases include `HEGEL_PHASE_EXPLICIT`. Each runs the body once on a `TestCase` with no engine handle (`TestCaseData::explicit_state`): `TestCase::draw` takes the case's values (`internal::take_explicit_value`, named first, then positional), target/event do nothing, and every misuse (a rejection, a draw with no value or a value of the wrong type, an unused value, `clone()`, anything else that needs the engine: `impl::DrawScope` checks) throws `internal::UsageError`, a `std::invalid_argument` that `run_body` lets propagate instead of recording a failure. A rejection's message begins "Explicit test cases must not be rejected". The first failing case prints its drawn values and notes and propagates its exception.
3. `hegel_run_start` builds the run; no engine work happens yet. With failure blobs (`HEGEL_REPRODUCE_FAILURE`, or the `failure_blobs` argument), each blob gets a run of its own from `hegel_run_start_blob`, driven by the same loop; a run with no failure means that blob did not reproduce, and if none reproduces `hegel::test()` throws `hegel::DidNotReproduce`.
4. Loop `hegel_next_test_case` until it yields NULL; run the user body for each case and `hegel_mark_complete` it (VALID / INVALID / OVERRUN / INTERESTING). libhegel has no background thread: all engine work (generation, mutation, shrinking, the final replay of every failure it will report) happens on the calling thread inside `hegel_next_test_case`. Notes and drawn values go into the engine's document for the case (`hegel_note`, and `hegel_test_case_printer` with the `hegel_printer_*` functions), and only for a case whose output can be shown: the final replay, a case the engine stamped for capture (`hegel_test_case_should_capture`), or any case at `Verbose`+ (never under `Quiet`); `TestCaseData::emit` holds that decision. After the body, `impl::test_case::rendered_output()` reads the document through the root handle: at `Verbose`+ its lines go to stderr, and every failing case stores them, if stamped, in a capture keyed by the failure's origin. Hegel for C++ keeps no output buffer, indentation or worker prefixes of its own: an indented section is a block handle (`internal::block`, `hegel_test_case_block`), a clone's output lands where the clone was made (and its draws share the parent's draw names, `TestCaseData::draw_names`), and the public `TestCase::set_worker` (`hegel_test_case_set_worker`) makes the engine prefix a worker's lines with `[worker N +X.XXXms]`.
5. `hegel_run_result` reports passed / failed / errored. An errored run throws the `RunError` subclass that `hegel_run_result_error_kind` selects (`FailedHealthCheck`, `Flaky`, `Unsatisfiable`, else `RunError`; see `include/hegel/errors.h` and `impl::raise_run_error`). On failure, each distinct failure is reported from the capture of its origin (the engine's final replay of a failure is a stamped case, so nothing is executed again after the run). The report follows hegel-rust's (`frontends/rust/src/run_lifecycle.rs`): with several failures, a `Property-based test failed with N distinct failures.` line and a blank line before each; then each failure's drawn values and notes, a `note: <caveat>` line when the engine qualifies how reliably the failure reproduced (`hegel_failure_caveat`), and, when `print_blob` is on and the failure has a blob, a blank line and `To reproduce this failure, ...` with the line that replays it. Under `Quiet` only the reproduce lines print. The single failure's own exception propagates; several raise `MultipleFailures`, whose message holds each one's.
6. The reproduce line names the entry point the run came from: `hegel::test(test_fn, settings, {...})`, the `TestLocation` overload's `hegel::test(test_fn, location, settings, {...})`, or, for `internal::test_from_macro` (what `HEGEL_TEST` expands to), `HEGEL_REPRODUCE_FAILURE(name, "...")`.

Hegel runs no tests for you: `HEGEL_TEST` defines a function the user calls from `main()`, and there is no registry. With a test framework, the property goes inside one of its tests and calls `hegel::test()`.

Nondeterminism (a test whose outcome or draws change when the same data is replayed) is the engine's business: under the default `Settings::nondeterminism_strictness` (`Quiet`) it confirms failures by replay and reports each with a caveat; `Warn` adds a notice; `Error` fails the run with the flaky-test error (what the `FlakyReporting` tests ask for). `Settings::stop_on_first_failure = false` makes a run use its whole `test_cases` budget before reporting.

### Test-framework integration

`internal::FrameworkHooks` (declared in `include/hegel/hegel.h`, stored in `src/hegel.cpp`) is how a framework takes part in a run: `current_test_name()` names the test that is running, and `run_case()` wraps each body invocation. `include/hegel/gtest.h` installs the GoogleTest pair from a namespace-scope variable initializer, and `hegel.h` includes it when `GTEST_TEST` is already defined. There is deliberately no opt-out: an assertion Hegel does not see reads as a passing test case.

- `run_case` runs the body under a `ScopedFakeTestPartResultReporter`, so the assertions a case fails do not reach GoogleTest, and raises them together as `hegel::GTestFailure`. Without this, an `ASSERT_*`/`EXPECT_*` failure only records and returns, which Hegel reads as a passing case.
- A failure's origin (what the engine groups bugs by) is the exception's demangled type plus the site it was thrown from, unless the exception also derives from `hegel::FailureOrigin`, in which case `run_body` takes `failure_origin()` instead. `GTestFailure` returns the positions of the assertions the case failed — never their messages, which hold generated values and would make every failing case its own bug.

### Throw sites

A bare `throw` carries no location, and the stack unwinds before `run_body` catches, so `src/hegel.cpp` defines its own `__cxa_throw` and forwards to the real one through `dlsym(RTLD_NEXT, ...)`. It records `__builtin_return_address(0)` in a `thread_local`, which `last_throw_site()` turns into `<binary>+0x<offset from its load address>` via `dladdr`. The offset is what tells two `throw`s apart; it is load-address independent, so the string repeats run to run under ASLR. Symbol names are deliberately not used: ELF `dladdr` reads only the dynamic symbol table, which cannot name a lambda or an internal function. Lookups are cached per thread because shrinking re-runs the failing body many times.

Hegel is a static library, so its definition lands in the test executable and wins for both ELF and Mach-O — provided the C++ runtime is a shared library. A statically linked runtime (e.g. `-static-libstdc++`, or a toolchain with `libc++abi.a`) defines `__cxa_throw` in an object the link always pulls in, and the two definitions collide; `HEGEL_THROW_SITE=OFF` sets `HEGEL_HAS_THROW_SITE=0` to compile the wrapper out for such toolchains. Where `RTLD_NEXT` is missing the whole thing compiles out the same way. Either way origins fall back to the type alone. `tests/subject_main.cpp`'s `throw_sites` scenario checks the grouping from a fresh process, which is what proves it survives ASLR.
- `Settings::database_key` defaults to `"<file>::<name>"`, derived in `run_test()` for `HEGEL_TEST` (its file and name) and `hegel::test()` alike. For `hegel::test()` the file comes from `__builtin_FILE()` in a default argument, so it names the call site; the name is `current_test_name()` (`Suite.Name`) or `__builtin_FUNCTION()`, or the `TestLocation`'s name. The key is set also when the database is disabled: a derandomized run (the `ci` profile) seeds from it, so without it every test would see the same inputs.
- `hegel::test()` builds a `TestLocation` only from `current_test_name()`, never from `__builtin_FUNCTION()`: a function name (often `operator()`) says too little to head a report with. Reports Hegel names itself carry an absolute path and the line of the call, so the snapshot suites scrub the header with `scrub_report()` (`tests/common/approvals.h`); a report named by an explicit `TestLocation` keeps its fixed position and uses `scrub_blob()`.

### Draw path

A `draw()` calls libhegel's typed draw primitives (`hegel_generate_integer`, `hegel_generate_float`, `hegel_generate_boolean`, `hegel_generate_bytes`, `hegel_generate_string`, `hegel_generate_date`/`_time`/`_datetime`, `hegel_generate_ipv4`/`_ipv6`, `hegel_generate_uuid`) directly — there is no schema or serialization layer. The template-visible primitives (`draw_integer`, `draw_integer_unsigned`, `draw_float`, `draw_boolean`, spans, collections) are declared in `include/hegel/internal.h` and implemented in `src/engine.cpp`; the string/bytes/date draws used only by `src/generators.cpp` live in `src/engine.h` (`hegel::impl`).
- Strings cross the ABI as borrowed `hegel_string_t` (`{data, len}`) values built by `engine.cpp`'s `borrow()`; optional ones travel as a nullable pointer to one. Engine-owned strings come back as `hegel_string_t` values (copied out at once) or, for `hegel_generate_string`, an `hegel_owned_string_t` freed with `hegel_owned_string_free`.
- Integer ranges that don't fit in `int64_t` (e.g. `integers<uint64_t>()`) go through `hegel_generate_integer_big` with two's-complement little-endian bound buffers.
- `text` and `binary` with no `max_size` pass `HEGEL_MAX_SIZE_DEFAULT` so the engine applies its default maximum; a `max_size` at or above that sentinel goes as `UINT64_MAX` (no bound).
- String-family generators (`text`, `characters`, `from_regex`, `emails`, `urls`, `domains`) build a validated, immutable `hegel_string_generator_t` handle once at generator construction and draw through it with `hegel_generate_string`.
- `HEGEL_E_STOP_TEST` → `HegelStopTest` (case marked OVERRUN); `HEGEL_E_ASSUME` → `HegelReject` (INVALID); other non-OK codes throw `std::runtime_error` with `hegel_context_last_error`.

### Key Components

Public headers in `include/hegel/`:
- **`hegel.h`** - Main include, declares `hegel::test()` entry point
- **`gtest.h`** - GoogleTest integration (header-only): `hegel::GTestFailure` and the `FrameworkHooks` it installs
- **`test_case.h`** - TestCase class (move-only) with `draw()`, `assume()`, `reject()`, `target()`, `event()`, `event_value()`, `repeat()`, `note()`, `clone()`, `spawn()` (returns a move-only `Worker<T>` for threaded drawing) methods passed to the test callback
- **`core.h`** - `IGenerator<T>` (draws silently) and `IPrintableGenerator<T>` (also `do_draw_and_print(tc, printer)`: draws and prints in one step), the handles `AnyGenerator<T>` (any generator) and `Generator<T>` (a printable one; it derives from `AnyGenerator<T>`), `map()` (returns `Mapped<U, SourcePrintable>`, which derives from the handle its value type allows and has `print_as_call()`), `flat_map()`, `filter()`, the escape hatches `print_with()` and `print_as_repr()`, `make_generator<Impl>()`, and `compose()` to build a generator from an imperative `TestCase`-drawing lambda. `detail::ValuePrinted<T>` (prints the drawn value with `Printer::repr`), `detail::Structural<T, Printable>` (one `draw_printing(tc, printer)` body for the silent and printed draws) and `detail::handle_t<Printable, T>` are what generators build on. `TestCase::draw` takes only a `Generator<T>`: overloads on `AnyGenerator<T>` fail with a `static_assert` that says how to make the generator printable
- **`settings.h`** - `Settings`, `Database`, the `Verbosity`, `HealthCheck`, `Phase`, `Backend` and `NondeterminismStrictness` enums, and `register_profile()`, `set_default_profile()` and `engine_version()`
- **`explicit.h`** - `ExplicitTestCase` and `named()`: values given in advance for the explicit phase
- **`errors.h`** - The exceptions `hegel::test()` throws for a run that ends without a verdict (`RunError`, `FailedHealthCheck`, `Flaky`, `Unsatisfiable`), for blobs that do not reproduce (`DidNotReproduce`), and for several failures (`MultipleFailures`)
- **`internal.h`** - The typed draw primitives (`draw_integer`, `draw_float`, `draw_boolean`, spans, collections), the owning handles over the engine's compound-draw objects (`Collection`, and the `PoolHandle` / `StateMachineHandle` backing `stateful.h`, each of which frees its handle in its destructor), `LabelName` (the `hegel.<kind>` names the generators derive their span labels from; `src/engine.cpp` hashes each with `hegel_label_from_name`, and `combine_labels` wraps `hegel_label_combine`), the output helpers (`NestedDraw`, `records_output`, `test_case_printer`, `draw_display_name`, `block`), and the `HegelReject` / `HegelStopTest` exceptions (internal only; users interact via `TestCase` methods and the macros)
- **`generators/`** - Strategy factory functions in `hegel::generators` namespace, split by category: `primitives.h`, `numeric.h`, `strings.h`, `collections.h`, `combinators.h`, `formats.h`, `builds.h`, `default.h` (type-directed derivation via reflect-cpp), `random.h`, `stateful.h` (model-based/stateful testing — see Stateful testing below)

Private implementation in `src/`:
- **`engine.{h,cpp}`** - Wrappers over the libhegel C ABI: run-lifecycle helpers (`hegel::impl`), string-generator construction, and the draw-primitive implementations
- **`test_case.{h,cpp}`** - Private `TestCaseData` struct (owns the `hegel_test_case_t*` — freed in its destructor — plus per-iteration state; the error-reporting context is per-thread via `impl::thread_context()`) and the `TestCase` method implementations — each outermost `TestCase::draw` (in `core.h`) prints a C++ declaration, `auto <name> = `, then lets the generator print its value with `do_draw_and_print` on the case's engine printer, then `;`, all in one speculative region, when the case records output; a nested draw, or a case that does not record output, draws silently
- **`include/hegel/printer.h` / `repr.h`** - the public `hegel::Printer` (move-only, over a `hegel_printer_t*`, borrowed or owned: text, break points, groups, `seq` in block layout, `reflow`, `if_break`, `comment`, `repr`; `noop()` discards everything and `should_print()` says whether output is kept; `deferred()` makes an owned child region, `hegel_printer_deferred`, filled later: a drawn `HegelRandom` records the values it hands out into one, as hegel-rust's `rand` extra does), `hegel::Speculation` (a speculative region, aborted unless committed: each draw line and each filter or collection attempt is printed in one) and `hegel::Document` (a standalone engine document); `repr.h`'s `print_value` prints a value as a C++ expression (a type with a `print_to(Printer&) const` member prints itself), giving the engine its groups (block style: items indented four columns, the close on its own line) so it can lay a wide value out across lines, and sends a stream operator's text through `hegel_printer_reflow`. `hegel::is_representable_v<T>` says whether a type has a representation. `repr()` renders on one line and `render()` within a width, through a `Document`
- **`generators.cpp` / `hegel.cpp`** - implementations for the corresponding public headers; `hegel.cpp` also holds the `hegel::test()` run loop and the failure report
- **`cmake/libhegel.cmake`** - builds the in-tree libhegel with cargo (or takes `HEGEL_LIBHEGEL_LIBRARY`) and exposes the `hegel::libhegel` imported target

### Generator Pattern

Each generator concept has its own concrete `IGenerator<T>` subclass (`IntegerGenerator<T>`, `VectorsGenerator<T>`, `OneOfGenerator<T>`, `TextGenerator`, …). The subclass stores its configuration (validated in the constructor) and implements `do_draw()`, composing the typed draw primitives the same way hegel-rust's own generator library does.

Printing follows hegel-rust's `PrintableGenerator`: the generator prints while it draws. A leaf (and every generator that only makes a value: `map`, `just`, `sampled_from`, `compose`, pools) derives from `detail::ValuePrinted<T>` or `detail::value_base_t<T, Printable>` and prints the finished value with `Printer::repr`. A structural combinator (collections, `tuples`, `optional`, `one_of`, `variant`, `filter`, `flat_map`, `recursive`, `deferred`, `builds`, derived structs, `randoms`) derives from `detail::Structural<T, Printable>` and prints its components through their own `do_draw_and_print`, so a component's custom printing shows inside it. Collections print each element in a speculative region and abort it for a rejected duplicate; sets and maps print in draw order. A combinator is printable exactly when its components are: each factory has an overload on `Generator<T>` that returns a `Generator`, and one on `AnyGenerator<T>` that returns an `AnyGenerator`. Every handle whose printability is computed (`Mapped`, `DerivedGenerator`, `deferred`'s handle) derives from one of the two, so template deduction sees it.

- **Primitives** map 1:1 onto a draw call (`integers` → `draw_integer`/`draw_integer_unsigned`, `floats` → `draw_float`, `booleans` → `draw_boolean(0.5)`). `just()` draws nothing and returns its captured value.
- **Collections** (`vectors`, `sets`, `maps`) open their span (labelled `hegel.lists`/`hegel.sets`/`hegel.dicts` combined with their components' labels), create an engine-managed collection (`Collection`), and loop over its `more()`, drawing one element per iteration — the engine owns the size logic. Duplicates (sets, maps, `unique` vectors) are pushed back with `reject()` so the engine draws a replacement.
- **Branching** (`one_of`, `variant`) opens a `hegel.one_of` span and draws a branch index; `optional` opens a `hegel.optional` span and gates on `draw_boolean(0.5)`; `sampled_from` is a bare index draw with no span.
- **Combinators**: `map` wraps the source draw in a `hegel.map` span; `flat_map` in a `hegel.flat_map` span; `compose` in a `hegel.composite` span; `filter` calls `IGenerator::do_draw_filtered`, which retries up to `internal::filter_attempts` times (the engine's `HEGEL_FILTER_ATTEMPTS`; a `static_assert` in `src/engine.cpp` keeps the two equal), each attempt inside a `hegel.filter.attempt` span, closing it with `discard=true` on predicate failure, then rejects the test case. A consuming pool draw overrides `do_draw_filtered` to put back (`hegel_pool_restore`) the variables its rejected attempts consumed.
- **Recursion**: `recursive(leaf, branch)` drives a `hegel_new_recursion` scope (`internal::RecursionHandle`). The root and every `SubtreeGenerator` ask `hegel_recursion_branch` whether to branch; a leaf first counts against the budget (`hegel_recursion_leaf`, whose `HEGEL_E_RETRY` throws `LeafBudgetExceeded`), and the root ends with `hegel_recursion_finish` (`HEGEL_E_RETRY` throws `AttemptMispriced`). `RecursiveGenerator` catches both and starts again from the root, calling `hegel_recursion_retry` after an over-budget attempt.
- **Grammars**: `from_lark` compiles its grammar once, at construction (`impl::grammar_new`: a `hegel_grammar_builder_t` fed each `%import`ed file from `LarkParams::import_path`, NULL for a missing one so the engine's bundled grammars stand in, then `hegel_grammar_new`). Each draw is a `hegel_grammar_draw_t` (`impl::draw_grammar`): the engine says which explicit terminal needs a value, the frontend draws it from that terminal's generator and hands it over, and the engine splices it in, keeps the string under `max_size`, and redraws or rejects. The frontend does no splicing or size accounting of its own.
- **Wide integers**: an integral type wider than 64 bits (`__int128`) draws through `hegel_generate_integer_big` (`internal::draw_integer_bytes`). `floats<float>` draws at width 32; every other floating type draws at width 64.

Every generator has a `label()`: a leaf's is `internal::label(LabelName::<Kind>)`, and a compound generator computes `combine_labels({its own, its components'...})` once, at construction, and opens its span with it. The names match the other frontends' (`hegel.lists`, `hegel.one_of`, ...), so the same generator shape has the same label in every language.

Spans exist so the shrinker can reason about a group of draws as a unit; open one span per compound draw and close it exactly once (rust-style: leave it open if an exception unwinds — the runner marks the case complete anyway).

### Stateful testing

`hegel::stateful` (`include/hegel/generators/stateful.h`, header-only, no `src/stateful.cpp`) is model-based state-machine testing, built entirely from the primitives above rather than a separate execution path. A user derives `StateMachine<Derived, State>` (CRTP) holding a `State` member, returning `std::vector<Rule<Derived>>` from `rules()` and (optionally) `std::vector<Invariant<Derived>>` from `invariants()`. A `Rule<T>` pairs a name with a step that draws from the `TestCase` and mutates the machine; an `Invariant<T>` is a named predicate that must throw to signal violation, checked before the first step and after every step that completes without an `assume()` rejection. `Pool<T>` lets one rule's output feed another rule's input as an ordinary `tc.draw(...)` (via `values_consumed`/`values_reusable` wrapping the pool in a `VariablesGenerator<T>`); it is non-copyable/non-movable and keeps a client-side map in sync with the engine's own pool state, throwing if they diverge.

`stateful::run()` opens a `hegel.stateful.rule` span around each rule application so the shrinker can delete or reorder a whole step as a unit, and draws which rule fires via `hegel_state_machine_next_rule`. The engine owns the stepping lifecycle: each `next_rule` call first makes the stop decision itself, and writes `HEGEL_STATE_MACHINE_DONE` (mirrored as `internal::state_machine_done`) when the sequence ends — the loop breaks there, leaving the final span open for the engine to freeze. Every case runs at least one step and at most `Settings::stateful_step_count` (default 50; `hegel::test()` rejects values below 1 with `std::invalid_argument`, and the value travels on `TestCaseData` to the `step_count` argument of `hegel_new_state_machine`). Replay (`Mode::SingleTestCase`) is unbounded engine-side: a failing sequence reruns for real until a rule throws or an invariant fails, not until the step budget expires. A step that hits `assume()` closes its span with `discard=true` and reports itself through `hegel_state_machine_rule_rejected`, so it does not count against the step budget.

`run_concurrent()` clones the test case once per worker for each round (`hegel_test_case_clone`) on the main thread, after the round's header note and before it wakes the workers, so each worker's output lands under that header; a clone can fail (the engine stops an overrun case at the clone), and the failure stops and joins the workers before it propagates. Each rule body draws through a block (`internal::block`) so its output is indented under its `Rule:` line.

### Test Suite Organization

`tests/CMakeLists.txt` defines `hegel_add_test(NAME ... SOURCE ... [USE_UTILS] [INTERNAL] [APPROVALS])`, wrapping `gtest_discover_tests`; `APPROVALS` swaps `gtest_main` for `tests/common/approval_main.cpp` + `ApprovalTests::ApprovalTests`, and stores snapshots under `tests/approvals/`. `find_quality/test_*.cpp` and `shrink_quality/test_*.cpp` (one file per generator family) are ordinary GTest cases using `tests/common/utils.h`'s `find_any()`/`minimal()` helpers, which run `hegel::test()` against a predicate and assert the engine reaches (find_quality) or shrinks to (shrink_quality) a known value. `property_tests.cpp` holds worked-example properties, distinct from `test_generators_*.cpp`, which unit-test generator construction/validation one family per file. `subject_main.cpp` builds a standalone `subject` binary with argv-selected failure scenarios; `test_printing.cpp` forks it via `tests/common/subprocess.h` and approves its stderr through `tests/common/approvals.h`'s scrubbers, because failure-report formatting (throw-site stability under ASLR, multi-failure grouping) must be observed from a fresh process, not in-process. `tests/consumer/{subdirectory,fetchcontent,install,tests_on,cxx17}/` each exercise one way of consuming the library as a dependency, driven by `just cpp::check-consumer`/`check-consumer-all`.

## Code Style

- **Formatting**: LLVM base style, 4-space indentation, left-aligned pointers (`int*`). Run `just cpp::format` before committing.
- **Headers**: Use `.h` extension (not `.hpp`)
- **Namespaces**: `hegel` for public API (including run configuration types like `Settings`, `Database`, `Verbosity`), `hegel::generators` for generators and strategies, `hegel::internal` for internals referenced in public headers, `hegel::impl::*` for purely private implementation
- **Includes**: Public headers use relative includes (`#include "settings.h"`), source files use angle brackets for both public (`<hegel/internal.h>`) and private (`<engine.h>`, `<test_case.h>`) headers
- **File organization**: Each focused `.cpp` has a corresponding `.h` in `src/`. Private headers live in `src/`, not `include/`
- **Public API surface**: Minimal. Only what users need goes in `include/hegel/`. Internal details hidden via `@cond INTERNAL` / `@endcond` in Doxygen
- **Parameter structs**: Designated initializers (C++20): `integers<int>({.min_value = 0})`
- **Designated initializer order**: Always list designators in declaration order. Out-of-order designators are a hard error under GCC (Clang only accepts them as an extension), so a reordering that builds locally on Clang still breaks the GCC CI jobs.
- **Self-contained**: Prefer small standalone implementations over adding heavy dependencies

### Error Handling: `TestCase::assume()` vs exceptions

`tc.assume(condition)` (a method on the `TestCase` passed to a test callback) is **only** for filtering generated test data that doesn't meet preconditions. It signals to the framework that the current test case should be silently discarded (via `HegelReject`), not counted as a failure. It must never be used to handle errors in the library implementation itself.

**Correct use** - filtering generated values in tests:
```cpp
auto x = tc.draw(integers<int>());
tc.assume(x != std::numeric_limits<int32_t>::min());  // Skip edge case
```

**Wrong use** - masking engine errors:
```cpp
// BAD: silently swallows an engine error as if it were bad test data
tc.assume(rc == HEGEL_OK);

// GOOD: surface the error so it can be diagnosed and fixed
if (rc != HEGEL_OK) {
    throw std::runtime_error("hegel_generate_integer failed: " + last_error(ctx));
}
```

Rules of thumb:
- libhegel returned an unexpected error code? Throw `std::runtime_error`.
- Caller passed invalid arguments (e.g. empty vector)? Throw `std::invalid_argument`.
- Generated test data doesn't meet a precondition? Use `tc.assume()`.
