# Frontend consistency: feature-by-feature design

Source: a read-only survey of libhegel and all six frontends on 2026-10-27, run per component against one fixed questionnaire, then compared per feature. This document replaces the per-frontend item lists of `frontend-consistency.md` for the follow-on work: work is now split **per feature**, and each feature section says what every frontend must do, which frontends already do it, and what has to change. The principles of `frontend-consistency.md` still apply (thin drivers of libhegel, settings from the engine, Rust is the behavioural reference but not the spelling reference, temporal generators out of scope).

Items marked **DECIDED** follow Rust or the existing plan and need no further input. Items marked **Q<n>** are open questions for David, collected in the last section with a recommendation each. Nothing below has been implemented yet.

Notation: R = Rust, P = Python, T = TypeScript, C = C++, J = Java, O = OCaml. A table cell says what the frontend does today.

---

## 1. Test identity: database key, test location, failure origin

**Design (DECIDED).**
- The macro / decorator / annotation / PPX form always derives a database key and a test location. The builder / function form derives them only where the language can identify the caller cheaply (TypeScript and C++ do), otherwise only when the user supplies them. Keys are per language and only need to be stable within that language; the exact spelling (`::` vs `.`) is not harmonised, because changing it invalidates existing databases.
- The key is sent with `hegel_settings_set_database_key` whenever the frontend has one, regardless of the database setting (the engine derives the derandomized seed from it). All six do this.
- `hegel_settings_set_test_location` is sent whenever a location is known; `class_name` is the language's natural enclosing scope (module path, class, file stem).
- Failure origin is `"<ExceptionKind> at <file>:<line>"`-shaped, computed natively, and only has to be stable. All six conform.

| | R | P | T | C | J | O |
|---|---|---|---|---|---|---|
| macro-form key | `module::fn` | `module.qualname` | `relfile::fn` | `file::name` | `Class.method` | `file::fn` |
| builder-form key | none unless given | none unless given | derived from call site | derived | none unless `name()` | none unless given |

**Changes:** none required. (See Q17 if you want the `::` spelling everywhere.)

## 2. Settings and profiles

**Design.**
- Every engine setting is reachable, spelled idiomatically, with "unset" expressible so the engine's resolved value wins. Every frontend now does this. **DECIDED.**
- One implementation model, Rust's (**DECIDED, Q19**): constructing a settings object resolves the profile through `hegel_settings_new[_for_profile]` *at construction time* and copies every resolved value through the getters; the builder/setters mutate that snapshot; the run re-sets every field onto a fresh engine settings. Consequences that P, T, C, J adopt with it: `register_profile` and `set_default_profile` are not retroactive (a settings object already created keeps its values), resolution errors (unknown profile, malformed `hegel.toml` or variable) surface when the object is created rather than when the test runs, and there is no "unset" state to carry: every field always has a value. Today P, T, C and J are *lazy* (they record only what the user set and resolve at run time): change them. O already follows Rust.
- `seed` is a full u64; out-of-range values are rejected, never wrapped. **DECIDED.** Python masks negatives silently (`settings.py:459-462`): fix. Java's `@HegelTest(seed = Long.MIN_VALUE)` sentinel makes one seed unreachable from the annotation: document or add a `hasSeed`-style attribute (minor).
- `suppress_health_check` must be able to say "none": all six can. **DECIDED.**
- Profiles: `register_profile`, `from_profile`, `set_default_profile` exist everywhere. **DECIDED.**
- `engine_version()` is exposed publicly in every frontend (today only P and C). **DECIDED.** Public getters for resolved settings are not required (only P has them).
- Frontend-only settings stay out of the settings object unless they configure the run itself. C++'s `Settings::stateful_step_count` moves to `stateful::RunParams` (every other frontend passes the step count to the stateful runner). **DECIDED.** Java keeping `examples`, `reproduceFailure`, `infrastructurePackages` on `Settings` is accepted as Java idiom.
- Environment variables and `hegel.toml` are the engine's: no frontend parses them. All conform.

## 3. CLI / `main`

**Design (DECIDED).** Where a frontend has a `main` form (R `#[hegel::main]`, O `Hegel.main`) or a test-runner plugin (P pytest), every setting value is parsed by the engine through `hegel_settings_set_from_string`; the frontend only maps flag names to `hegel.toml` keys. All three conform. A `main` run uses one test case with `TooSlow` and `TestCasesTooLarge` suppressed (R) — O additionally sets `unbounded_choices = true` explicitly, which is redundant with suppressing `TestCasesTooLarge`; align O to R. T, C, J have no CLI, and none is required.

## 4. Test-case API

**Design (DECIDED unless marked).** Every frontend exposes, on the test case:
- `draw(g)`, a named draw, `draw_silent(g)`, `draw_and_print(g, printer)`.
- `assume`, `reject`, `note`, `target(score[, label])`, `event(label)`, `event_value(label, value)`, `repeat(body)`.
- `clone()` and `set_worker(i)` as separate operations. Java only has `fork(worker)` (clone + set_worker fused, plus registration of the handle for freeing at the end of the case): replace it with `clone()` and `setWorker(int)`; the registration moves into `clone()`. **DECIDED (Q14).**
- Blocks (`hegel_test_case_block`) and `should_capture` are driver internals, not public API. Java's public `isFinal()` is tolerated. Spans are hidden/internal (R hides them, P and J expose them; harmless).
- User-written generators implement a silent `do_draw` and, if printable, `do_draw_and_print(tc, printer)`; the default label is the host type name of the generator (R does this; P, T, C, J use a shared `hegel.generator` label, O's `printable_composite` defaults to `hegel.fixed_dicts`). Switch P, T, C, J, O to a per-type default name.
- `repeat` is composed identically in all six (silent `[0, 2^40]` min size, collection loop, `// Repetition #N` note, indent-2 block, assume swallowed per iteration, one boolean drawn before rethrowing). **Q7**: move this policy into libhegel. **Q8**: the note's comment syntax.

## 5. Draw naming

**Design (Q1, recommended = Rust's rule).**
- A name captured statically from a single-use binding (R macro, P AST, O PPX) prints bare: `let x = …`.
- A name that may repeat — an explicitly supplied name, or a captured name bound in a loop/closure/helper or bound by more than one draw — prints numbered from 1: `x_1`, `x_2`, skipping names already taken.
- A nameless draw prints `draw_1`, `draw_2`, …
- Numbering is per naming scope: shared with clones, fresh per block (repetition, stateful step).

| | R | P | T | C | J | O |
|---|---|---|---|---|---|---|
| explicit name, first use | `x_1` | `x_1` | `x` | `x` | `x` | `x` |
| explicit name, second use | `x_2` | `x_2` | `x_2` | `x` again (!) | `x_2` | `x` again (!) |
| nameless | `draw_1` | `draw_1` | `draw_1` | `draw_1` | `draw_1` | `draw_1` |

**Changes:** T, J start numbering at `_1` for explicit names; C and O number an explicitly supplied name (C's `repeatable` flag and O's `?label` currently print duplicates).

## 6. Printing

**Model (DECIDED, done everywhere).** Generator-driven draw-and-print into the engine printer; structural generators print through their components; rejected attempts in speculative regions; value-producing built-ins print the host repr via `reflow`; escape hatches `print_with` / `print_as_<repr>` / `print_as_call` (O: `with_ocaml_printer` / `with_printer` / `map_as_call`). Printability is compile-time where the language allows (R, O, C), type-level plus runtime (T), runtime (P, J). All six conform.

**Printer surface (DECIDED).** All six expose text, reflow, breakable, if_break, hard_break, groups, seq/separator, shift_indent, comment, should_print, no-op, speculation, deferred regions and a standalone document. Nothing missing.

**Open points.**
- **Q6** Python alone prints the nested draws of a *failing* draw (`draw_1 = <KeyError raised while drawing from …>` plus indented inner draws). Rust discards the half-printed line. Adopt everywhere or drop from Python.
- **Q16** TypeScript prints `binary()` as `Buffer.from([…])` on Node and `Uint8Array.from([…])` in browsers (Node returns a `Buffer`). Return `Uint8Array` on every host.
- **Q15** Java prints sets as `[1, 2]` (indistinguishable from lists) and UUIDs as bare `toString()` (`f47ac10b-…`, not an expression). Suggested: `Set.of(1, 2)`, `UUID.fromString("…")`.
- **Q8** `// Repetition #N` is emitted verbatim in P and O, whose comment syntax is `#` and `(* *)`. Everything else those frontends print is host syntax.

## 7. Failure report and verbosity

**Design (DECIDED; Rust's layout, `run_lifecycle.rs:raise_failures`).**
- With several failures: `Property-based test failed with N distinct failures.`, then a blank line before each failure's block (not under quiet).
- Per failure: the captured case lines (draws, notes, stateful headings); the language's own diagnostic where the language prints one (R panic text, O `Exception: …`; P/T/C/J rethrow and let the host runner print); `note: <caveat>` (not under quiet); then, if `print_blob` resolves true and the failure has a blob, a blank line and `To reproduce this failure, <action>:` / `    <code>`.
- Under quiet: no case lines, no header, no caveat; the reproducer still prints. All six conform.
- Verbose: every case's lines as it completes, then the report. Verbose per-case vocabulary follows Rust: `Test case stopped: failed assumption`, `Test case stopped: out of data`; no "passed" line. P's `Test case passed` / `Test case rejected: assumption failed` / `Test case stopped: the engine abandoned it` / `Test case failed: <origin>` and C/J's `Running explicit …` lines are realigned (**Q18** if you prefer Python's richer vocabulary).
- **Q5** Python attaches the report as exception notes and prefixes the lines with `Falsifying example:`. No other frontend has a header.

## 8. Explicit examples

**Design.** Native in every frontend, no engine involvement (accepted exception). Run first, only when `Phase.Explicit` is resolved, in declaration order; each example prints its draws as host-syntax bindings (R and O print the source expression, the others the value's repr) and its notes; a failing example prints those lines and rethrows, with no reproducer. `assume(false)` / `reject()` is the usage error `Explicit test cases must not be rejected`; a value never drawn, or a draw with no value, is a usage error. `target`, `event`, `event_value` are no-ops; `repeat` runs the body once under `// Repetition #1`. All six conform to this much.

Divergences to settle:
- **Q2** Blob replay. R runs the explicit examples and then replays; P, T, C, J, O replay only.
- **Q25 (DECIDED)** `report_multiple_failures`: a failing example ends the run unless the setting is on, in which case every example runs, failures are collected by origin, the generated run still happens, and generated failures sharing an explicit failure's origin are dropped (P, T, J, O do this; R and C stop at the first explicit failure regardless of the setting). Align R and C.
- **Q3** `draw_silent` inside an example: R, P, T, C reject it as unsupported; J consumes the next positional value; O's PPX names silent draws so they can be supplied by name.
- **Q4** `clone` / `set_worker` inside an example: R has neither on `ExplicitTestCase`; P and J work; T returns itself; C and O raise. A threaded test cannot have explicit examples in R, C, O.

## 9. Replay

**Design (DECIDED).** Every blob the test was given (stacked attributes, arrays, repeated flags) is replayed with its own `hegel_run_start_blob` run; every failure is reported; none reproducing raises the frontend's `DidNotReproduce`; a reproduced failure prints no reproducer line; `hegel_test_case_from_blob` is unused. All six conform.

## 10. Errors

**Design (DECIDED).** Per frontend, in its idiom: the user's own failure rethrown unchanged (single failure); an aggregate for several; `RunError` ⊃ `FailedHealthCheck` (with which check) / `Flaky` (covers FLAKY and NONDETERMINISTIC) / `Unsatisfiable`; `DidNotReproduce`; a usage-error type for `HEGEL_E_INVALID_ARG` and frontend validation; internal errors distinct. All six conform. Changes: TypeScript exports `EngineError` (today users cannot catch engine errors by type).

## 11. Generators

**Design.** Parity with Rust's generator set, spelled per language, built on the ABI. Status (✓ present, ✗ absent, ~ partial):

| Generator / option | R | P | T | C | J | O | Decision |
|---|---|---|---|---|---|---|---|
| integers, big integers (±2^127 unbounded convention) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ (zarith) | ok |
| floats `allow_subnormal` | ✓ | ✓ | ✗ | ✗ | ✓ | ✗ | add T, C, O **DECIDED** |
| floats exclusive bounds | `min_value_exclusive(v)` | `exclude_min` | `excludeMin` | `exclude_min` | `excludeMin` | `exclude_min` | **DECIDED (Q12)**: everyone follows Rust's 2026 change. P `floats(min_value_exclusive=, max_value_exclusive=)`, T `{minValueExclusive, maxValueExclusive}`, C `.min_value_exclusive(v)`/`.max_value_exclusive(v)`, J `.minValueExclusive(v)`/`.maxValueExclusive(v)`, O `?min_value_exclusive`/`?max_value_exclusive`. Giving both the inclusive and the exclusive form of one bound is an `invalid_argument` where arguments are simultaneous (P, T, O keyword/options forms); in builders (C, J) the last call wins as in Rust. The boolean forms are removed (0.x). Validation of the resulting bound stays in the engine. |
| weighted booleans | ✓ | ✓ | ✗ | ✗ | ✗ | `?p` | add T, C, J **DECIDED** |
| unit / none | ✓ | ✓ | ✓ | ✓ | `nulls` | ✗ | add O `unit ()` **DECIDED** |
| just, sampled_from, one_of, optional (OPTIONAL label) | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| text options, characters, binary | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| from_regex alphabet | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ | add C **DECIDED** |
| emails, urls, domains, uuids, ip_addresses | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| lists unique / unique_by, sets, maps | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| tuples, fixed arrays | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok (idiom) |
| permutations | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| subsequences, samples | ✓ | ✓ | ✗ | ✗ | ✗ | ✗ | add T, C, J, O (**Q11b**) |
| randoms | ✓ | ✗ | ✗ | ✓ | ✗ | ✗ | **Q11b** |
| map, filter (3 attempts, pool_restore), flat_map, composite | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| recursive, deferred | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| from_lark with explicit terminals via `hegel_grammar_draw_*` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ok |
| derived / default generators | derive | from_type | n/a | reflect | forType | ppx | ok |
| derived generators on recursive types | manual | deferred in builds | n/a | unbounded | unbounded | handled | J, C: reject with a clear error **DECIDED** |

Python-only extras: `nothing()`, `iterables`, `uuids(allow_nil)`, `ip_addresses(network=)`, `text(alphabet=<generator>)` (a native text generator over a collection, bypassing the engine's string generator), `from_type` for Decimal/Fraction/complex. **Q11a.**

Argument validation duplicated in frontends is accepted: it gives construction-time errors; the engine remains the backstop. **DECIDED.**

## 12. Labels and spans

- Labels come from `hegel_label_from_name` / `hegel_label_combine` everywhere; `hegel.<kind>` names are consistent. **DECIDED.**
- Spans: R and P open a span for *every* draw made through the test case (labelled with the generator's label), and compound generators add none of their own beyond filter attempts and stateful rounds. T, C, J, O open spans per compound generator and leave leaves bare. **Q13**: adopt the Rust rule everywhere.

## 13. Stateful testing

**Design.**
- Rules with weights, invariants with always-check, preconditions via `assume` (→ `hegel_state_machine_rule_rejected` + `Rule stopped early due to violated assumption.`), pools as the bundle mechanism, concurrency via `hegel_state_machine_next_group` with one clone per worker and `set_worker`. Step count default 50, passed to the runner.
- Output wording follows Rust: `Checking invariants on the initial state.`, `Step N: <rule> {` … `}`, `Rule <name> failed:`, `Invariant <name> failed:`, `Checking invariants on the final state.`, `Concurrency level: N`, `---------------- Round N: group "g" ----------------`, `Rule: <name> {`. **DECIDED.** C (`Initial invariant check.`, `Invariant x violated after step 2`) and O (`Invariant x violated after step 2.`, `Rule: name` without brace) realign.

| | R | P | T | C | J | O |
|---|---|---|---|---|---|---|
| rule weights | ✓ | ✓ | ✗ | ✗ | ✓ | ✓ |
| invariant always-check | ✓ | ✓ | ✓ | ✗ | ✓ | ✓ |
| concurrency | ✓ | ✓ | ✗ (single-threaded host) | ✓ | ✓ | ✓ |
| `state = …` printing | ✗ | ✗ | ✗ | ✓ (opt-out) | ✗ | ✓ (opt-in) |
| teardown | ✗ | ✓ | ✗ | ✗ | ✗ | ✗ |

**Changes (DECIDED):** T adds weights; C adds weights and always-check; T documents concurrency as unsupported in JavaScript (and fixes the CHANGELOG/BROWSER.md claims that it is pending a newer libhegel); J discovers rules on superclasses too (today `getDeclaredMethods` only). P removes `teardown()` (**Q10**). C and O remove `state = …` printing (`RunParams::print_state`, `?print_state` / `?sexp_of_state`); "print the state" is tracked as a future feature for all six (**Q9, DECIDED**).

## 14. Resources and Unicode

All handles are freed in every frontend (GC-timed for J/O string generators and grammars, accepted). TypeScript's `Document` leaks if never finished: add a `FinalizationRegistry` or document it. **DECIDED.** `hegel_set_unicode_version`: R, P, T(Node) set it; J, C, O have no host source and do nothing. **DECIDED.**

## 15. libhegel

Doc and ABI fixes found by the survey (**DECIDED**): `HEGEL_HC_LARGE_INITIAL_TEST_CASE` header says "warns" but the engine errors the run; `Settings::print_blob` rustdoc says default `false` (it is `true`); the recursion give-up count is unspecified in the header while filters have `HEGEL_FILTER_ATTEMPTS`; the recursion handle is `HegelRecursion` while the prose says `hegel_recursion_t`; the `hegel_verbosity_t` docs describe frontend output ("drawn values", "a short summary line") that the engine never produces. `repeat` moves into the engine (**DECIDED, Q7**).

Environment variables (**DECIDED, Q20**): `HEGEL_VERBOSITY`, `HEGEL_PHASES`, `HEGEL_SUPPRESS_HEALTH_CHECK`, `HEGEL_REPORT_MULTIPLE_FAILURES` and `HEGEL_BACKEND` are added in this PR, with the `hegel.toml` vocabulary (lists comma-separated, `none` for an empty list, booleans as the existing ones), documented in `frontends/rust/src/docs/settings.md` and the `hegel_settings_new` doc comment, tested per the `new-setting` skill. How the gap arose: the five settings date from June 2026, the first variable (`HEGEL_TEST_CASES`) from 2026-08-09, and the "never add a setting without its variable" rule from 2026-10-12; the rule is forward-looking, nothing was backfilled, and no lint enforces it. Add `scripts/check-settings-env.py`, run from `just lint`, that fails when a `hegel_settings_set_<name>` in `hegel.h` has no `HEGEL_<NAME>` in `libhegel/src/settings.rs` (allow-list: `database_key`, `test_location`, which are per-test, not per-run).

## 16. Documentation fixes (DECIDED)

R README calls the crate `hegel-rust`; P README calls it `hegel-python`, `TestCase.draw` docstring on nested draws, `failures.md` numbering rule, settings doc on `HEGEL_TEST_CASES`; T README sample output and `Uint8Array` claim, `src/index.ts` Vitest example form, BROWSER.md/CHANGELOG concurrency claims, `lark.ts` limit claims; C README "requires C++20"; J package-info "requires Java 22+", `x = 42;` form in `Reporter`/`Failure` javadoc, `forType` list, README "every setting has an annotation attribute"; O getting-started `n = 50` form, `note` doc "prints to stderr", `HEGEL_LIBHEGEL_PATH` filename.

---

## Decisions (2026-10-27)

- **Q1 Draw naming.** Rust's rule (section 5) everywhere.
- **Q2 Explicit examples on blob replay.** Skipped: a replay reproduces that blob and nothing else. Rust changes.
- **Q3 `draw_silent` in an explicit example.** Usage error everywhere; OCaml may supply silent draws by name since its PPX can. Java stops consuming positionals for silent draws.
- **Q4 `clone` / `set_worker` in an explicit example.** Supported (a clone shares the example's values; the worker is ignored). Added to R, C, O.
- **Q5 Python's `Falsifying example:` header.** Removed; Python's report matches the others.
- **Q6 Nested draws of a failing draw.** Python drops them (Rust discards the line).
- **Q7 `repeat` in the engine.** `hegel_new_repeat` / `hegel_repeat_more` added to libhegel; the six native copies go.
- **Q8 Repetition note.** Host comment syntax (`# Repetition #N` in Python, `(* Repetition #N *)` in OCaml, `// …` elsewhere).
- **Q10 Python `teardown()`.** Removed.
- **Q11a Python-only extras.** `text(alphabet=<generator>)` removed; `nothing()`, `iterables`, `uuids(allow_nil)`, `ip_addresses(network=)` stay Python-only.
- **Q11b.** `subsequences` and `samples` added to T, C, J, O; `randoms` stays R and C.
- **Q12 Float exclusive bounds.** Everyone follows Rust's `min_value_exclusive(v)` / `max_value_exclusive(v)` (section 11 table has the spellings).
- **Q13 Spans.** Every draw opens a span, everywhere.
- **Q14 Java.** `fork(worker)` is removed; `clone()` and `setWorker(int)` replace it.
- **Q15 Java printed forms.** `Set.of(…)` and `UUID.fromString("…")`.
- **Q16 TypeScript `binary()`.** `Uint8Array` on Node too.
- **Q17 Database key spelling.** Per language.
- **Q18 Verbose per-case vocabulary.** Rust's.
- **Q19 Settings model.** Rust's eager model everywhere (section 2).
- **Q20 Engine env vars.** Added in this PR, plus a lint (section 15).
- **Q9 Stateful `state = …` printing.** Removed from C++ and OCaml; "print the state" is a future feature for all six.

## Work split

One agent per feature, each in its own worktree on a `consistency/<feature>` branch off `DRMacIver/consistency`, touching every frontend the feature needs, merged back by the coordinator:
1. `naming-spans`: draw naming (Q1: T, J, C, O), user-generator default label = host type name (P, T, C, J, O), spans per draw (Q13: T, C, J, O), Python stops printing the nested draws of a failing draw (Q6).
2. `explicit-examples`: skip under blob replay (Q2: R), `report_multiple_failures` collects explicit failures (Q25: R, C), `draw_silent` is a usage error (Q3: J), `clone`/`set_worker` supported (Q4: R, C, O), Java `fork` → `clone()` + `setWorker()` (Q14).
3. `report-verbosity`: Python drops `Falsifying example:` (Q5); verbose vocabulary is Rust's (Q18: P, C, J).
4. `generators`: `weighted_booleans` (T, C, J), `unit` (O), `from_regex` alphabet (C), `subsequences`/`samples` (T, C, J, O), derived generators reject recursive types (J, C), Python drops `text(alphabet=<generator>)` (Q11a).
5. `floats-and-forms`: `allow_subnormal` (T, C, O), exclusive bounds (Q12: P, T, C, J, O), Java `Set.of`/`UUID.fromString` (Q15), TypeScript `binary()` → `Uint8Array` (Q16).
6. `stateful`: weights (T, C), always-check (C), wording (C, O), remove state printing (Q9: C, O), remove Python `teardown()` (Q10), Java superclass rules, TypeScript concurrency docs, C++ `stateful_step_count` → `RunParams`.
7. `repeat-engine`: `repeat` into libhegel (Q7) and all six frontends onto it, host comment syntax for the note (Q8), libhegel header doc fixes (section 15).
8. `settings`: eager model (Q19: P, T, C, J), env vars + lint (Q20), Python seed, Java seed sentinel doc, `engine_version` (R, T, J, O), OCaml `main` alignment, TypeScript `EngineError` export and `Document` finalizer, documentation fixes (section 16).
