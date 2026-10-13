RELEASE_TYPE: minor

This release removes the `hegel.hypothesis` namespace, the reimplementation of
Hypothesis's API (`@given`, `strategies`, `settings`, `RuleBasedStateMachine`,
...) on top of Hegel, along with the pytest plugin's `--hypothesis-profile`,
`--hypothesis-seed` and `--hypothesis-verbosity` options and its `hypothesis`
marker. Write tests against the `hegel` API instead; `docs/comparison-with-hypothesis.md`
maps Hypothesis's API onto it. Hypothesis's own test suite now runs against the
`hegel` API directly.

It also fixes the bugs that porting that suite turned up:

- `lists`, `sets`, `frozensets` and `dicts` of a generator with no values
  (`nothing()`) always generate the empty collection instead of rejecting
  test cases; with `min_size > 0` they raise `InvalidArgument` when built, or
  when drawn if the element generator is only known to be empty then. Unique
  collections whose `just`, `booleans`, `sampled_from` or bounded `integers`
  elements have fewer distinct values than `min_size` are rejected when
  built. `iterables()` hands the test a one-shot iterator.
- `sampled_from` of a `Flag` class samples every named member (the zero
  member and aliases included) and the empty flag as well as combinations; a
  memberless `Enum` or `Flag` raises `InvalidArgument`. `sampled_from` of an
  enum reprs as `sampled_from(MyEnum)`, keeps the argument's sequence type
  in its repr, no longer materialises ranges, and copies lists so later
  changes to the argument do not affect the generator. `none()` reprs as
  `none()`; `characters()` and `text()` reprs show their arguments;
  `permutations`, `subsequences` and `samples` have reprs.
- `floats()` with an infinite bound on its own side (`min_value=-inf`,
  `max_value=inf`, also after `exclude_min`/`exclude_max`) no longer disables
  infinities, so `floats(min_value=-inf, max_value=0.0)` can generate `-inf`;
  an infinite bound with `allow_infinity=False` is rejected, and
  `allow_subnormal` is defaulted and validated after the exclusions are
  applied. `integers()` with one bound beyond `2**127` generates a spread of
  values instead of only the bound. `timedeltas()` draws days, seconds and
  microseconds separately, so deltas shrink towards whole days.
- `text("")` is a valid generator of only `""` (`text("", min_size=1)` is an
  error). `from_type` resolves bare `dict`, `deque` and `OrderedDict`; bare
  `list`, `set`, `frozenset` and `tuple` generate non-empty collections;
  `tuple[()]` generates only `()`; `Sequence[int]`, `Iterable[int]` and
  `Collection[int]` also generate `bytes`; an explicit `__signature__`
  outranks `__annotations__`; required positional-only constructor arguments
  are inferred and passed positionally; classes with an unhashable metaclass
  are accepted; a `Protocol` resolves through its subclasses and otherwise
  raises `ResolutionFailed` when built rather than failing when drawn; a
  `TypedDict` with unresolvable annotations is `ResolutionFailed` rather than
  a `NameError`.
- Bad arguments raise `InvalidArgument` where they used to raise
  `TypeError`, `ValueError`, `AttributeError`, `OverflowError` or
  `RecursionError`, or were silently accepted: non-numeric float bounds and
  non-bool `exclude_min`/`exclude_max`; non-collection or bytes alphabets
  for `text` and `from_regex`, any alphabet with a bytes pattern, and
  non-integer codepoint bounds; `ip_addresses` with `v` other than the
  integers 4 and 6 or a bytes network; `uuids` with a non-bool `allow_nil` or
  `allow_nil=True` together with a version; non-bool `allow_imaginary` and
  `no_cache`; `one_of` of a non-generator; `lists` with a non-callable
  `unique_by`; `fixed_dicts` with a non-dict mapping or `optional`;
  `register_type_generator` with a string key or an empty generator; a
  `deferred` generator defined as itself; `Settings` with a non-`Settings`
  parent, a non-string profile name or `test_cases >= 2**64`;
  `hegel.run`/`@hegel.test` with a non-`Settings` `settings`; `tc.target`
  and `tc.event_value` with a non-numeric score or non-string label;
  `hegel.composite` and `@hegel.test` on a class, a function with no
  parameters, or one whose test case parameter cannot be passed
  positionally.
- `hegel.MultipleFailures` implements `derive`, so `split`, `subgroup`,
  `except*` and `contextlib.suppress` keep its type. An exception group
  containing only Hegel's control-flow exceptions (an assumption failure in
  a `TaskGroup`, say) is handled as that exception instead of reported as a
  failure. Failures are distinguished by their cause chain and, for groups,
  their members, so the same `raise` reached through different underlying
  errors reports distinct bugs.
- Explicit test cases are reported in verbose output like generated ones,
  and with `report_multiple_failures=True` every explicit case runs and all
  distinct failures are reported together; an engine error after an
  explicit failure is chained onto it instead of hiding it. A test-runner
  skip raised inside an explicit case propagates unchanged.
