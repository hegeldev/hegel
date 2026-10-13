# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.45.0) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

## Regular expressions

- A pattern that the given alphabet cannot match (`abc` with
  `alphabet="def"`, `aa|bb` with `"c"`, `[ab]x[de]` with `"abcdef"`, `\d`
  with `"abc"`, `[^a]` with `"a"`) is not reported up front. Hypothesis
  raises an error naming the class and the alphabet when the generator is
  built; libhegel already computes whether each subpattern is producible
  under the alphabet (`producible_sub` in `native/draws/regex.rs`), but only
  lazily during generation, where an unproducible branch calls
  `mark_invalid`, so every test case is rejected and the run ends
  `Unsatisfiable`. Running that check on the top-level pattern when the
  string generator is constructed, and returning an error, would fix it
  (`test_regex.py::test_charsets_which_match_nothing_in_the_alphabet`,
  `tests/hypothesis_port/cover/test_direct_strategies.py`, the
  `REGEX_ALPHABET_UNCHECKED` rows).
- `from_regex(".")` shrinks to `'ā'` (U+0101) rather than to `'0'`: the
  shrinker's smallest match for `.` is not the smallest code point
  (`test_regex.py::test_regex_output_should_print_as_string`).
## Shrinking and generation

- Shrinking can stop short of the minimum: with seed 6, a search over
  `integers(0, 100)` for a positive multiple of six shrinks to 78 rather
  than 6. Unseeded, about one run in twenty stops early like this
  (`tests/test_engine_issues.py`).
- Small finite spaces are covered unevenly. The all-zero choice sequence is
  drawn far more often than its share (28% of cases against an expected
  12.5% for an eight-value space), so other values can be missed entirely:
  with seed 27, a hundred cases of `one_of(sampled_from(["a", "b", "c",
  "empty"]), …)` never draw `"empty"` (`tests/test_engine_issues.py`).
- Targeting does steer generation, but only from halfway through the
  budget: the first targeting pass starts after `test_cases / 2` cases. With
  `test_cases=10_000`, `lists(integers(0, 1000))`, `target(min(sum(xs),
  50_000))` and `assert sum(xs) < 30_000`, the failure is found at about
  case 5,006. Hypothesis targets from much earlier.
- A float draw with an infinite bound and `allow_infinity=false` aborts the
  run with an `InternalError` (`FloatChoice::simplest: no valid float for
  this choice`) at width 64, and at width 32 clamps the bound to the largest
  finite value and then rejects it as exceeding the other bound. The
  bindings now reject that combination as an invalid argument before it
  reaches the engine, so this is only reachable from other bindings.
- A reproduce blob replayed against a test whose draws have changed shape is
  continued with fresh draws past the divergence, so a failure found that way
  is reported as the failure rather than as `DidNotReproduce`
  (`tests/hypothesis_port/cover/test_reproduce_failure.py::test_errors_with_did_not_reproduce_if_the_shape_changes`).

## Messages and reporting

- A deterministic failure is caveated as nondeterministic when a different
  failure made the run nondeterministic. With a test that has two bugs, one
  deterministic (`TypeError`) and one that fails exactly once
  (`ValueError`), the first replay miss switches the whole run into
  nondeterministic handling (`native/test_runner.rs` `first_check_sweep`)
  and the report is a lone `TypeError` carrying the caveat "failed 20 of 20
  replays", even though that bug reproduced at every replay; under
  `nondeterminism_strictness="error"` it is a `Flaky` that aborts the run.
  The caveat should be per failure. (Reporting the never-reproduced
  `ValueError` as well, as Hypothesis does with a `Flaky`, is not a goal.)
- Verbose output announces generated test cases with `Running test case`,
  but not the first case of a run (the simplest input,
  `NativeTestCase::for_simplest` in `native/test_runner.rs`) nor the replays
  of the reuse phase, so the first drawn values appear under
  `Starting phase: Generate` with no heading.
- The `FilterTooMuch` health check message uses Rust syntax
  (`HealthCheck::FilterTooMuch`) for the setting to suppress it, and blames
  `assume()` even when a generator's `.filter()` did the rejecting.
- Run errors (health checks, flaky tests, unsatisfiable runs) reach the
  bindings only as messages, with no structured kind, so `hegel.runner`
  classifies them by message prefix (`_RUN_ERROR_PREFIXES`). A change of
  wording in the engine would silently turn them into plain `RunError`s.

## Stateful testing

- There is no minimum step count for a state machine run: the engine decides
  how many steps to take, so Hypothesis's `_min_steps` is rejected with
  `InvalidArgument` rather than honoured.
- A value consumed from a pool inside a filter's rejected attempt is lost.
  `hegel_pool_generate(consume=true)` removes the variable at draw time
  (`native/data_source.rs` `pool_generate`, then
  `NativeVariables::consume`), and the C API has no call to put it back, so
  `pool.values_consumed().filter(f)` consumes every value the filter rejects
  on the way to an accepted one although no rule ran with them. Repro: add
  0..9 to a pool, draw `values_consumed().filter(even)`, and the pool is
  short an odd value. The fix is a restore call that un-consumes the
  stream's most recent consume from the pool, which `filter` calls for a
  value it rejects (the combinator still holds the value, so no copy is
  needed). A rule that consumes a value and then fails `assume` keeps
  losing it, as the rule body owns the value by then; Hypothesis's
  argument-level `consumes(bundle)` differs there, and that is not a goal
  (`tests/hypothesis_port/cover/test_stateful.py::test_filtering_consumed_bundle_consumes_only_the_drawn_value`).
## Other

- A test whose every input is rejected ends as a `FilterTooMuch` health
  check, or passes quietly if that check is suppressed. `Unsatisfiable` is
  only raised when the test draws no data at all. Hypothesis raises
  `Unsatisfiable` in both cases; the health check is for tests that reject
  *most* inputs, not all of them. The same shows up for a regex no string can
  match (`a^b`, `(?!foo)foo`), where every draw is rejected
  (`tests/hypothesis_port/cover/test_given_error_conditions.py`,
  `cover/test_testdecorators.py::test_notes_high_filter_rates_in_unsatisfiable_error`,
  `::test_rejecting_every_input_is_unsatisfiable`,
  `cover/test_searchstrategy.py::test_example_raises_unsatisfiable_when_too_filtered`,
  `nocover/test_given_error_conditions.py`, `nocover/test_testdecorators.py`,
  `cover/test_sampled_from.py::test_sample_when_all_filtered_fails_as_filter_too_much`,
  `cover/test_regex.py::test_caret_in_the_middle_does_not_generate_anything`,
  `::test_impossible_negative_lookahead`).
- Recursive generators rarely reach some shapes: for `deferred` JSON-like
  data, a dict with key `""` holding a list is almost never generated within
  the budget Hypothesis finds it in
  (`tests/hypothesis_port/quality/test_deferred_strategies.py`).
- Bounded integers are drawn with a heavy weight on huge values:
  `integers(0, 10**100)` produced values above `10**25` about two thirds of
  the time (`tests/hypothesis_port/nocover/test_integer_ranges.py`).
- A run whose first twenty or so test cases are all rejected ends quietly as
  passed: with `test_cases=1000` and every health check suppressed, a test
  that draws a value and rejects all but one of seven equally likely values
  stops after about twenty rejections roughly 4% of the time, with no
  `Unsatisfiable` (which is only raised when the test draws no data) and no
  failed health check. A test whose assumptions always fail therefore passes.
  (Seen as an intermittent failure of `find_example` in
  `tests/test_generators_combinators.py::test_sampled_from`; pinned with a
  seed in `tests/test_engine_issues.py`.)
- `uuids()` repeats values far more often than random UUIDs would: each
  64-bit half is drawn with the boundary-weighted integer distribution, so a
  few hundred UUIDs usually contain a duplicate and a `lists(uuids())`
  often holds the same value twice. Hypothesis's `uuids()` are unique within
  a run because they bypass the choice sequence (and so never shrink); Hegel
  draws them as choices, and uniqueness would need engine state rather than
  a different distribution.
- No widening of `one_of` branches. For `integers() | sampled_from([4991,
  4999])` and a predicate the sampled values satisfy, Hypothesis rewrites the
  choices from the sampled branch, `(1, 1)`, into the wide branch carrying the
  same value, `(0, 4999)`, then shrinks on to `4990`. That is a shortlex
  improvement in Hypothesis's typed choice sequence, where a `text()` or
  `integers()` draw is one choice. Hegel shrinks within the sampled branch and
  reports `4991`. Whether widening would be an improvement in Hegel depends on
  how libhegel encodes a string or integer draw: if a draw is several choices,
  the wide form is longer and the engine's answer is the shortlex minimum.
  (A `just()` branch never widens in Hypothesis either, as its encoding is
  shorter.)
