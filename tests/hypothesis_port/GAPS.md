# Differences from Hypothesis found while porting its test suite

Each entry: the Hypothesis behaviour, what Hegel does instead, and whether
the gap is in the compatibility layer (fixable here) or in the engine.

## Stateful, nocover and quality (agent C)

Files: `cover/test_stateful.py`, `nocover/*.py`, `quality/*.py`.

### Stateful (compat layer, mostly fixed here)

- **Rule-level `assume()`.** Hegel's `Machine` treats `AssumeFailed` inside a
  rule as "this rule is rejected, pick another"; Hypothesis aborts the whole
  test case. `hegel.hypothesis.stateful` now converts `UnsatisfiedAssumption`
  raised while drawing rule arguments or inside a rule body into an internal
  `_AbortCase` that aborts the run, so consumed bundle values and invariants
  stay consistent. Compat layer; fixed.
- **Bundle semantics.** Filtered bundles sample only among filter-passing
  values (Hypothesis `_BundleSampler`), filter-only transformations keep
  drawing references so names print as `v1`, consumption is immediate, and
  `Bundle.is_currently_empty` lets `lists(bundle)` yield `[]` instead of
  rejecting the case. Compat layer; fixed.
- **Falsifying example notes.** Hegel attaches one note `"Falsifying
  example:\n..."` where Hypothesis attaches `"Failing test case:"` followed by
  one note per line. Tests use a `_notes()` helper that flattens the note.
  Compat layer; left as is.
- **Flaky errors.** Hegel raises `hegel.errors.Flaky` with no
  `FlakyStrategyDefinition`/`FlakyFailure` subclass and no "precondition
  changed" note. Engine; xfailed (`test_flaky_precondition_error_message`).
- **`stateful_step_count` / `_min_steps`.** Hegel has no minimum step count.
  Engine; xfailed (`test_min_steps_argument`).
- **Shrinker tie-break.** `just([]) | lists(bundle)` shrinks to the
  `just([])` branch in Hegel so the printed name differs. Engine; xfailed
  (`test_doesnt_replace_when_different_id`).
- **FilterTooMuch message.** Health check text is `FailedHealthCheck:
  FilterTooMuch — ...`; tests match case-insensitively.
- **Database is a directory.** Tests count files under `_hegel_path` rather
  than reading `db.data`.

### nocover

Dropped as Hypothesis internals: `test_compat.py`, `test_fixtures.py`
(time-patching conftest fixture), `test_reusable_values.py`
(`has_reusable_values`), `test_database_agreement.py` and
`test_database_usage.py` (key/value database API: Hegel's database is an
opaque directory with no `save`/`fetch`/`move`), plus individual tests that
patched `ConjectureData`, `LazyStrategy`, `base_regex_strategy`,
`OneCharStringStrategy`, `note_deprecation`, `core.threadlocal`,
`SearchStrategy.branches`, `defines_strategy` and `from_typing_type`.

Compat-layer fixes made here:

- `from_type()`/`builds()` honour a class's explicit `__signature__` (dropping
  a spurious `self`), merge class and `__init__` type hints, generate
  annotated optional arguments as `just(default) | from_type(annotation)`,
  and skip annotations that are not types (`python/hegel/generators/from_type.py`).
- `.map/.filter/.flatmap` reprs use `__name__` rather than `__qualname__`,
  matching Hypothesis (`python/hegel/generators/base.py`).
- `find()` reports a search space filtered out by a health check as
  `NoSuchExample` (`python/hegel/hypothesis/core.py`).
- `settings.get_current_profile_name()` added.

Remaining gaps (xfailed, strict unless noted):

- **Eager validation.** Hegel validates strategy arguments at construction;
  Hypothesis defers until the test runs (`test_deferred_errors.py`, and
  `test_argument_validation.py`/`test_threading.py` build the strategy inside
  the `raises` block). Compat layer by design.
- **No `Unsatisfiable` when every example is rejected** and health checks are
  suppressed; Hegel passes silently. Engine
  (`test_given_error_conditions.py`, `test_testdecorators.py`).
- **`lists(nothing(), min_size=1)`** is reported as Unsatisfiable at run time
  rather than InvalidArgument.
- **No duplicate suppression**: the engine re-generates identical inputs and
  its flakiness detection trips on state-dependent tests
  (`test_duplication.py`). Engine.
- **No `data_too_large`/`large_base_example` health checks**
  (`test_health_checks.py`). Engine.
- **Targeting**: `target()` is accepted but no "Highest target score" is
  reported and generation is not steered toward the threshold
  (`test_targeting.py`, `quality/test_targeting_quality.py`). Engine.
- **Interesting origin**: failures are distinguished by exception type and
  location only, not by the cause chain (`test_interesting_origin.py`).
  Engine.
- **ExceptionGroups**: control exceptions (`AssumeFailed`, `InvalidArgument`)
  inside an `ExceptionGroup` are not unwrapped, and there is no
  `conjecture_data.mark_invalid()` (`test_exceptiongroup.py`). Compat layer.
- **`SystemExit` inside a strategy** propagates instead of being reported as
  Flaky, and no falsifying example is printed when `SystemExit`/`GeneratorExit`
  escapes the test (`test_baseexception.py`).
- **`characters(codec=...)`** supports only ascii, latin-1 and utf-8
  (`test_characters.py`). Compat layer.
- **`complex_numbers(width=32|64, min_magnitude=1.8)`** fails validation
  instead of rounding to the component width (`test_complex_numbers.py`).
- **`from_type` limits**: no `ForwardRef` resolution, non-runtime typing
  constructs (`Any`, `ClassVar`, ...) are accepted rather than rejected,
  `Callable[...]` fails with `TypeError`, `from_type(type)` is unsupported,
  `Collection[int]` never yields `bytes`, unresolvable local forward refs
  raise `NameError`, and `builds()` does not check generic arguments against
  registered subclasses (`test_type_lookup*.py`, `test_from_type_recipe.py`,
  `test_drypython_returns.py`). Compat layer.
- **`sampled_from`**: enums repr as a member list and the repr is not
  truncated; `Flag` enums only yield named members (no combinations) and an
  all-alias `Flag` is rejected as empty; `sets(sampled_from(range(50)),
  min_size=50)` fails FilterTooMuch instead of sampling without replacement;
  `sampled_from(range(2**25))` materialises the range (that parameter was
  dropped). `none()` reprs as `just(None)`. Compat layer / engine.
- **Distribution**: bounded `integers(0, 1e100)` draws far more huge values
  than Hypothesis (observed 680/1000 above 1e25); `uuids()` collides often
  (`test_integer_ranges.py`, `test_uuids.py`, non-strict where random).
- **`recursive()`/`deferred()` self-reference** (`st.recursive(st.deferred(lambda:
  ... | SELF), ...)` and a deferred `one_of` referring to itself) hits
  `RecursionError`; marked `xfail(run=False)` because the failure takes ~60s
  (`test_recursive.py`, `quality/test_deferred_strategies.py`,
  `quality/test_shrink_quality.py::test_calculator_benchmark`).
- **`from_regex`** builds non-matching strings for case-insensitive negated
  sets (`test_regex.py`, non-strict).
- **`NoSuchExample` message** does not include the lambda's source
  (`test_find.py`).
- **Skip replay** re-runs the replayed skipping example once more
  (`test_skipping.py`, assertion relaxed to `<= 2`).
- **Deadlines**: the shared test profile disables deadlines, so the threading
  tests set `deadline=200` explicitly. With that, Hegel raises
  `DeadlineExceeded` both in a thread and after concurrent runs.

### quality

Dropped as internals: `test_discovery_ability.py`, `test_poisoned_lists.py`,
`test_poisoned_trees.py`, `test_zig_zagging.py` (all drive `ConjectureRunner`).

- **Widening shrinks** (`quality/test_widening_shrinks.py`): Hegel's shrinker
  never rewrites a specific `one_of` branch into the wide branch, so
  `wide | just(needle)` shrinks to the needle rather than the minimal wide
  value. Engine; all 16 xfailed.
- **Lowering together** (`test_lowering_together_*`): `minimal()` often cannot
  find a pair of bounded integers with a large fixed gap within its budget,
  suggesting Hegel's bounded integer distribution is heavily weighted toward
  boundaries. Engine; non-strict xfail.
- **Shared empty list**: repeated draws of `lists(none(), max_size=0)` return
  the same list object (`test_multiple_empty_lists_are_independent`).
- Everything else in `test_shrink_quality.py` (84 tests), `test_float_shrinking.py`
  and `test_integers.py` passes unchanged.
