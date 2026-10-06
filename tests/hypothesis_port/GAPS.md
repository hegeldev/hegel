# Differences from Hypothesis found while porting its test suite

Each entry: the Hypothesis behaviour, what Hegel does instead, and whether
the gap is in the compatibility layer (fixable here) or in the engine.

## Strategies (agent A)

Files ported: test_direct_strategies, test_simple_collections, test_simple_strings,
test_simple_characters, test_numerics, test_float_nastiness, test_subnormal_floats,
test_regex, test_sampled_from, test_one_of, test_nothing, test_recursive,
test_deferred_strategies, test_lookup (+ py38/py39/py310/py314), test_type_lookup,
test_type_lookup_forward_ref, test_typealias_py312, test_annotations, test_slices,
test_uuids, test_permutations, test_functions, test_randoms, test_random_module,
test_complex_numbers, test_datetimes, test_provisional_strategies, test_searchstrategy,
test_custom_reprs and datetime/test_zoneinfo_timezones. Dropped as internals-only:
test_pretty, test_charmap, test_invert, test_filtered_strategy, plus individual tests
that drive ConjectureData, FilteredStrategy, JustStrategy, to_jsonable,
DatetimeStrategy's transition probing, Hypothesis's entropy module or
define_function_signature.

- **from_type coverage (engine/compat).** Hypothesis resolves every builtin type
  (exceptions, range, slice, memoryview, ...), typing special forms (Final, Annotated
  metadata, Callable[..., X], Type[...], Protocols, ItemsView/KeysView, re.Pattern,
  TypeVar factories, NewType conditionally) and forward references found in caller
  frames. Hegel's from_type knows builtin value types, generics it has a generator
  for, enums, dataclasses, TypedDicts and annotated constructors; everything else
  raises ResolutionFailed. Most of test_lookup* and test_type_lookup is xfailed on this.
- **from_type is eager (engine).** Recursive and mutually recursive types (dataclass
  trees, self-referential NamedTuples, `type Tree = int | list[Tree]`) recurse without
  bound instead of being deferred; Hypothesis builds them lazily via `deferred()`.
- **Resolved strategies are cached (compat).** `from_type(Abstract)` keeps the strategy
  it first resolved, so registering/unregistering a subclass afterwards has no effect,
  and `repr(from_type(int))` is `from_type(<class 'int'>)` rather than `integers()`.
- **Timezone-aware datetime bounds (engine).** Hypothesis accepts aware `min_value` /
  `max_value` and compares them as instants, targets DST folds/gaps and leap seconds
  ("tricky" draws), and generates `fold=1`. Hegel's datetimes()/times() reject aware
  bounds and never set fold. `dates(x, x)`/`timedeltas(x, x)` do not return the bound
  object itself, and timedeltas shrink by component rather than to whole days.
- **from_regex flags and classes (engine).** re.ASCII, re.IGNORECASE on classes,
  re.DOTALL, re.MULTILINE insertion around ^/$, bytes `\w`/`\s`/`\d` classes,
  negative lookaheads, inline-flag scoping and alphabet-incompatible charsets are not
  honoured by Hegel's regex generator; the alphabet is also not checked against the
  pattern's codec (no warning).
- **randoms() (engine).** Hegel returns seeded `random.Random` subclasses
  (`HypothesisRandom` / `TrueRandom` with `note_method_calls`), not Hypothesis's
  ArtificialRandom whose method results are drawn from the test data. Exact boundary
  values (random() == 0, betavariate == 1, randrange(100, step=3)) cannot be targeted,
  and `random_module()` draws its seed so two draws can coincide after shrinking.
- **register_random is a no-op (engine).** Hegel does not manage registered PRNG state
  across test cases; only argument validation is kept.
- **sampled_from(...).filter() (engine, fixed on main).** Tests relying on the filtered
  sample searching untried elements are lenient xfails pending the merge of
  `FilteredSample`; `sampled_from(dataclass)` gives no "annotations" hint.
- **Reprs (compat).** Hegel's Characters generator does not repr its arguments
  (`characters()` for every configuration); sampled_from is built eagerly so its repr
  shows the copied tuple; the call that created a value is printed even when distinct
  calls returned the same object; failing examples print `name = value` lines rather
  than Hypothesis's `test(name=value)` form; six snapshot-based repr tests need syrupy.
- **Lambda descriptions (compat).** Lambdas with positional-only, keyword-only or
  variadic parameters are described as `<lambda>`; `@composite` does not rewrite the
  return annotation to `SearchStrategy[...]`.
- **Health checks vs Unsatisfiable (engine).** `check_can_generate_examples` of a fully
  filtered strategy raises FailedHealthCheck (FilterTooMuch), where Hypothesis raises
  Unsatisfiable from `.example()`.
- **Misc.** `builds()` TypeErrors mentioning "SearchStrategy" get no explanatory note;
  ambiguous forward references across frames are not warned about;
  `from_type(Sequence[int])` is lists only (never bytes), and Hashable is not
  restricted to hashable values.
