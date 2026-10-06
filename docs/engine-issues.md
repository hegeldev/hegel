# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.45.0) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

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
  `tests/test_generators_combinators.py::test_sampled_from`.)
- Identical inputs are generated repeatedly; there is no duplicate suppression
  (`tests/hypothesis_port/nocover/test_duplication.py`).
- No widening of `one_of` branches. For `integers() | sampled_from([4991,
  4999])` and a predicate the sampled values satisfy, Hypothesis rewrites the
  choices from the sampled branch, `(1, 1)`, into the wide branch carrying the
  same value, `(0, 4999)`, then shrinks on to `4990`. That is a shortlex
  improvement in Hypothesis's typed choice sequence, where a `text()` or
  `integers()` draw is one choice. Hegel shrinks within the sampled branch and
  reports `4991`. Whether widening would be an improvement in Hegel depends on
  how libhegel encodes a string or integer draw: if a draw is several choices,
  the wide form is longer and the engine's answer is the shortlex minimum.
  (`tests/hypothesis_port/quality/test_widening_shrinks.py`; a `just()`
  branch never widens in Hypothesis either, as its encoding is shorter.)
