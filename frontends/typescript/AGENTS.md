# TypeScript frontend

Hegel for TypeScript: the `@hegeldev/hegel` package on npm. The client drives
libhegel, the engine in `../../libhegel`, directly through its C ABI
(`../../libhegel/include/hegel.h`). On Node, Bun and Deno that is the native
shared library via the `koffi` FFI library; in browsers it is the same engine
compiled to `wasm32-unknown-unknown`, driven through the raw Wasm exports.
There is no subprocess and no wire protocol: the engine runs inside libhegel,
and the client calls its functions synchronously. Both runtimes share one
runner, one schema interpreter and one public API. Repo-wide conventions are
in the root `AGENTS.md`.

## Commands

```bash
just typescript::setup            # npm ci
just typescript::build-libhegel   # cargo-build the engine natively and for wasm32-unknown-unknown
just typescript::check-test       # vitest with coverage against both builds (fails if < 100%)
just typescript::check-lint       # prettier --check + libhegel-version check + eslint + tsc + typecheck:portable
just typescript::format           # prettier --write
just typescript::check-docs       # typedoc (treatWarningsAsErrors)
just typescript::check-packaging  # node --test scripts/packaging.test.mjs
just typescript::check-browser    # pack the package, bundle it four ways, run in Chromium (`setup-browser` first)
just typescript::check            # lint + docs + test + packaging — what CI runs, less the browser and smoke jobs
```

- The test recipes build libhegel first (incremental, so cheap once built);
  the Wasm build needs `rustup target add wasm32-unknown-unknown`. Plain
  `npx vitest run tests/integers.test.ts` (or `-t "test name"`) works after
  `just typescript::build-libhegel`: `vitest.config.ts` points
  `HEGEL_LIBHEGEL_PATH` at the native build. Omit `--coverage` for a single
  file — the 100% threshold only holds for the full suite.
- `scripts/engine.mjs` is the one place that knows where the builds are
  (`target/release/libhegel_c.{dylib,so}` or `hegel_c.dll`, and
  `target/wasm32-unknown-unknown/release/hegel_c.wasm`, under the workspace's
  `target/` or `CARGO_TARGET_DIR`); `HEGEL_LIBHEGEL_PATH` and `HEGEL_WASM_PATH`
  override them.
- The smoke tests in `tests/smoke/` (Node 20, Bun, Deno in CI) run against the
  built `dist/`: `just typescript::build`, then `npm install` in `tests/smoke`
  and `HEGEL_LIBHEGEL_PATH=$(node scripts/engine.mjs native) node <file>`.

## Engine distribution

Each platform's native libhegel ships as its own npm package
(`@hegeldev/hegel-<os>-<arch>`), exact-pinned in the published package's
`optionalDependencies`; the `os`/`cpu` fields in each manifest make package
managers install only the host's package. The pins are **not committed**:
`npm ci` rejects a manifest whose optional dependencies do not exist on the
registry yet, so `scripts/make-platform-packages.mjs --pin` injects them in the
release job just before `npm publish` (`.github/scripts/publish_typescript.py`,
run by ci.yml's `release-typescript` job from the `frontends/typescript/<version>`
tag). The release job fetches the five libraries from the GitHub release of
the libhegel version in `libhegel/Cargo.toml` (`scripts/fetch-libhegel.mjs`,
sha256-verified); `--local` packages the in-tree build for the host alone,
which is what the browser test installs.

Runtime resolution (`src/locate.ts`, synchronous by necessity):
`HEGEL_LIBHEGEL_PATH` → the platform package's `./binary` export.

The Wasm module (`libhegel-wasm32-unknown-unknown.wasm`) ships inside the main
package at `dist/browser/`, next to the browser entry: `npm run build` is
`tsc && node scripts/engine.mjs wasm dist/browser`, and `prepack` runs it.
`src/browser/index.ts` loads it with a static
`new URL("./libhegel-wasm32-unknown-unknown.wasm", import.meta.url)` so Vite
and webpack emit it as an asset; esbuild and Rollup users copy the
`./libhegel.wasm` export beside their bundle (`BROWSER.md` documents this).
The root export map is ordered `types` → `node` → `browser` → `default`, so
Node wins whenever it is enabled and browser-oriented tools cannot fall back to
the koffi entry; `scripts/packaging.test.mjs` pins that ordering.

`src/libhegel-version.ts` is generated from `libhegel/Cargo.toml`
(`just typescript::update-libhegel-version`; `check-lint` fails when it is
stale) and is what the runtime version checks compare the loaded engine
against. The release script rewrites it on a libhegel release. Never edit it
by hand.

## Architecture

Layers, each building on the previous. Everything above the engine adapters is
shared by Node and the browser; `tsconfig.portable.json` type-checks exactly
that shared set plus `src/browser/` against `ES2022 + DOM` with no Node types,
so a Node import creeping into a shared module fails `check-lint`.

1. **Engine interface** (`src/engine.ts`) — `Engine` is the runtime-independent
   view of the C ABI: one method per ABI operation, opaque branded handle types
   (`ContextHandle`, `TestCaseHandle`, …) so pointers from different adapters
   cannot be mixed, the `Status`/`RunStatus`/`NativeVerbosity` enums copied
   from `hegel.h`, and `EngineError` for adapter faults that must never reach
   the shrinker.
2. **Native adapter** (`src/locate.ts`, `src/libhegel.ts`, `src/session.ts`) —
   `locate.ts` resolves the shared library as described above; `Libhegel`
   wraps `koffi` bindings and implements `Engine`. Fallible calls return
   `hegel_result_t` codes that `Libhegel.check` maps to exceptions:
   `HEGEL_E_STOP_TEST` → `StopTestError` (choice budget exhausted; unwind and
   `mark_complete` with `OVERRUN`), `HEGEL_E_ASSUME` → `AssumeError` (engine
   rejected a draw; `mark_complete` with `INVALID`), any other non-OK code →
   `LibhegelError` carrying `hegel_context_last_error`. Strings cross as
   `hegel_string_t` views (`{data, len}`, so they may contain NUL).
   `session.ts` holds the process-global, lazily-loaded `Libhegel` with a
   `major.minor` version check.
3. **Wasm adapter** (`src/browser/`) — `abi.ts` is the table of raw exported
   function signatures (checked at instantiation, so a drifted module fails
   loudly), `arena.ts` provides bounds-checked memory views, `hegel_string_t`
   marshalling and `WasmArena.scoped` for temporary allocations released after
   each call, `host.ts` supplies the two `hegel_host` imports (`entropy_fill`
   from Web Crypto, `monotonic_nanos` from `performance.now()`), `engine.ts`
   implements `Engine` over those (copying engine-owned bytes and strings out
   before the matching destructor runs), and `load-wasm.ts`
   compiles/instantiates from a URL, `Response`, `ArrayBuffer` or `Module`
   (streaming when the MIME type allows, byte fallback otherwise) and checks
   the engine's version string against `artifact.ts`.
4. **Schema interpreter** (`src/generate.ts`) — the ABI exposes one typed
   entry point per primitive draw (`hegel_generate_integer`,
   `hegel_generate_string`, …); this module walks the generators' schema IR
   and drives those calls through `Engine`, handling compound structure
   (`one_of`, `tuple`, `list`, `dict`, `constant`) client-side with the
   matching shrinker spans and the collection protocol. String-shaped draws go
   through a per-engine `StringGeneratorCache` of `hegel_string_generator_t`
   handles, bounded and freed with the runner. The same run-owned cache
   compiles each `fromLark` generator's grammar once (the builder loop, with
   imported files read through `RuntimeServices.readGrammarFile`) and frees
   the `hegel_grammar_t` handles when the run ends; grammars are never evicted,
   because a draw in progress borrows its grammar. A grammar draw
   (`EngineDataSource.drawGrammar`) is one `hegel_grammar_draw_t` loop per
   value, the frontend drawing each explicit terminal's value between
   `hegel_grammar_draw_next` and `hegel_grammar_draw_provide`. Text and binary
   draws without a `maxSize` pass `HEGEL_MAX_SIZE_DEFAULT`, and `filter` tries
   `HEGEL_FILTER_ATTEMPTS` values: both constants live in `src/engine.ts`.
5. **Runtime services** (`src/runtime.ts`, `src/nodeRuntime.ts`,
   `src/browser/runtime.ts`) — `RuntimeServices` is the small set of
   host-specific things the shared loop needs: how to get the `Engine`,
   settings validation (the browser rejects `Database.fromPath`), whether the
   host can keep a database (the browser cannot, so its runs disable it), how
   to read a grammar file a `fromLark` grammar imports (Node only), and
   where report lines go (`output`: `console.error` on every host, so Node and
   browsers print the same report). Settings defaults, CI detection and Antithesis
   reporting all belong to the engine: a run starts from
   `hegel_settings_new` (or `_for_profile`) and `src/settings.ts` sets only
   the fields the user gave; the test's location and database key come from
   its call site (`src/callSite.ts`) through `hegel_settings_set_test_location`
   and `hegel_settings_set_database_key`.
6. **Shared runner** (`src/runnerCore.ts`) — the sync and async run loops:
   explicit examples (`src/explicit.ts`), then `run_start` (or
   `run_start_blob` for each `reproduceFailure` blob) → `next_test_case` →
   `mark_complete` → `run_result`. The engine owns the whole run, the final
   replay of each failure included; the runner never replays. A test case
   the engine stamps (`hegel_test_case_should_capture`), or every one in a
   verbose run, prints its draws and notes into the engine's printer
   document (`src/pretty.ts` renders values as one-line JavaScript that
   `hegel_printer_reflow` lays out); the runner keeps each failure origin's
   best capture and, on `FAILED`, prints it with the caveat and the
   `.reproduceFailure(...)` line, then rethrows the test's own error (an
   `AggregateError` of them for several distinct failures). An errored run
   throws a `RunError` subclass chosen by `hegel_run_result_error_kind`
   (`src/errors.ts`). `EngineDataSource` implements `DataSource` against an
   engine test case, owning the clones, blocks and printers derived from it.
   Span labels come from the engine (`hegel_label_from_name` /
   `hegel_label_combine`, `src/labels.ts`), never hard-coded numbers.
   `createRunner(runtime)` produces `test`/`testAsync`.
7. **Composition roots** (`src/runner.ts` + `src/index.ts` for Node,
   `src/browser/index.ts` for browsers) — each builds one runner from its
   `RuntimeServices` and re-exports the same public surface. The browser entry
   loads the Wasm module with top-level `await`, so consumers need ES2022 ESM
   tooling; there is no separate init API.
8. **Generators** (`src/generators/`) — transport-agnostic: they build plain
   schema records and draw through the `DataSource` interface. `composite`
   (imperative) and `record` (declarative) in `compose.ts` are the composition
   entry points.

Neither adapter frees anything implicitly — every handle the ABI writes back
is caller-owned: the runner frees `Context`/`Settings`/`Run` handles, each test
case from `next_test_case`, the run result and each failure explicitly in
`finally` blocks, in ownership order, and `EngineDataSource.dispose` frees
each test case's collections, pools, recursions, state machines, printers and
derived test cases; each grammar draw frees its `hegel_grammar_draw_t` when it ends. `src/session.ts` passes `process.versions.unicode` to
`hegel_set_unicode_version` when the native library loads; the browser reports
no Unicode version, so the Wasm engine keeps its own tables. Generated strings decode
through `wtf8.ts` (UTF-8 exactly; safe if the engine ever hands back a lone
surrogate again). The `typescript-ffi` skill (`.claude/skills/typescript-ffi/`)
is the guide to changing the binding layers when `hegel.h` changes.

### Synchronous engine calls

All libhegel calls block — `hegel.test` is a synchronous function.
`hegel.testAsync` only awaits the user's async body between (synchronous)
draws. In the browser this means generation and shrinking run on the calling
thread (the main thread unless the user runs tests in a worker themselves).

### Browser non-goals (for now)

No persistence (`Database.fromPath` throws; every run's database is
disabled), no grammar files (`fromLark`'s `importPath` fails the test), no worker offloading, no Antithesis output (the location is passed,
but the Wasm engine has no output directory), no raw engine-output callbacks. Web
Crypto in a secure context is required, and the Wasm asset must be servable
under the page's CORS/CSP.

## Rules

- **Coverage is 100%** (vitest v8 thresholds plus `scripts/check-coverage.py`).
  No coverage-ignore comments: drive real error paths against the real
  library, and use an injected fake `Bindings` (native) or fake Wasm exports /
  `tests/fakeEngine.ts` (Wasm and shared runner) only for the few
  result-code/NULL/corrupt-output branches the engine can't be driven into.
- **Never mock the engine.** Integration tests run against the real libhegel,
  both the native library (`tests/libPath.ts`) and the real Wasm module
  instantiated in Node (`tests/wasmFixture.ts`); `tests/wasm-*.test.ts` and
  `tests/browser-index.test.ts` cover the Wasm adapter and loader.
- **Engine divergences are fixed in the engine.** When libhegel misbehaves,
  fix it in `../../libhegel` (with a `libhegel/RELEASE.md`) in the same PR
  rather than working around it in the bindings.
- **Every PR that changes what ships needs a `RELEASE.md` here.** That means
  any change to `src/` or `package.json`. See the root `CONTRIBUTING.md` for
  the format and the semver rules (on 0.x, breaking changes are `minor`,
  everything else `patch`). CI checks this (`.github/scripts/release.py
check`); on merge to main the release job bumps the version in
  `package.json` and `package-lock.json`, moves the text into `CHANGELOG.md`,
  tags `frontends/typescript/<version>`, and the `release-typescript` job
  publishes the five platform packages and then `@hegeldev/hegel` to npm and
  pushes the typedoc output to hegel.dev/typescript. A libhegel release
  re-releases this frontend too, with an auto-generated engine-bump entry
  unless the PR writes a `RELEASE.md` here. Never edit `CHANGELOG.md` by hand.

## Conventions

- ESM with `"module": "Node16"`: imports need `.js` extensions, and type
  re-exports need `export type { Foo }`.
- Prettier config lives in `package.json` (`"prettier"` key); there is no
  `.prettierrc`.
- TypeDoc runs with `treatWarningsAsErrors` — a `{@link Foo}` to a
  non-exported symbol fails `check-docs`. Export it or drop the link.
- Node >= 20.11 required (koffi's loader needs `import.meta.dirname`).
- `tests/browser/` (with its own lockfile) and `tests/smoke/` are separate npm
  packages; keep tooling that only they need out of the root `package.json`.
- Vitest ESM note: `vi.spyOn(fs, ...)` throws on frozen ESM namespaces — use a
  hoisted `vi.mock("node:fs", ...)` factory and `vi.mocked(...)` instead.
- `scripts/` and `tests/browser/` are plain Node scripts outside eslint's
  scope; prettier still formats them.
