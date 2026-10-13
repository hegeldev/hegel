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
- A character class that matches nothing in the given alphabet (`\d` with
  `alphabet="abc"`, `[^a]` with `alphabet="a"`) is not reported up front.
  Hypothesis raises an error naming the class and the alphabet; libhegel
  generates and filters
  (`test_regex.py::test_charsets_which_match_nothing_in_the_alphabet`).
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
  "empty"]), …)` never draw `"empty"` (`tests/test_engine_issues.py`;
  `tests/hypothesis_port/nocover/test_sampled_from.py::test_exhaustive_flags`, which
  fails intermittently for the same reason).
- Targeting does steer generation, but only from halfway through the
  budget: the first targeting pass starts after `test_cases / 2` cases. With
  `test_cases=10_000`, `lists(integers(0, 1000))`, `target(min(sum(xs),
  50_000))` and `assert sum(xs) < 30_000`, the failure is found at about
  case 5,006. Hypothesis targets from much earlier, and also reports the
  highest score seen for each label when the run ends; libhegel reports no
  scores.

- A float range consisting of a single infinity cannot be drawn from: with
  64-bit floats, `floats(inf, inf)` aborts the run with an `InternalError`
  (`FloatChoice::simplest: no valid float for this choice`), and with 32-bit
  floats the infinite bound is clamped to the largest finite value and then
  rejected as exceeding the other bound. Hypothesis returns the infinity
  (`tests/hypothesis_port/cover/test_float_nastiness.py::test_interval_of_a_single_infinity`).
- A reproduce blob replayed against a test whose draws have changed shape is
  continued with fresh draws past the divergence, so a failure found that way
  is reported as the failure rather than as `DidNotReproduce`
  (`tests/hypothesis_port/cover/test_reproduce_failure.py::test_errors_with_did_not_reproduce_if_the_shape_changes`).

## Messages and reporting

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

## Other

- `sets(sampled_from(range(n)), min_size=n)` fails `FilterTooMuch` because the
  engine's collection generation does not sample without replacement
  (`tests/hypothesis_port/nocover/test_sampled_from.py`).
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
