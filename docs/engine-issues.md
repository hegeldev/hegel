# Engine issues found while building Hegel for Python

Places where libhegel (`hegeltest-c` 0.45.13) behaves differently from what
Python's `re` module or Hypothesis would do. These are recorded here rather
than worked around in the bindings, so they can be fixed in the engine.

## Unicode tables

- libhegel's Unicode tables are Unicode 17.0, ahead of CPython's
  `unicodedata` (15.1 in 3.13, 16.0 in 3.14), so `\w` and `\d`, the
  categories a `characters()` or `text()` draw admits, and case folding
  follow assignments and reclassifications Python does not know yet. Seen
  in practice: `[^\W_]` drew U+323B0 (CJK Extension J), `^\w\Z` drew
  U+A7CE, `characters(categories=("Lu", "Nd"))` drew U+10D40 (Garay), and
  `text(exclude_categories=["Lu", "Ll"])` drew U+0295 `ʕ`, which Unicode 17
  reclassified from `Ll` to `Lo`.

  This is a correctness problem for users, not only for the test suite: on
  every Python we support, `from_regex(pattern)` can return a string that
  Python's own `re` says does not match `pattern`, and a category-filtered
  `characters()` or `text()` can return a character Python puts in an
  excluded category. Hypothesis never does either, because it generates from
  the host's tables. It is rare per draw, since the engine leans toward
  ASCII, but Unicode 17 added about 4,800 characters, most of them in CJK
  Extension J, so a test drawing many non-ASCII characters will meet one.
  Python 3.15 should ship Unicode 17 and close the gap for one release; it
  reopens with every engine table update, and every binding has the same
  problem against its own host (Go's `unicode` package and each JavaScript
  engine carry their own version).

  The fix belongs in the engine: a maximum Unicode version that character
  class expansion honours. Unicode publishes the version each code point was
  assigned in (DerivedAge.txt, a small range table), so given a cap the
  engine can exclude code points assigned after it from `\w`, `\d`,
  category sets, regex classes and case folding, which removes the
  "unassigned on the host" mismatches, nearly all of them. The binding
  supplies the host's version (Python's `unicodedata.unidata_version`,
  Go's `unicode.Version`; TypeScript would pick a conservative default), and
  exposing it as a setting lets a user pin it deliberately. Reclassifications
  between versions, such as U+0295, need the old tables rather than an age;
  they are a handful of characters per version and can stay documented as a
  remnant. Filtering in the bindings instead would cost draws, distort the
  distribution and shrinking, and need redoing per binding, so it is not
  done here.

  Until then, whether a run meets such a character is down to chance, so the
  affected tests are skipped while this Python's
  `unicodedata.unidata_version` is older (`tests/helpers.py`,
  `UNICODE_NEWER_THAN_PYTHON` and `skip_if_unicode_newer_than_python`, used
  by `tests/test_generators_strings.py` and, in the port,
  `cover/test_regex.py` and `cover/test_simple_characters.py`).

## Regular expressions

- Python 3.14 added `\z` to `re`, an end-of-string anchor equivalent to
  `\Z` and the spelling most other engines use. libhegel rejects it as a bad
  escape (`invalid regex pattern "a\z": bad escape \z at position 1`), so
  `from_regex` fails for a pattern `re` compiles
  (`tests/hypothesis_port/cover/test_regex.py::test_end_of_string_anchor_z`,
  xfail on 3.14 and later; the fuzz test in `nocover/test_regex.py` assumes
  such patterns away).

## Shrinking and generation

- A float draw with an infinite bound and `allow_infinity=false` aborts the
  run with an `InternalError` (`FloatChoice::simplest: no valid float for
  this choice`) at width 64, and at width 32 clamps the bound to the largest
  finite value and then rejects it as exceeding the other bound. The
  bindings now reject that combination as an invalid argument before it
  reaches the engine, so this is only reachable from other bindings.

## Collections and repeat

- `repeat` runs a body "logically forever", until the test case fails or
  overruns. Both the Python and Rust bindings implement it by drawing a
  repetition count from `[0, 2^40]` and opening a collection with that
  minimum size, so the engine forces `more()` to continue until the count
  is reached. Each forced continue is a recorded choice, retaining about
  100 bytes, and counts toward the 2^20 choice limit, which is what ends the
  loop in practice. The consequences for a body that draws nothing (a
  repeated observation, say):

  - every test case whose drawn count exceeds 2^20 spins for about a
    million iterations, each with its `// Repetition #N` note and block,
    before overrunning, and overruns are invalid, so most cases are wasted;
    only the cases whose drawn count is small are valid. In Python that is
    about 0.3 s per case, and several seconds per case under coverage on a
    CI runner, which failed `tests/test_test_case.py::test_repeat_can_run_zero_times`
    with `too_slow` (only 1 to 3 valid inputs after 30 s). That test now
    stops the collection itself rather than running the engine's loop;
  - with the `test_cases_too_large` health check suppressed, which
    `HealthCheck.all()` does, the choice limit goes too (see
    `hegel_settings_set_unbounded_choices` in `hegel.h`), so the loop runs to
    the drawn count: hours, and at 100 bytes per step hundreds of gigabytes.
    A probe written that way took about 200 GB before it was killed.

  A body that draws is bounded the same way, by the choice limit rather than
  by anything `repeat` decides, so a long body still runs up to a million
  times per case.

  What would fix it in the engine: a repeat protocol of its own, like the
  state machine's step count, where the engine decides when to stop
  (bounded by the choice budget, with a forced continue not retained as a
  choice node), or at least a collection whose forced continues are not
  retained. Separately, suppressing `test_cases_too_large` silently removing
  the choice limit is a footgun for anyone who suppresses health checks
  wholesale; a limit that stays unless lifted explicitly would be safer.

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
