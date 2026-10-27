RELEASE_TYPE: minor

This release brings Hegel for C++ into line with the other Hegel frontends: `hegel::Settings` now combine with the engine's settings profiles instead of overriding them, run errors and failure reports match hegel-rust's, every reproduce blob is replayed, output goes through libhegel's printer, generators print the values they draw, and it adds explicit test cases, several generators and the rest of the engine's settings.

### Settings

A `Settings` field that you leave unset now keeps the value of the settings profile (`hegel.toml`, `HEGEL_DEFAULT_PROFILE`, the `HEGEL_*` variables, or the `ci`, `workload` and `development` profiles the engine picks itself). Before, `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness`, `phases` and the seed were always set to Hegel for C++'s own defaults, which overrode the profile, and `print_blob` was never sent to the engine at all. Hegel for C++ no longer detects CI itself; the engine's `ci` profile does it. To support this:

- `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness` and `print_blob` are now `std::optional`. Code that reads these fields from a `Settings` must handle the unset case; code that sets them does not change.
- `suppress_health_check` and `phases` are now `hegel::ListSetting`, which is unset by default. A braced list sets them as before, and an empty list (`.suppress_health_check = {}`) now suppresses no health checks instead of keeping the profile's set.
- `Database::unset()` (the default) now keeps the profile's database, and the new `Database::default_location()` selects `.hegel/examples` even where the profile disables the database.

The new `Settings::show_statistics` and `Settings::unbounded_choices` reach the engine's settings of those names, `Settings::profile`, `hegel::register_profile()` and `hegel::set_default_profile()` select and define settings profiles from C++, and `hegel::engine_version()` returns the version of the bundled libhegel.

`hegel::test()` now derives its database key (`<file>::<name>`) also when the database is disabled, as `HEGEL_TEST` already did, because a derandomized run derives its seed from the key. Without it, every derandomized test under the `ci` profile saw the same inputs.

### Run errors and failure reports

A run that stops without a verdict now throws `hegel::FailedHealthCheck` (with the check that failed), `hegel::Flaky`, `hegel::Unsatisfiable`, or else `hegel::RunError`, their base class, instead of a plain `std::runtime_error` with a `Hegel run error:` prefix. A run that reports several failures throws `hegel::MultipleFailures`, which holds each failure's exception and whose message gives each one's message. All of these still derive from `std::runtime_error`.

The failure report now has the same layout as hegel-rust's. It no longer has a `--- Failure: ... ---` header, a `Falsified after N test cases` count, an `Exception:` line or `Failure i of N:` headings: it prints each failure's drawn values and notes, a `note:` line when the engine qualifies how reliably the failure reproduced, and how to replay the failure, and the test's own exception propagates as before. Several failures are introduced with `Property-based test failed with N distinct failures.`. With `Verbosity::Quiet`, only the lines that replay the failure print. Each failure is reported from the run's own execution of it, so the test body no longer runs once more per failure after the run.

```
# before
--- Failure: below_50 (my_test.cpp:3) ----------------------------------
Falsified after 3 test cases (0 discarded):

  auto n = 50;

Exception: std::runtime_error: n should be below 50
rerun with: HEGEL_REPRODUCE_FAILURE(below_50, "AAEAAAAACgEAAAAy")

# after
auto n = 50;

To reproduce this failure, add this line above HEGEL_TEST(below_50):
    HEGEL_REPRODUCE_FAILURE(below_50, "AAEAAAAACgEAAAAy")
```

Every blob given to `HEGEL_REPRODUCE_FAILURE` or to `hegel::test()` is now replayed, not only the first, and a nondeterministic failure's blob is replayed until it fails. A blob replay prints the same failure report as a generated failure. If no blob makes the test fail, `hegel::test()` throws `hegel::DidNotReproduce`, and an invalid blob throws `std::invalid_argument`.

### Explicit test cases

Explicit test cases run a test with values you choose, before Hegel generates any, when the `Explicit` phase is enabled. Add one to a `HEGEL_TEST` with `HEGEL_EXPLICIT_TEST_CASE(name, values...)`, or pass a list of `hegel::ExplicitTestCase` to `hegel::test()` as its new `explicit_cases` argument, after `failure_blobs`. Positional values go to the test's draws in order, and `hegel::named("d_1", value)` gives a value to the draw printed as `d_1`, the first `tc.draw("d", ...)`:

```cpp
HEGEL_EXPLICIT_TEST_CASE(division, 7, hegel::named("d_1", 0))
HEGEL_TEST(division)(hegel::TestCase& tc) {
    auto n = tc.draw(gs::integers<int>());
    auto d = tc.draw("d", gs::integers<int>());
    // ...
}
```

A failing explicit test case prints what it drew and noted (nothing under `Verbosity::Quiet`), and its exception propagates. With `report_multiple_failures` on it does not end the run: every explicit test case runs, the generated test cases follow, and every distinct failure is reported together, under `Property-based test failed with N distinct failures.`, as a `hegel::MultipleFailures`, the explicit test cases' failures first. A generated failure with the same origin as an explicit one is the same bug, and is dropped; a run error after an explicit failure (a failed health check, say) is reported after it as one more failure rather than hiding it. A misuse of an explicit test case is a usage error, which makes `hegel::test()` throw `std::invalid_argument` without reporting a failure: a rejection (its message begins `Explicit test cases must not be rejected`), a draw with no value or a value of another type, a value that no draw takes, and anything that needs generated data other than a draw, such as `TestCase::draw_silent()` (`draw_silent is not supported in explicit test cases`) or stateful testing. `TestCase::repeat()` runs its body once, so a rejection inside it is reported like any other. `TestCase::clone()` draws the test case's values, in sequence with the test case it was cloned from, so a test that draws from several threads runs in an explicit test case too, and `TestCase::set_worker()` does nothing there.

### Output

Notes and drawn values now print through libhegel's printer instead of Hegel for C++'s own output code, which changes how they look:

- A drawn value too wide for one line is broken across lines, one element per line, instead of printing as one long line.
- A clone's output appears where the clone was made (`tc.clone()` or `tc.spawn()`), not interleaved with the parent's in the order the lines were written, and a clone's draws continue the parent's draw names (`draw_1`, `draw_2`, ...) instead of starting again at `draw_1`.
- In a failure report of `hegel::stateful::run_concurrent()`, each worker's lines appear under the header of the round they ran in.
- A drawn `hegel::generators::randoms()` engine prints the values it handed out to the test, as `HegelRandom{.consumed = {...}}`, or its seed with `use_true_random`.

The new `TestCase::set_worker()` attributes what a test case prints to a worker thread, which prefixes each of its lines with `[worker N +X.XXXms]`, as `run_concurrent()` does for its workers:

```cpp
auto worker = tc.clone();
worker.set_worker(1);
```

### Printing drawn values

A generator now prints each value as it draws it, as in hegel-rust, instead of Hegel for C++ printing the finished value. A container prints each element as the element's generator prints it, so printing that you give one generator shows inside every value built from it. This makes the following changes, some of them breaking:

- `TestCase::draw()` takes a printable generator, `hegel::generators::Generator<T>`. Every built-in generator of a type with a C++ representation (a `print_to(hegel::Printer&) const` member, an `operator<<`, or a reflectable aggregate) is one. A generator that cannot print is the new `hegel::generators::AnyGenerator<T>`, which `Generator<T>` derives from. Drawing one with `tc.draw()` does not compile, and the error says how to fix it. `map()` to a type with no representation, `just()` and `sampled_from()` of one, `compose()` that returns one, and a collection or combinator of an `AnyGenerator` give an `AnyGenerator`. To make one printable, use `.print_with(f)` (prints each value with `f(value, printer)`), `.print_as_repr()`, or `.print_as_call("f")` on a `map()` of a printable generator, which prints `f(<input>)`:

  ```cpp
  auto keys = gs::integers<int>()
                  .map([](int raw) { return KeyData::from_ffi(raw); })
                  .print_as_call("KeyData::from_ffi");
  // auto key_1 = KeyData::from_ffi(3);
  ```

- The new `TestCase::draw_silent()` draws from any generator and prints nothing, and `TestCase::draw_and_print()` draws from a printable generator inside another generator's printing. Both are usage errors in an explicit test case.
- `map()` now returns a `hegel::generators::Mapped<U, SourcePrintable>`, which derives from `Generator<U>` (or from `AnyGenerator<U>`) and has `print_as_call()`. It prints the value it made. Code that names the type `Generator<U>` still compiles when `U` has a representation.
- `IGenerator<T>` is now documented, and the new `IPrintableGenerator<T>` is the interface of a printable generator: implement `do_draw_and_print(tc, printer)`, and wrap it with `make_generator<Impl>()`. A `Generator<T>` can no longer be made from an implementation of `IGenerator<T>` only; make it an `AnyGenerator<T>` or implement `IPrintableGenerator<T>`.
- `hegel::Printer`, `hegel::Speculation` and `hegel::Document` are the new public printer: text, break points, groups, `seq()` for a list, `repr()` for a value's representation, speculative regions, and `deferred()` for a part of the line that is filled in later. They replace the undocumented printer types in `hegel::internal`. A type's `print_to()` member now takes a `hegel::Printer&`. `hegel::is_representable_v<T>` says whether a type has a C++ representation, and `label_from_name()` and `combine_labels()` are public, for generators of your own.
- `HegelRandom::print_to()` is removed. A `randoms()` generator prints what its engine hands out.
- `deferred<T>()` is printable and its `set()` takes a `Generator<T>`; the new `deferred_silent<T>()` takes any generator. `recursive()` is printable when its leaf and the generators its branch function returns are, and `flat_map()` when its function returns a printable generator.
- `sets()` and `maps()` print their elements in the order they were drawn, not in sorted order: `std::set<int>{9, 8}`.
- `builds<T>()` prints a constructor call, `T(<arg>, <arg>)`, with each argument as its generator prints it.
- A `default_generator<T>().override(...)` struct draws each overridden field once, from the override. Before, it drew the field from its default generator and then again from the override. Database entries saved by earlier versions may no longer replay.
- A `filter()` of a `filter()` checks both predicates on each attempt, instead of retrying the inner filter for each attempt of the outer one.
- `LarkParams::explicit_terminals` holds `AnyGenerator<std::string>` values. A `Generator<std::string>` converts to one.

### Generators

This release adds:

- `TestCase::event()` and `TestCase::event_value()`, which record events and numeric observations for the end-of-run statistics report that `Settings::show_statistics` turns on.
- `hegel::generators::none()`, which generates `std::monostate{}`.
- `hegel::generators::permutations()`, which generates reorderings of a list of values.
- `hegel::generators::subsequences()`, which generates sublists of a list of values that keep its order, and `hegel::generators::samples()`, which generates lists of values chosen from it, with replacement by default or each value at most once with `{.replacement = false}`. Both take `min_size` and `max_size`, and shrink toward fewer values from earlier in the list.
- `hegel::generators::weighted_booleans(p)`, which generates `true` with probability `p`, between 0 and 1.
- A `from_regex(pattern, RegexParams)` overload whose `alphabet` restricts every character of the generated strings, wildcards and the padding around a partial match included, to those a `CharactersParams` describes: `from_regex("a.c", {.alphabet = CharactersParams{.codec = "ascii"}})`.
- A `vectors()` overload that takes a key function, `vectors(elements, params, unique_by)`: no two elements of a generated vector have equal keys.
- `hegel::generators::recursive()`, which generates recursively defined values, such as trees, through libhegel's recursion support: the engine decides where a value branches and how large it grows, within `RecursiveParams::max_depth` and `RecursiveParams::max_leaves`.
- `hegel::generators::from_lark()`, which generates strings of a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library. `LarkParams` selects the start rule, gives explicit terminals whose values come from your own generators, restricts the characters of the grammar's own terminals with an `alphabet`, limits the size and nesting depth of the strings, and sets the directory that `%import`ed grammar files are read from.
- Support for integer types wider than 64 bits in `integers()`, such as `__int128` and `unsigned __int128`. Before, their bounds were truncated to 64 bits.

`default_generator<T>()` now rejects a struct that refers back to itself through its fields, directly or through other structs and containers, such as `struct Node { int value; std::vector<Node> children; };`. Drawing such a generator recursed without bound; it now fails as a usage error naming the struct and the field, and pointing at `recursive()`, `deferred<T>()` and `.override(field<&Node::children>(...))`, which remain the ways to generate it.

Generators now identify themselves to the engine with the same span labels as the other Hegel frontends: each label comes from a `hegel.<kind>` name (`hegel.lists`, `hegel.one_of`, `hegel.integers`, ...), and a generator built from others combines its own label with theirs. Before, every `vectors()` span had the same label whatever its elements were, so the shrinker treated a vector of integers and a vector of strings as interchangeable. Database entries saved by earlier versions may no longer replay.

Which draws a span groups has changed too, to match the other Hegel frontends. Every draw made through the test case (`TestCase::draw()`, `draw_silent()` and `draw_and_print()`) now runs its generator inside a span labelled with the generator's label, and the generators open none of their own: `vectors()`, `sets()`, `maps()`, `tuples()`, `arrays()`, `one_of()`, `optional()`, `variants()`, `builds()`, `compose()`, `recursive()`, `default_generator<T>()` of a struct, `map()` and `flat_map()` used to open a span of their own around their draws. A generator of your own that draws its components through `tc.draw_silent()` or `tc.draw_and_print()` gets a span for each of them with no code of its own, and `IGenerator<T>::label()` now defaults to a label derived from the name of the generator's type (`typeid(*this).name()`), so two generators of your own shrink apart, instead of the one `hegel.generator` label for all of them (`hegel::internal::LabelName::Generator` is removed). The spans a test case records are part of its failure blob, so a blob printed by an earlier version, or a failure the database saved under one, no longer replays.

`text()` and `binary()` with no `max_size` now take libhegel's default maximum, the same as every other Hegel frontend: 100, or `min_size + 100` when `min_size` is above 100. Before, a `min_size` above 100 with no `max_size` made every value exactly `min_size` long. A `max_size` too large for any allocation (such as `SIZE_MAX`) now means no upper bound. `text({.alphabet = ...})` now generates only the characters of the alphabet, and `characters({.categories = {}})` allows no category. Before, an empty `categories` list was sent to libhegel as no list, so these generated characters from all of Unicode.

`floats<long double>()` now draws a `double`. Before, it failed with an engine error where `long double` is wider than 64 bits. There, a bound that is not a `double` is moved inward to the nearest `double`, so every value it generates is within the bounds you gave.

A `filter()` on `hegel::stateful::values_consumed()` now removes only the value it returns from the pool. Before, every value its predicate rejected was also removed, so later draws could not use it, and the same happened to every value it had tried when the predicate threw.

### Draw names

Each use of a name in `tc.draw("x", ...)` now prints with a 1-based suffix (`auto x_1 = ...;`, then `x_2`, ...), as in every other Hegel frontend, instead of the first use printing bare and a reused name printing bare again. Each name counts on its own, and an unnamed draw counts under `draw`, so the first draw under a name is always `_1`. The `repeatable` parameter of `TestCase::draw()` is removed: every name is repeatable. `hegel::named()` takes the name a draw prints under, so an explicit test case for the test above gives `d_1` its value where it gave `d` before:

```cpp
// before
auto d = tc.draw("d", gs::integers<int>());           // auto d = 0;
auto x = tc.draw("x", gs::integers<int>(), true);     // auto x_1 = 0;
HEGEL_EXPLICIT_TEST_CASE(division, hegel::named("d", 0))

// after
auto d = tc.draw("d", gs::integers<int>());           // auto d_1 = 0;
auto x = tc.draw("x", gs::integers<int>());           // auto x_1 = 0;
HEGEL_EXPLICIT_TEST_CASE(division, hegel::named("d_1", 0))
```

### Stateful testing

A `Rule` or `ConcurrentRule` can now carry a weight, as in the other Hegel frontends: pass it as a trailing constructor argument (`Rule<M>("push", step, 3.0)`), and the engine offers the rule that many times as often as a rule of weight 1. A weight must be finite and positive; the engine rejects any other value with `std::invalid_argument`. An `Invariant` can be declared always checked (`Invariant<M>("sorted", check, true)`), in which case it runs at every join point instead of being sampled between steps.

The step budget moves from `Settings::stateful_step_count` to `stateful::RunParams::step_count` (default 50), so it is set per machine rather than per run: `hegel::stateful::run(machine, tc, {.step_count = 10})`, and `run_concurrent` takes the same `RunParams` as a trailing argument. `Settings::stateful_step_count` is removed; `run` and `run_concurrent` reject a step count below 1 with `std::invalid_argument`.

A failing stateful test's report now reads as it does in the other Hegel frontends. Each step prints as `Step 1: add {` with its draws and notes indented beneath it and a closing `}` (a concurrent worker's as `Rule: add {`), and the rule or invariant the failure escaped from is named right before the failure (`Rule add failed:`, `Invariant bounded failed:`) instead of `Invariant bounded violated after step 2`. The invariant checks on the initial and final state are announced as `Checking invariants on the initial state.` and `Checking invariants on the final state.`, and only when the machine has invariants. State printing is removed: the `state = …` lines and `RunParams::print_state` are gone, so a machine's state no longer needs to be printable. Note what matters from a rule or invariant instead.

### Removals and corrections

The undocumented `hegel::verbosity_to_string()` and `hegel::health_check_to_string()` are removed.

The 0.13.2 changelog entry says the Nix flake was removed. That is not correct: `nix/flake.nix` is still available and still exports `lib.mkHegelCppProject`. It now builds the in-tree libhegel instead of downloading one.

With `Verbosity::Verbose` or `Verbosity::Debug`, a test case that stops short now says why, as the other Hegel frontends do: `Test case stopped: failed assumption` or `Test case stopped: out of data`. An explicit test case is no longer introduced with a `Running explicit test case` line: it prints its drawn values and notes like a generated one.

This release also replaces the `exclude_min` and `exclude_max` flags of `FloatsParams` with `min_value_exclusive` and `max_value_exclusive`, which take the bound directly, as the other Hegel frontends do:

```cpp
// before
gs::floats<double>({.min_value = 0.0, .max_value = 1.0, .exclude_min = true, .exclude_max = true})

// after
gs::floats<double>({.min_value_exclusive = 0.0, .max_value_exclusive = 1.0})
```

Giving both `min_value` and `min_value_exclusive` (or both `max_value` and `max_value_exclusive`) is a `std::invalid_argument`. `FloatsParams` also gains `allow_subnormal`: subnormal ("denormalised") values are drawn whenever the bounds admit any, as before, and `.allow_subnormal = false` keeps them out, for code that may run with flush-to-zero floating point (compiled with `-ffast-math`, say). Asking for subnormals with bounds that exclude them all, or excluding them from a range that holds nothing else, is a `std::invalid_argument`.

### `TestCase::repeat()`

`TestCase::repeat()` now has the engine decide how many repetitions run and which one failed, through libhegel's new `hegel_repeat_t` functions, as the other Hegel frontends do, rather than drawing a loop length of its own. Each repetition's draws and notes now print indented under its `// Repetition #N` note, and a failing repetition is marked for the engine before its exception propagates, so a failure shrinks to the fewest repetitions. The loop makes different choices from before (its minimum length is drawn from a larger range), so a reproduce blob from an earlier release that went through `repeat()` no longer replays.
