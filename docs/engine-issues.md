# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.45.0) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

## Regular expressions

- Patterns no string can match, such as `a^b` or `(?!foo)foo`, are not
  recognised as empty: the engine keeps generating strings that fail the
  match, and the run ends `FilterTooMuch` rather than finding nothing
  (`tests/hypothesis_port/cover/test_regex.py::test_caret_in_the_middle_does_not_generate_anything`,
  `test_impossible_negative_lookahead`).
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
- Under `re.IGNORECASE`, a negated class whose excluded character has a
  multi-character case fold (`[^İ]+`; `İ` folds to `i` followed by U+0307)
  can generate the fold's components, so the result is not disjoint from the
  fold (`tests/hypothesis_port/nocover/test_regex.py::test_case_insensitive_not_literal_never_constructs_multichar_match`,
  pinned with a seed).

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
  case 5,006. Hypothesis targets from much earlier, and also reports the
  highest score seen for each label when the run ends; libhegel reports no
  scores.

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

- A failure that reproduces only once is dropped once another failure is
  confirmed. With a test that has two bugs, one deterministic (`TypeError`)
  and one that fails exactly once (`ValueError`), the first replay miss
  switches the whole run into nondeterministic handling
  (`native/test_runner.rs` `first_check_sweep`), the discovery sweep then
  rejects the `ValueError` origin when its confirmation batch never
  reproduces (`reject_origin`), and `build_report` only reports unconfirmed
  origins when nothing confirmed survived. The result is a lone `TypeError`
  carrying a nondeterminism caveat ("failed 20 of 20 replays") even though
  that bug is deterministic, and under `nondeterminism_strictness="error"`
  a `Flaky` that aborts the run. Hypothesis reports both: the `TypeError`
  and a `Flaky` for the `ValueError`
  (`tests/hypothesis_port/cover/test_slippage.py::test_handles_flaky_tests_where_only_one_is_flaky`).

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
- Pool consumption is not rolled back when the draw is discarded.
  `hegel_pool_generate(consume=true)` removes the variable from the pool at
  draw time (`native/data_source.rs` `pool_generate`, then
  `NativeVariables::consume`), and neither closing the enclosing span with
  `discard=true` nor `state_machine_rule_rejected` restores it. The C API has
  no `remove`/`restore` call and no way to generate from a subset of the
  pool, so the bindings cannot defer the removal either. Consequently
  `pool.values_consumed().filter(f)` consumes every value the filter rejects
  on the way to an accepted one, `lists(pool.values_consumed())` consumes
  elements of a list that is then rejected, and a rule that draws a consumed
  value and then fails `assume` loses that value, where Hypothesis's
  `consumes(bundle)` only removes the value the rule actually ran with.
  Repro: add 0..9 to a pool, draw `values_consumed().filter(even)`, and the
  pool is short an odd value. Fix options: make consumption transactional
  with spans and rule rejection, or expose restore/remove so the bindings can
  do it (`tests/hypothesis_port/cover/test_stateful.py::test_filtering_consumed_bundle_consumes_only_the_drawn_value`).

## Other

- `sets(sampled_from(range(n)), min_size=n)` fails `FilterTooMuch` because the
  engine's collection generation does not sample without replacement; the
  same goes for the keys of `dicts(sampled_from(range(n)), ..., min_size=n)`
  (`tests/hypothesis_port/nocover/test_sampled_from.py`,
  `cover/test_sampled_from.py`).
- A test whose every input is rejected ends as a `FilterTooMuch` health
  check, or passes quietly if that check is suppressed. `Unsatisfiable` is
  only raised when the test draws no data at all. Hypothesis raises
  `Unsatisfiable` in both cases, and its message notes the rejection rate
  (or that the finite search space was exhausted); the health check is for
  tests that reject *most* inputs, not all of them
  (`tests/hypothesis_port/cover/test_given_error_conditions.py`,
  `cover/test_testdecorators.py::test_notes_high_filter_rates_in_unsatisfiable_error`,
  `::test_notes_exhausted_search_space_in_unsatisfiable_error`,
  `cover/test_searchstrategy.py::test_example_raises_unsatisfiable_when_too_filtered`,
  `nocover/test_given_error_conditions.py`, `nocover/test_testdecorators.py`,
  `cover/test_sampled_from.py::test_sample_when_all_filtered_fails_as_filter_too_much`).
- A finite search space is not detected as exhausted: a test drawing one
  `booleans()` runs for the full `test_cases` (200 calls with
  `test_cases=200`) where Hypothesis stops after the two distinct inputs
  (`cover/test_testdecorators.py::test_given_usable_inline_on_lambdas`).
- Shrinking a `lists(uuids())` to length five drives every element to the
  same minimal UUID (`UUID(int=1)` five times); Hypothesis's shrinker keeps
  the elements distinct
  (`nocover/test_uuids.py::test_retains_uniqueness_in_simplify`).
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
- Identical inputs are generated repeatedly; there is no duplicate suppression
  (seen as repeated UUIDs in `tests/hypothesis_port/nocover/test_uuids.py`).
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
