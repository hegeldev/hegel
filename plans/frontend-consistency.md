# Frontend consistency plan

Source: the cross-frontend review of 2026-10-24. The per-frontend inventories it is based on cite file:line, but they were written by reading code and may be wrong in places: **verify each item against the code before changing anything**, and if an item turns out to be already done or mistaken, record that instead of changing code.

## Principles

1. **Frontends are thin drivers of libhegel.** A frontend that does natively what the C ABI provides is a defect. Use the ABI. If engine logic lives in a frontend and the ABI has no function for it, the fix is to add one to libhegel.
   - Exception: explicit examples (`@example` equivalents) depend too much on the host language to live in the ABI. Implementing them natively in each frontend is expected.
   - Host-language work stays in the frontend: rendering a value as host syntax, formatting bytes the engine returns as IP or UUID text, and finding a failure's origin.
2. **Settings come from the engine.** Start from the engine's resolved settings (`hegel_settings_new` / `hegel_settings_new_for_profile`, which apply profiles, `hegel.toml` and the `HEGEL_*` environment variables). Then set only the fields the user explicitly gave. Never re-set a field to a frontend default, and never detect CI in the frontend.
3. **Matching Hypothesis is not a goal** for the Python frontend, or for any other.
4. **Every engine setting and every engine capability is reachable from every frontend**, spelled in that language's idiom. Rust is the reference for behaviour, but not for spelling.
5. **Temporal generators are out of scope.** Dates, times, datetimes, durations, timezones and the chrono/jiff extras belong to PR hegeldev/hegel#16 (branch `DRMacIver/datetime`), which reworks them in every frontend. Don't touch them here.

## Working rules

- All of this work lives on the one branch `DRMacIver/consistency` and goes in as one PR. Agents working in parallel may use temporary worktrees, but merge them back and delete them.
- Commit after each item, or each coherent group of items, with a message naming the item IDs. Don't push, and don't open PRs.
- Run the component's definition of done before finishing: `just <component>::check`. Use `just libhegel::check` for the engine and `just rust::check` for Rust, which also needs `just lint` from the root for workspace lints. If a check can't run locally (a missing toolchain), say exactly which and why.
- A change to shipped code needs `<component>/RELEASE.md`; see the `changelog` skill. Each component's RELEASE.md accumulates all its entries. Breaking API changes are `minor` on 0.x.
- Never add `// nocov`, `# nocov`, `GCOVR_EXCL`, `v8 ignore` or other coverage exclusions, and never raise the numbers in coverage ratchets. If code seems uncoverable, restructure it or report it as blocked.
- Follow the repo conventions in `AGENTS.md` and the component's `AGENTS.md`: tests in `tests/`, no stray comments, and so on.
- Keep each component's docs (README, AGENTS.md, API docs, guides) in step with what you change.

---

## libhegel

- **L1. Engine-owned default sizes for text and binary.** Every frontend hard-codes `max_size = 100` (or `min_size + 100`) for text and binary when the user gives no maximum, while collections default to engine-unbounded. Let the frontend say "no maximum given" and have the engine apply the default. Keep it ABI-compatible, for example with a documented sentinel or a new constant in `hegel.h`. Frontends then pass the sentinel instead of 100.
- **L2. Parse setting values from strings in the engine.** Rust's `#[hegel::main]` CLI (`frontends/rust/src/cli.rs`) and OCaml's `Hegel.main` (`frontends/ocaml/lib/cli.ml`) hand-write vocabularies for verbosity, health checks, backend, booleans and so on. The engine already parses these for `hegel.toml` and `HEGEL_*`. Add an ABI function that sets a setting from its key and string value, using the same parser and the same key names as `hegel.toml`, with the full `hegel_result_t` error contract. Then every engine setting is settable from a CLI.
- **L3. Publish the filter retry policy.** Every frontend hard-codes 3 attempts per filter before rejecting. Put the policy in `hegel.h`, for example `HEGEL_FILTER_ATTEMPTS`, so that frontends use the engine's number.
- **L4. Move lark explicit-terminal splicing into the engine.** Rust (`frontends/rust/src/generators/lark.rs:160-262`) does marker splicing, size accounting and the 3-attempt retry with budget shrinking natively, around `hegel_grammar_placeholder` and `hegel_generate_grammar`. Every frontend that adds grammar support would have to duplicate it. Give the ABI what it needs so that a frontend supplies only the explicit terminals' values, through a callback or a draw-then-provide protocol consistent with the rest of the ABI.
- **L5. Header and doc accuracy.** Fix `hegel.h` documentation found stale while doing L1–L4. Update the TypeScript `typescript-ffi` skill notes if the ABI grows.

Done when `just libhegel::check` passes and `libhegel/RELEASE.md` describes the additions.

---

## Cross-cutting requirements (X)

Each frontend section below says which X items it still owes. Rust and Python already meet most of them.

- **X1. Settings resolution.** Follow Principle 2. Expose every engine setting:
  - `test_cases`, `seed` (full u64), `derandomize`, `database` (default / disabled / path), `verbosity`, `phases`, `suppress_health_check`
  - `print_blob`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness`, `backend`, `show_statistics`, `unbounded_choices`
  - `test_location`, and the database key (derived consistently, and only when a database is in use, as `hegel::test()`/Rust do)
  - programmatic profiles: `register_profile`, `new_for_profile`/from-profile, `set_default_profile`

  Remove frontend CI detection.
- **X2. Typed run errors.** Classify errored runs with `hegel_run_result_error_kind` into distinct host-language error types: health check (with which check, if the ABI says), flaky/nondeterministic, unsatisfiable, and other run error. Use Python's taxonomy as the model: `FailedHealthCheck`, `Flaky`, `Unsatisfiable`, `RunError`. Spell the names idiomatically. Show failure caveats from `hegel_failure_caveat`.
- **X3. Output through the engine.**
  - Draws are printed through `hegel_test_case_printer` and the `hegel_printer_*` functions. The frontend renders each value in host-language syntax, consistently quoted, and gives the engine the structure (groups and breakables) for layout. Use `hegel_printer_reflow` for opaque reprs.
  - Notes go through `hegel_note`.
  - Use `hegel_test_case_should_capture` to decide when to print.
  - Use `hegel_test_case_block` for indented sections and `hegel_test_case_set_worker` for worker attribution.
  - No native buffering, `[worker N]` prefixes or draw-numbering tables where the engine provides them.
  - Print the reproduce blob according to the engine's `print_blob` setting.
- **X4. Labels from the engine.** Use `hegel_label_from_name` / `hegel_label_combine`, not hand-written FNV-1a or hard-coded integers. Labels only need to be consistent within a single test case, so frontends may name their standard generators' labels differently.
- **X5. Replay through the engine.** Replay a failure blob with `hegel_run_start_blob`, using every blob given, not only the first.
- **X6. Test-case API parity.**
  - `draw` (named and silent), `assume`/`reject`, `note`, `target` (with labels), `event`/`event_value` (with `show_statistics`), and `repeat`.
  - A public way for a user's test to draw from several threads: clone plus a worker index, as Rust's `TestCase: Clone` allows.
  - Explicit examples, natively in the frontend. An explicit test case that calls `assume(false)` or `reject()` is a usage error: the run fails, saying so. It is never skipped.
- **X7. Recursion.** `recursive(base, extend)` through `hegel_new_recursion` and the `hegel_recursion_*` functions, plus `deferred` for forward references.
- **X8. Grammars.** Lark-grammar generation through `hegel_grammar_*`, including explicit terminals, using L4 and with Rust as reference. Only after the libhegel branch is merged in.
- **X9. Pools and handles.** Call `hegel_pool_restore` when a filter attempt that consumed pool values is rejected. Free every handle the ABI hands out: collections, pools, contexts, recursions and printers.
- **X10. Generator parity, excluding temporal ones.**
  - Arbitrary-width integers (`hegel_generate_integer_big`), uuids, and 32-bit floats (float width).
  - Lists with `unique` and `unique_by`; sets; permutations.
  - Fixed-size arrays or tuples where idiomatic; a none/unit generator; `optional` using the engine's OPTIONAL label.
  - Text `alphabet` and `codec`; `from_regex` `alphabet`.
  - Derived or default generators where the language can support them.
- **X11. Unicode version.** Where the host runtime reports its Unicode version, pass it to the engine as Python does (see `frontends/python`); TypeScript has `process.versions.unicode`. Where the host has none, do nothing. It is never a setting.
- **X12. Docs.** Fix stale version pins, examples and claims in that frontend's docs.
- **XL. Adopt the libhegel changes** once the libhegel branch is merged in: L1 (drop the hard-coded 100), L3 (filter attempts), L2 (CLIs, where the frontend has one), then X8.

---

## Rust

- **R1.** X2: run errors are all one `panic!(message)` (`run_lifecycle.rs` ~606-621). Panic with distinguishable, documented messages or payload types per error kind, and show caveats.
- **R2.** X4: replace the FNV-1a reimplementation in `src/generators/labels.rs` with the ABI functions. If compile-time labels are needed, compute them lazily (for example with `OnceLock`) rather than duplicating the hash.
- **R3.** Add the `unbounded_choices` setting on every surface (see the `new-setting` skill). `register_profile` must not drop it. Make `Settings::database` able to express "engine default", and make the database key a supported setting.
- **R4.** The `#[hegel::main]` CLI must cover every setting. Do this through L2 in the XL stage. Until then, leave `cli.rs` alone.
- **R5.** Bugs:
  - `ExplicitTestCase` lacks `event`, `event_value`, `repeat` and `draw_named`.
  - The `repeat()` docs say `// Loop iteration N` but the code prints `// Repetition #N`.
  - The reproducer line always says "below #[hegel::test]", even for `#[hegel::main]`, standalone and builder runs.
  - `HealthCheck::all()` returns a fixed `[HealthCheck; 4]` on a `#[non_exhaustive]` enum.
  - The float `exclude_min`/`exclude_max` stubs `unreachable!` at runtime (`numeric.rs:250-265`). Make them work or remove them.
- **R6.** X3: add `if_break` to `PrettyPrinter` (`hegel_printer_if_break`).
- **R7.** X10: make `unique_by` on `vecs` public. Integers wider than i128 have no Rust type, so record that rather than changing anything.
- **R8.** XL, including R4.

## Python

- **P1.** X1: `Settings._build` (`settings.py:360-390`) rebuilds from `base` and copies only the 13 fields it knows about, so other engine settings are reset on every run. Keep the engine's settings and apply only what the user overrode. Add `unbounded_choices`.
- **P2.** X6: make clone and worker public so a user's test can draw from threads. These are `_clone`/`_set_worker`, `test_case.py:447-452`.
- **P3.** Remove Python-only limits that Rust doesn't have, unless they guard against a real Python failure (for example `RecursionError`): the 64-deep `deferred` cap and `MAX_DRAW_DEPTH = 100`. If a limit stays, document why.
- **P4.** XL, including X8 (`from_lark`). Fix the 0.2.0 CHANGELOG claim about `from_lark()` by making it true, without rewriting released history.

## TypeScript

- **T1.** X1:
  - Remove the native CI detection and defaults (`src/nodeRuntime.ts:8-42`, `src/browser/runtime.ts`) and the unconditional overwrites (`src/runnerCore.ts:411-421`).
  - Add the missing settings: `phases`, `print_blob`, `stop_on_first_failure`, `nondeterminism_strictness`, `backend`, `show_statistics`, `unbounded_choices`, profiles and `test_location`.
  - Make `seed` accept a `bigint`.
  - Derive the database key from the test's identity (file plus name), not `testFn.toString()`.
- **T2.** Antithesis reporting: delete the native `sdk.jsonl` writer (`src/nodeRuntime.ts:44-89`) and use `hegel_settings_set_test_location`. Export the test-location and builder API.
- **T3.** X2. Rethrow the user's own error, keeping its class, `cause` and assertion diffs. With `reportMultipleFailures`, throw all the failures (for example as an `AggregateError`), not only the last.
- **T4.** X3: drop `console.error`/`util.inspect` draw printing and the native `isFinal` logic, so that Node and browser print the same. X4: replace the hard-coded labels 1–16 (`src/testCase.ts:40-58`).
- **T5.** X5 (`hegel_run_start_blob`, and print the blob) and X6 (`target`, `event`, explicit examples, clone/worker/block).
- **T6.** X7, X9 (`hegel_pool_restore`) and X10: `uniqueBy`, float width, unbounded big integers (not capped at ±2^127), permutations, deferred, and a none generator.
- **T7.** X11 (`process.versions.unicode`) and X12:
  - The README's sample output is wrong.
  - Remove the `v8 ignore` comment in `src/generators/compose.ts:42-44` by restructuring or testing the code.
  - Fix `sampledFrom`'s "Panics" doc and the `Generator` doc's mention of "the hegel server".
- **T8.** XL, including X8.
- Keep Node and browser at parity. Where the browser can't do something (persistence), document it rather than diverging silently.

## C++

- **C1.** X1:
  - `apply_settings` (`src/hegel.cpp:480-576`) must set only the fields the user gave. Make them `std::optional`, or equivalent, and fix the `settings.h` docs to match.
  - Remove `internal::in_ci()`.
  - Send `print_blob`. Add `show_statistics`, `unbounded_choices`, profiles and `hegel_version`.
  - Let `suppress_health_check` express "none".
  - Make `HEGEL_TEST` and `hegel::test()` agree on database-key derivation, and fix the contradictory `database_key` doc.
- **C2.** X2 (no more blanket `std::runtime_error`) and X5 (replace the single `hegel_test_case_from_blob` replay in `run_from_blob` with `hegel_run_start_blob`).
- **C3.** X3: replace the native output machinery with the engine printer, `hegel_note`, `hegel_test_case_block` and `hegel_test_case_set_worker`. That machinery is `repr.h`'s rendering of values to text (keep the C++ rendering; send it through the engine printer), `DrawLogScope`, the `TestCase::note` buffering, `NoteIndentScope`, the `[worker N]` prefixes and `group_concurrent_output`. X4: add `hegel_label_combine`.
- **C4.** X6 (`event`/`event_value`, explicit examples), X7 (`recursive`), X9 (`hegel_pool_restore`) and X10: `none()`, permutations, `unique_by`, integers wider than 64 bits if a sensible type exists, and float width.
- **C5.** X12:
  - Bump the README and Doxygen tag pins from 0.13.1 to the current version.
  - Delete the vestigial `verbosity_to_string`/`health_check_to_string` and fix the `RandomsParams` doc.
  - Correct the CHANGELOG's claim that the nix flake is gone. The flake is a deliberate consumer interface; correct this with a RELEASE.md note rather than editing old entries.
  - Make `compose()` open a span like map, flat_map and filter do.
- **C6.** XL, including X8.

## Java

- **J1.** Leaks: `hegel_collection_free`, `hegel_pool_free` and `hegel_context_free` are never called. Collections leak on every draw, pools always, and contexts once per worker thread per test case in concurrent stateful runs. Free them deterministically and add tests that would catch the leak.
- **J2.** X1:
  - `Runner.java:271,278` always sets `verbosity` and `reportMultipleFailures`; stop doing that.
  - Add `stop_on_first_failure`, `show_statistics`, `unbounded_choices`, `test_location` and profiles.
  - Class-qualify the default database key, so same-named tests in different classes stop sharing one.
- **J3.** X2: replace the `startsWith("FailedHealthCheck")` sniffing in `RunReport.java:70`. X4: drop the FNV-1a copy in `Label.java`.
- **J4.** X3: replace `TestCase.repr`/`nextName`, the `PrintingReporter` draw and note buffering, the stateful notes and `TestCase.prefix` worker stamps with the engine printer, `hegel_note`, block and `set_worker`. Keep Java's value rendering.
- **J5.** X6 (`event`/`event_value`, explicit examples, so that `Phase.EXPLICIT` stops being "reserved"), X7, X9 (`hegel_pool_restore`) and X10:
  - `BigInteger` integers, unique lists and `unique_by`, permutations, fixed-size arrays.
  - `optional` with the OPTIONAL label.
  - `text()` alphabet/codec and `fromRegex` alphabet.
- **J6.** X12: fix the README install snippet (0.1.0 → current). Check `HealthCheck` against the engine's full list.
- **J7.** XL, including X8.
- FFM and JNA must stay at parity: the same shared code paths, and both bindings for every new ABI function.

## OCaml

- **O1.** X3, the main item. OCaml calls the engine printer but only as a layout engine for s-expressions (`lib/internal.ml.in:444`, `render_sexp`), which causes several problems:
  - Values print as sexps, not OCaml syntax: lists as `(1 2 3)`, tuples as `(1 2)`, records as `((x 1) (y 2))`.
  - Strings are quoted inconsistently: `hello` prints bare but `"hello world"` is quoted.
  - `map`, `flat_map`, `just`, `sampled_from` and `composite` produce generators that can't be printed, so users need `with_printer`/`draw_silent`.

  The datetime branch already needed an ephemeron `raw_text` hack (commit `eb76fd98`) to work around this. Replace the sexp-based printer type with one that renders OCaml syntax through the engine printer's groups and breakables:
  - `[1; 2; 3]`, `(1, 2)`, `Some x`, `{ x = 1; y = 2 }`, and strings always quoted with `%S`-style escaping.
  - A printer for derived types through `ppx_hegel_generator`.
  - Generators that can always be drawn, falling back to an opaque placeholder such as `<abstr>` when there is no printer, for values that have no printer. Keep the existing printable/non-printable distinction and `with_printer` so that old code still compiles. Printable generators print through the engine printer in OCaml syntax, and sexp is used only where the user explicitly asked for it.

  Follow Rust's printing (`frontends/rust/src/pretty.rs` and its use) as the behavioural reference. This conflicts with the datetime branch; that's accepted. Keep the change easy to rebase, and note the conflict in RELEASE.md or the commit.
- **O2.** X4: `lib/generators_core.ml.in:26-40` reimplements FNV-1a; use the bound ABI functions. Remove the client-side duplicate validation in `text`/`characters` (`lib/generators_primitives.ml.in:91-123, 177-204`) and let the engine validate.
- **O3.** X2: no more blanket `Failure` (`lib/internal.ml.in:858-863`). Add the `unbounded_choices` setting, and make `Hegel.main` use it instead of suppressing `Test_cases_too_large`. X5: use every failure blob, not only the first.
- **O4.** X6: explicit examples, and make worker/clone public. X7: `recursive` and `deferred`. The deriver should support recursive types once these exist.
- **O5.** X10: uuids, big integers (if a dependency such as zarith is acceptable as an optional sublibrary; otherwise record that), 32-bit float width, sets, permutations, arrays, `unique_by`, `from_regex` alphabet, and tuples beyond 4.
- **O6.** X12: `AGENTS.md` refers to `GAPS.md`, `plan.md` and `examples/workload.ml`, which don't exist. Fix the references.
- **O7.** XL, including X8 and L2 for `Hegel.main` (`lib/cli.ml`).

---

## Not in this plan (needs a decision from David)

- Temporal generators everywhere, including Rust's chrono/jiff draws bypassing `hegel_generate_time`/`hegel_generate_datetime`. This is PR #16's job.
- Stateful preconditions, initialize rules, teardown and bundle semantics, which are missing in every frontend.
- Decimals, fractions and complex numbers, which are missing everywhere except Python's `from_type`.
- Python `database=None` meaning "engine default" where Rust's `None` disables. Changing it is breaking.
- TypeScript naming that differs from the other frontends (`arrays`, `maps`, `record`, `bigIntegers`). Changing it is breaking.
- Whether the engine should own failure-report wording: the "N distinct failures" header and stateful step and round headers.
- A deadline or per-test-case timeout. The engine has none.
