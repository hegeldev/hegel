# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.45.13) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

## Regular expressions

- libhegel's Unicode tables are Unicode 17.0, ahead of CPython's
  `unicodedata` (15.1 in 3.13, 16.0 in 3.14), so `\w`, the categories a
  `characters()` or `text()` draw admits, and case folding follow
  assignments and reclassifications Python does not know yet: `[^\W_]`
  drew U+323B0 (CJK Extension J), `characters(categories=("Lu", "Nd"))`
  drew U+10D40 (Garay), and `text(exclude_categories=["Lu", "Ll"])` drew
  U+0295 `ʕ`, which Unicode 17 reclassified from `Ll` to `Lo`. Not an
  engine bug so much as a version gap, which closes as Python catches up;
  whether a run meets such a character is down to chance, so the affected
  tests are skipped while this Python's `unicodedata.unidata_version` is
  older (`tests/helpers.py`, `UNICODE_NEWER_THAN_PYTHON`).

## Shrinking and generation

- A float draw with an infinite bound and `allow_infinity=false` aborts the
  run with an `InternalError` (`FloatChoice::simplest: no valid float for
  this choice`) at width 64, and at width 32 clamps the bound to the largest
  finite value and then rejects it as exceeding the other bound. The
  bindings now reject that combination as an invalid argument before it
  reaches the engine, so this is only reachable from other bindings.

## Stateful testing

- There is no minimum step count for a state machine run: the engine decides
  how many steps to take, so Hypothesis's `_min_steps` is rejected with
  `InvalidArgument` rather than honoured.

## Messages and reporting

- The `Unsatisfiable` message does not name the test it is about, though the
  engine is given the test's location; Hypothesis's names the function
  (`tests/hypothesis_port/nocover/test_testdecorators.py`).

## Other

- Recursive generators rarely reach some shapes: for `deferred` JSON-like
  data, a dict with key `""` holding a list is almost never generated within
  the budget Hypothesis finds it in
  (`tests/hypothesis_port/quality/test_deferred_strategies.py`).
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
