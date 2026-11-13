# Changelog

## 0.17.4 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.4.

## 0.17.3 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.3.

## 0.17.2 - 2026-11-13

This release updates the bundled libhegel engine to 0.51.2.

## 0.17.1 - 2026-11-10

This release updates the bundled libhegel engine to 0.51.1.

## 0.17.0 - 2026-11-10

This release brings Hegel for C++ into line with the other Hegel frontends: `hegel::Settings` now combine with the engine's settings profiles instead of overriding them, run errors and failure reports match hegel-rust's, every reproduce blob is replayed, output goes through libhegel's printer, generators print the values they draw, and it adds explicit test cases, several generators and the rest of the engine's settings.

### Settings

A `Settings` now holds a value for every setting, resolved by the engine when the `Settings` is created: from its settings profile (`hegel.toml`, `HEGEL_DEFAULT_PROFILE`, or the `ci`, `workload` and `development` profiles the engine picks itself) with the `HEGEL_*` variables applied over it. A field you set replaces the resolved value, and a field you leave alone keeps it. Earlier releases set `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness`, `phases` and the seed to Hegel for C++'s own defaults, which overrode the profile, and never sent `print_blob` to the engine; the last release made those fields `std::optional` and added the `hegel::Phases` type so that an unset field would take the profile's value. Every field now holds the resolved value itself: `test_cases`, `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness` and `print_blob` are plain values rather than `std::optional` (only `seed` and `database_key` stay optional, since a profile may have neither), and `phases` is a `std::vector<Phase>`, so `hegel::Phases` is gone. Hegel for C++ no longer detects CI itself; the engine's `ci` profile does it. To support this:

- `Settings::from_profile(name)` resolves the named profile instead of the default one, and replaces the `profile` field. Adjust the result field by field. A designated initializer always resolves the default profile.
- `suppress_health_check` and `phases` are `std::vector`s holding the resolved lists. A braced list sets them as before, and an empty list (`.suppress_health_check = {}`) now suppresses no health checks instead of keeping the profile's set.
- `database` holds the resolved database, so `Database::unset()` is gone; the new `Database::default_location()` selects `.hegel/examples` even where the profile disables the database. `Backend::Auto` is gone too: `backend` holds the resolved backend.
- The values are fixed when the `Settings` is created. `hegel::register_profile()` and `hegel::set_default_profile()` do not change a `Settings` created before them, and an unknown profile or a malformed `hegel.toml` or `HEGEL_*` variable throws `std::invalid_argument` from the creation rather than from `hegel::test()`.

```cpp
hegel::Settings nightly = hegel::Settings::from_profile("ci");
nightly.test_cases = 10000;
hegel::register_profile("nightly", nightly);
hegel::test(body, hegel::Settings::from_profile("nightly"));
```

The new `Settings::show_statistics` and `Settings::unbounded_choices` reach the engine's settings of those names, `Settings::from_profile()`, `hegel::register_profile()` and `hegel::set_default_profile()` select and define settings profiles from C++, and `hegel::engine_version()` returns the version of the bundled libhegel.

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

A program defined by `HEGEL_MAIN` reports a failure the same way, ending with `To reproduce this failure, run the program with:` and the `--failure-blob` option, then an `Exception:` line naming the test case's exception, which the program has no caller to propagate to. The program's inline settings are created after its `--profile` option applies, so they take that profile's values, and its `--failure-blob` is replayed along with the blobs `HEGEL_REPRODUCE_FAILURE` registers.

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

A failing explicit test case prints what it drew and noted (nothing under `Verbosity::Quiet`), and its exception propagates. With `report_multiple_failures` on it does not end the run: every explicit test case runs, the generated test cases follow, and every distinct failure is reported together, under `Property-based test failed with N distinct failures.`, as a `hegel::MultipleFailures`, the explicit test cases' failures first. A generated failure with the same origin as an explicit one is the same bug, and is dropped; a run error after an explicit failure (a failed health check, say) is reported after it as one more failure rather than hiding it. A misuse of an explicit test case is a usage error, which makes `hegel::test()` throw `std::invalid_argument` without reporting a failure: a rejection (its message begins `Explicit test cases must not be rejected`), a draw with no value or a value of another type, a value that no draw takes, and anything that needs generated data other than a draw, such as `TestCase::draw_silent()` (`draw_silent is not supported in explicit test cases`) or stateful testing. `TestCase::repeat()` runs its body once and then ends the test case, so a rejection inside it is reported like any other. `TestCase::clone()` draws the test case's values, in sequence with the test case it was cloned from, so a test that draws from several threads runs in an explicit test case too, and `TestCase::set_worker()` does nothing there.

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

The step budget moves from `Settings::stateful_step_count` to `stateful::RunParams::step_count` (default 50), so it is set per machine rather than per run: `hegel::stateful::run(machine, tc, {.step_count = 10})`, and `run_concurrent` takes the same `RunParams` as a trailing argument. `Settings::stateful_step_count` is removed; `run` and `run_concurrent` reject a step count below 1 with `std::invalid_argument`.

A failing stateful test's report now reads as it does in the other Hegel frontends. Each step prints as `Step 1: add {` with its draws and notes indented beneath it and a closing `}` (a concurrent worker's as `Rule: add {`), and the rule or invariant the failure escaped from is named right before the failure (`Rule add failed:`, `Invariant bounded failed:`) instead of `Invariant bounded violated after step 2`. The invariant checks on the initial and final state are announced as `Checking invariants on the initial state.` and `Checking invariants on the final state.`, and only when the machine has invariants. State printing is removed: the `state = …` lines and `RunParams::print_state` are gone, so a machine's state no longer needs to be printable. Note what matters from a rule or invariant instead.

### Removals and corrections

The undocumented `hegel::verbosity_to_string()` and `hegel::health_check_to_string()` are removed.

`TestCase::clone()` on a test case the engine has stopped now ends the test case, as a draw does. Before, it threw a `std::runtime_error` reading `engine stopped test`, which was reported as a failure of the test.

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

`TestCase::repeat()` also no longer returns, as in every other Hegel frontend: the loop is logically infinite, and when the engine ends it the test case ends as passing, so code after the call never runs. A test that did something after its loop now does it in the body, on a later repetition:

```cpp
// before
tc.repeat([&] { stack.push(tc.draw(gs::integers<int>())); });
EXPECT_EQ(stack.size(), stack.pushed());

// after
tc.repeat([&] {
    stack.push(tc.draw(gs::integers<int>()));
    EXPECT_EQ(stack.size(), stack.pushed());
});
```

In an explicit test case it still runs its body once, and then ends the test case the same way.

## 0.16.0 - 2026-11-09

This release fixes `hegel::Settings` overriding the engine's settings profile. A field you do not set now takes its value from the profile: `HEGEL_DEFAULT_PROFILE`, a `hegel.toml` profile, `HEGEL_MAIN`'s `--profile`, or a settings environment variable such as `HEGEL_SEED`, `HEGEL_PHASES`, `HEGEL_VERBOSITY` or `HEGEL_PRINT_BLOB`.

`verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness` and `print_blob` are now `std::optional`. `phases` is now the new `hegel::Phases` type, which is unset by default.

## 0.15.1 - 2026-11-09

This patch lets a long state-machine run explore more than one configuration. Between rounds, a state machine now occasionally redraws its value distribution, the rules each worker has enabled, and the concurrency level. Previously these were fixed for the whole test case, so a `HEGEL_MAIN` binary kept one configuration for its whole run.

`hegel::stateful::run_concurrent` now always starts `max_concurrency` worker threads, and only the current concurrency level's worth of them run rules in a round. A failure report notes `Concurrency level: n` at the first round and whenever the level changes.

## 0.15.0 - 2026-11-06

This release adds weights, a hint for how often to choose the rule relative to the others, to stateful rules. `hegel::stateful::Rule` and `hegel::stateful::ConcurrentRule` each take an optional `weight`, and `weight()` returns it. A rule without a weight has weight 1.0.

`hegel::stateful::Invariant` also now takes an optional `always_check` flag. An invariant with the flag runs at every step of a state machine, or every round of a concurrent one, instead of being sampled. `always_check()` returns it.

These can be passed positionally or by name through the new `RuleParams`, `ConcurrentRuleParams` and `InvariantParams` structs:

```cpp
hegel::stateful::ConcurrentRule<Bank>("transfer", {"bank"}, 6.0, transfer);

hegel::stateful::ConcurrentRule<Bank>({.name = "transfer",
                                       .groups = {"bank"},
                                       .weight = 6.0,
                                       .step = transfer});
```

`ConcurrentRule::function()` is now `step()`, and `ConcurrentRule::Function` is now `Step`, to match `Rule`.

## 0.14.1 - 2026-11-06

This patch adds `HEGEL_MAIN` for running a Hegel test as a standalone binary. 
Each setting except `test_cases` is a command-line option named after its `hegel.toml` 
key (`--seed`, `--phases`, `--backend` and so on).

```cpp
HEGEL_MAIN(workload)(hegel::TestCase& tc) {
    auto n = tc.draw("n", gs::integers<int>());
    if (n + 0 != n) {
        throw std::runtime_error("n + 0 != n");
    }
}
```

## 0.14.0 - 2026-11-03

This release lets a rule of a concurrent state machine belong to several concurrency groups. `hegel::stateful::ConcurrentRule` now takes a list of group names in place of one name, and `group()` becomes `groups()`, returning that list. A rule constructed without groups still belongs to `anonymous_group`.

Before:

```cpp
hegel::stateful::ConcurrentRule<Store>("write", "write", write);
```

After:

```cpp
hegel::stateful::ConcurrentRule<Store>("write", {"write"}, write);
hegel::stateful::ConcurrentRule<Store>("read", {"write", "flush"}, read);
```

A rule in several groups runs in the rounds of each of them: here `read` can overlap with `write` in a `write` round and with `flush` in a `flush` round, which is the only way to observe a read racing a flush. The list must name at least one group and must not repeat one; the engine reports either as a `std::invalid_argument` when the machine starts.

## 0.13.4 - 2026-10-30

This patch adds `GIT_CONFIG lfs.fetchexclude=website` to the recommended `FetchContent_Declare` in the README and the API reference. The hegeldev/hegel repository also holds the hegel.dev website, whose images and generated API references are stored in Git LFS, and with this line a machine that has git-lfs installed no longer downloads them when fetching Hegel.

## 0.13.3 - 2026-10-28

This release updates the bundled libhegel engine to 0.49.0.

## 0.13.2 - 2026-10-22

This patch moves Hegel for C++ into the [hegeldev/hegel](https://github.com/hegeldev/hegel) monorepo, next to the engine it drives. Fetch it from there now, with the frontend as the source subdirectory:

```cmake
FetchContent_Declare(
    hegel
    GIT_REPOSITORY https://github.com/hegeldev/hegel.git
    GIT_TAG frontends/cpp/<version>
    SOURCE_SUBDIR frontends/cpp
)
```

The build no longer downloads a prebuilt libhegel: it compiles the in-tree engine with `cargo`, so a Rust toolchain is needed unless `-DHEGEL_LIBHEGEL_LIBRARY` points at a libhegel built elsewhere (`-DHEGEL_LIBHEGEL_INCLUDE_DIR` then names that library's `hegel.h`). The Nix flake is gone with the download it existed for.

This release also updates the bundled engine from 0.42.4 to 0.48.0, which changes what a C++ user can see:

- A test whose outcome or draws change when the same generated data is replayed no longer fails the run with a flaky-test error. The engine confirms each such failure by replaying it, reports it with a `note:` line saying how reliably it reproduced, and leaves out the `rerun with:` line when it could not be confirmed. The new `Settings::nondeterminism_strictness` restores the old error (`NondeterminismStrictness::Error`) or adds a one-line notice per run (`Warn`); the default is `Quiet`.
- The new `Settings::stop_on_first_failure` (default `true`) can make a run use its whole `test_cases` budget before reporting, so every distinct failure found along the way is persisted to the database.
- A run that generates no valid test case now fails with an `Unsatisfiable` error, even with the `FilterTooMuch` health check suppressed.
- Concurrent state machines no longer print a `Concurrent state machine detected` notice; a failure one of their workers finds is reported from the output the engine captured for it.
- Strings passed to the engine may contain NUL characters, `gs::from_regex()` rejects a pattern no string can match when the generator is built, integer shrinking no longer stalls short of the minimum on some sparse sets of failing values, and seeded runs may generate different values from this version on.

## 0.13.1 - 2026-09-28

This patch fixes stateful tests continuing after a rule fails a GoogleTest `ASSERT_*` ([#138](https://github.com/hegeldev/hegel-cpp/issues/138)). A `ASSERT_*` failure now ends the test case.

## 0.13.0 - 2026-09-16

This release bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.37.5](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.37.5) to [0.42.4](https://github.com/hegeldev/hegel-rust/releases/tag/libhegel-v0.42.4). hegel-rust now tags libhegel releases `libhegel-v<version>`, and the build downloads the engine from that tag.

The engine now resolves its settings from named profiles, and a `Settings` field left at its default takes the profile's value: the `default` profile comes from a `hegel.toml` in the working directory or an ancestor, the `HEGEL_DEFAULT_PROFILE` environment variable, or the detected environment (`ci` on a CI server, `workload` inside Antithesis, `development` otherwise). This changes what the defaults mean:

- `Settings::test_cases` left unset runs the profile's number of test cases, which is 100 unless a `hegel.toml` changes it.
- `Settings::suppress_health_check` left empty suppresses the profile's set: nothing locally, `TooSlow` on a CI server.
- `Backend::Auto` leaves the randomness backend to the profile (`Urandom` inside Antithesis, `Default` otherwise) instead of the engine's own detection.

Setting a field explicitly overrides the profile, as before.

`hegel::test()` now rejects a `Settings::stateful_step_count` below 1 with `std::invalid_argument` before the run starts. Previously the engine rejected it as a `std::runtime_error`. The step budget itself is unchanged: each stateful case runs at least one step and at most `stateful_step_count` of them.

Inside [Antithesis](https://antithesis.com/), the engine now reports every run's verdict as an `always` assertion, so a property test is listed and flagged alongside the assertions in the system under test. A property inside a GoogleTest test `Suite.Name` reports as `Suite::Name passes properties`; a test defined with `HEGEL_TEST` or run with an explicit `TestLocation` reports as `<file>::<name> passes properties`. Outside Antithesis nothing changes.

The new engine also changes what runs observe:

- A test case may make 2^20 choices before the engine stops it as an overrun, up from 8,192.
- A test whose verdict for the same generated data differs between two runs now fails the run with `Flaky test detected`, where it previously kept the first verdict. Discarding a case based on state outside the drawn data, such as a call counter, now trips this.
- Shrinking reaches smaller counterexamples in several situations: pairs of draws a test pins together, a draw that only fails at multiples of a round number, a `one_of` whose shorter failing alternative needs a non-trivial value, and stateful sequences with redundant steps.
- The failure database saves a new entry before removing the one it supersedes, so an interrupted shrink no longer loses a failure.

## 0.12.1 - 2026-09-08

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.35.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.35.0) to [0.37.5](https://github.com/hegeldev/hegel-rust/releases/tag/v0.37.5).

## 0.12.0 - 2026-09-02

`Mode`, `Mode::SingleTestCase`, and `Settings::mode` have been removed.

## 0.11.6 - 2026-09-02

This patch adds the `HEGEL_THROW_SITE` CMake option (default `ON`). Hegel
names the source position of a failing throw by wrapping `__cxa_throw` and
forwarding to the real one through `dlsym(RTLD_NEXT, ...)`, which requires
the C++ runtime to be a shared library. Toolchains that link the C++ runtime
statically (for example `-static-libstdc++`, or a hermetic toolchain with
`libc++abi.a`) previously failed to link with a duplicate `__cxa_throw`
definition. Configure with `-DHEGEL_THROW_SITE=OFF` (or, outside CMake,
compile the library with `HEGEL_HAS_THROW_SITE=0`) to compile the wrapper
out; failure reports then name the exception type without the throw site.

## 0.11.5 - 2026-09-01

This patch adds concurrent stateful testing: stateful tests where rules run concurrently from a number of worker threads. See the documentation for
`run_concurrent` and `ConcurrentStateMachine` for details.

It also adds `ConcurrentPool<T>`, which allows concurrent rules to safely add,
reuse, and consume shared generated values. Stateful invariants now run in full on the initial and final states and are sampled between rules in sequential stateful tests and between rounds (a set of concurrent rule executions) in concurrent stateful tests.

## 0.11.4 - 2026-08-14

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.32.3](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.3) to [0.32.5](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.5).

## 0.11.3 - 2026-08-11

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.32.2](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.2) to [0.32.3](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.3).

## 0.11.2 - 2026-08-10

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.30.5](https://github.com/hegeldev/hegel-rust/releases/tag/v0.30.5) to [0.32.2](https://github.com/hegeldev/hegel-rust/releases/tag/v0.32.2).

## 0.11.1 - 2026-08-04

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.29.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.29.0) to [0.30.5](https://github.com/hegeldev/hegel-rust/releases/tag/v0.30.5).

## 0.11.0 - 2026-08-04

This release replaces the Hegel test framework with an opt-in GTest integration.
`gtest_throw_on_failure` is no longer needed for a property to see a failed 
assertion. Users can still call existing Hegel tests as regular functions from 
`main()`.

 `HEGEL_DRAW`, `HEGEL_REQUIRE_EQUAL`, `HEGEL_REQUIRE`, `HEGEL_FAIL`, 
 `hegel::run_all_tests()`, and the test registry have been removed. 

Hegel can now distinguish between multiple exceptions of the same type. Their
locations in the test function is the differentiator by default. 
Derive exceptions from `hegel::FailureOrigin` to differentiate them some other
way.

## 0.10.0 - 2026-07-29

This release requires the values `HEGEL_REQUIRE_EQUAL` compares to be ones
Hegel can render or to have an `operator<<`, and fixes a bug where two
unrenderable values were always recognized as equal.

## 0.9.0 - 2026-07-28

This release overhauls what Hegel prints when a property fails, adds three
macros for stating what a test requires, and changes how a state machine
declares its state to accommodate state printing.

A failing run now produces a framed report like the one below:

```
--- Failure: sort_agrees_with_std_sort (sort_test.cpp:9) ----------------
Falsified after 8 test cases (0 discarded):

  auto vec1 = std::vector<int>{0, 0};

Exception: std::runtime_error: sort mismatch
rerun with: HEGEL_REPRODUCE_FAILURE(sort_agrees_with_std_sort, "AXicY2VgYGBkZOBiZEBhMAAAAd8AIQ==")
```

`Settings::print_blob` now defaults to `true`, so the `rerun with:` line
prints by default. A run that finds several distinct failures reports each in
its own numbered section.

`HEGEL_TEST` supplies the test's name and source line, so its failures name
themselves in the header. `hegel::test()` now optionally takes a `TestLocation`.

```cpp
hegel::test(my_body, {"my_property", __FILE__, __LINE__}, {.test_cases = 500});
```

This release also adds `HEGEL_REQUIRE`, `HEGEL_REQUIRE_EQUAL`, and
`HEGEL_FAIL` for stating properties and failures.

```cpp
HEGEL_TEST(addition_commutes)(hegel::TestCase& tc) {
    HEGEL_DRAW(tc, x, gs::integers<int>());
    HEGEL_DRAW(tc, y, gs::integers<int>());
    HEGEL_REQUIRE_EQUAL(tc, x + y, y + x);
}
```

For an equality property prefer `HEGEL_REQUIRE_EQUAL`. Its report shows a
difference of the two values:

```
HEGEL_REQUIRE_EQUAL: values differ (- lhs / + rhs):
  Team{
    .name = std::string("a"),
    .scores = std::vector<int>{
      1,
-     2,
+     9,
      3,
    },
  }
```

Prefer all three over a raw `throw`. Hegel tells one bug from another by
where the failure came from, and a raw `throw` carries no source position, so 
every throw of one type in a run counts as a single bug under 
`Settings::report_multiple_failures`. Each macro invocation carries the position
of the line you wrote it on, so failures on different lines are distinct.

`StateMachine` now takes the state's type as a second template argument and 
holds the state itself, and its constructor takes the initial state. A failing 
stateful run prints that state before the first step and after every step that
runs to completion.

```cpp
struct Stack : hegel::stateful::StateMachine<Stack, std::vector<int>> {
    Stack() : StateMachine({.initial_state = {}}) {}

    std::vector<hegel::stateful::Rule<Stack>> rules() {
        return {hegel::stateful::Rule<Stack>(
            "push", [](hegel::TestCase& tc, Stack& m) {
                m.state.push_back(tc.draw(gs::integers<int>()));
            })};
    }
};
```

State printing can be disabled with `.print_state = false`.

```cpp
hegel::stateful::run(machine, tc, {.print_state = false});
```

## 0.8.0 - 2026-07-27

This release changes how drawn values are printed in failure replays and
verbose output. Hegel previously printed one `Generated: <value>` line per
primitive draw inside the engine. It now prints one line per user-level `tc.draw(...)`, 
with the final composed value rendered as a C++ declaration:

```
auto draw_1 = std::vector<int>{0, 0};
```

Types Hegel cannot render as an expression fall back to
their `operator<<` output, then to an `<unprintable Type>` placeholder.

This release also adds named draws. `tc.draw("x", gen)` prints the draw
under the given name, and the new `HEGEL_DRAW` macro binds a variable and
captures its name in one step:

```cpp
HEGEL_TEST(addition_commutes)(hegel::TestCase& tc) {
    HEGEL_DRAW(tc, x, gs::integers<int>());
    HEGEL_DRAW(tc, y, gs::integers<int>());
    if (x + y != y + x) throw std::runtime_error("not commutative");
}
// replay output: auto x = 10;
//                auto y = 3;
```

A name prints bare on every use. Pass `repeatable = true`
(`tc.draw("x", gen, true)`) to number repeated draws `x_1`, `x_2`, ...
for draws in loops. Unnamed draws print as `draw_1`, `draw_2`, ...
per test case. In stateful tests, each rule's draws print indented under
their `Step N:` header.

## 0.7.5 - 2026-07-22

This patch adds test-case cloning. `TestCase::clone()` forks an independent draw stream of the current test case. The clone draws from its own choice sequence but shares the case's outcome and budget. A single test case must not be drawn from concurrently.

`TestCase::spawn()` runs a callable on a clone in a new thread and returns a `hegel::Worker`. `Worker::join()` awaits it, returning the callable's result and re-raising any exception it threw. Join every worker before the test body returns.

```cpp
auto worker = tc.spawn([](hegel::TestCase& c) {
    return c.draw(gs::integers<int>());
});
auto mine = tc.draw(gs::integers<int>());
auto theirs = worker.join();
```

## 0.7.4 - 2026-07-18

This patch adds stateful testing under the `hegel::stateful` namespace. A stateful test drives the system under test through a sequence of randomly chosen actions applied to a state machine. When a sequence falsifies an invariant or throws, the engine shrinks it to a minimal failing sequence and replays it.

Derive a machine from `hegel::stateful::StateMachine<T>` and define its `rules()`, returning `hegel::stateful::Rule<T>` actions. Optionally override `invariants()` to add `hegel::stateful::Invariant<T>` predicates. Pass an instance to `hegel::stateful::run`. A rule's step mutates the machine in place. Invariants are evaluated before the first step and after every valid step.

```cpp
namespace gs = hegel::generators;

struct IntegerStack : hegel::stateful::StateMachine<IntegerStack> {
    std::vector<int> stack;

    std::vector<hegel::stateful::Rule<IntegerStack>> rules() {
        return {
            hegel::stateful::Rule<IntegerStack>(
                "push", [](hegel::TestCase& tc, IntegerStack& m) {
                    m.stack.push_back(tc.draw(
                        gs::integers<int>({.min_value = 0, .max_value = 100})));
                }),
            hegel::stateful::Rule<IntegerStack>(
                "pop", [](hegel::TestCase& tc, IntegerStack& m) {
                    tc.assume(!m.stack.empty());
                    m.stack.pop_back();
                }),
        };
    }
};

hegel::test([](hegel::TestCase& tc) {
    IntegerStack machine;
    hegel::stateful::run(machine, tc);
});
```

## 0.7.3 - 2026-07-16

This patch adds pools for storing previously generated values. It also adds two
generators, `values_reusable` and `values_consumed`, for drawing previously generated
values with and without replacement, respectively.

Pools will be more useful once stateful testing is added in a later release.

## 0.7.2 - 2026-07-15

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.28.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.28.0) to [0.29.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.29.0).

## 0.7.1 - 2026-07-10

This patch adds three `TestCase` methods and three generators.

New `TestCase` methods:

- `tc.reject()` rejects the current test case unconditionally. Unlike `tc.assume(false)`, it is marked `[[noreturn]]` so it can stand in for a value in a branch that cannot continue.

- `tc.target(score, label = "")` records a numeric observation for the engine's targeted-search phase to maximize. Higher scores are treated as more interesting, so the engine biases later test cases toward inputs that produced higher scores under the same label.

- `tc.repeat(body)` runs `body` in an engine-managed loop whose iteration count the engine chooses and shrinks, like any other drawn value.

```cpp
int total = 0;
tc.repeat([&] {
    total += tc.draw(gs::integers<int>({.min_value = 0, .max_value = 10}));
    if (total >= 50) throw std::runtime_error("too much");
});
```

New generators:

- `uuids()` produces canonical hyphenated UUID strings (e.g. `f47ac10b-58cc-4372-a567-0e02b2c3d479`). By default any version is generated. Pass `{.version = N}` to force an RFC 4122 version (1-5).

- `arrays<T, N>(element)` produces a fixed-size `std::array<T, N>`, drawing exactly `N` elements from the generator 
`element`. Unlike `vectors()`, the length is fixed at compile time, so `N` is given explicitly: `arrays<int, 3>(integers<int>())`.

- `deferred<T>()` creates a forward reference for recursive or mutually recursive generators. Call `.generator()` to obtain handles before the implementation is known, embed them in other generators, then call `.set(...)` once to install the implementation:

```cpp
struct Tree { int leaf; std::vector<Tree> children; };

auto tree = gs::deferred<Tree>();
auto leaf = gs::integers<int>().map([](int v) { return Tree{v, {}}; });
auto branch = gs::compose([tree](const hegel::TestCase& tc) {
    return Tree{0, {tc.draw(tree.generator()), tc.draw(tree.generator())}};
});
tree.set(gs::one_of<Tree>({leaf, branch}));
```

## 0.7.0 - 2026-07-09

This release changes the default value of `fullmatch` in `from_regex` from `false` to `true`.

## 0.6.3 - 2026-07-09

This patch bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.27.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.27.0) to [0.28.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.28.0).

## 0.6.2 - 2026-07-09

This patch adds the ability to reproduce a specific failing example from its
reproduction blob and a `print_blob` setting to toggle printing those blobs.

To replay a failure, annotate a `HEGEL_TEST` with `HEGEL_REPRODUCE_FAILURE`. 
The test replays that blob instead of generating new cases:

```cpp
HEGEL_REPRODUCE_FAILURE(my_property, "AAEAAAAACgEAAAAA")
HEGEL_TEST(my_property)(hegel::TestCase& tc) {
    int n = tc.draw(gs::integers<int>());
    if (n < 50) {
        throw std::runtime_error("fail");
    }
}
```

At least one blob is required. More than one blob can be added for bookkeeping but 
only the first is replayed. Delete the annotation to return to a normal run.

When calling `hegel::test` directly, pass the blobs as the third argument:

```cpp
hegel::test(my_property, {}, {"AAEAAAAACgEAAAAA"});
```

## 0.6.1 - 2026-07-07

This release adds `HEGEL_TEST`, the new recommended way to define a property test. The macro defines the test as a plain function you can invoke from `main()` or any test framework, and derives a database key from the defining file and test name, so failing examples are persisted to the example database and replayed first on later runs. Settings can be written inline after the test name and become the function's default argument. Settings passed when invoking the test replaces them for that run:

```cpp
HEGEL_TEST(addition_commutes, {.test_cases = 500})(hegel::TestCase& tc) {
    int x = tc.draw(gs::integers<int>());
    int y = tc.draw(gs::integers<int>());
    if (x + y != y + x) {
        throw std::runtime_error("addition is not commutative");
    }
}
```

Each `HEGEL_TEST` is also registered with the new `hegel::run_all_tests()`, which runs every test defined with the macro in a translation unit. It reports failures to stderr and returns 0 if everything passed (1 otherwise).

You can run the test above in two ways:

```cpp
int main() {
    return hegel::run_all_tests();
}
```
```cpp
int main() {
    addition_commutes();
    return 0;
}
```

There are four new settings:

- `database_key`: the key scoping which examples are stored in and replayed from the database. `HEGEL_TEST` fills it in automatically. Set it yourself when calling `hegel::test()` directly.
- `phases`: which phases of the run to enable (`Phase::Explicit`, `Reuse`, `Generate`, `Target`, `Shrink`)
- `mode`: `Mode::TestRun` (the default) or `Mode::SingleTestCase`, which produces one test case and stops with no shrinking intended for long running tests
- `backend`: the engine's randomness source. `Backend::Auto` (the default), `Backend::Default` (seeded PRNG), or `Backend::Urandom` (fresh entropy per draw, intended for Antithesis)

## 0.6.0 - 2026-07-07

This release bumps our pinned `libhegel` ([hegel-rust](hegeldev/hegel-rust)) from [0.23.2](https://github.com/hegeldev/hegel-rust/releases/tag/v0.23.2) to [0.27.0](https://github.com/hegeldev/hegel-rust/releases/tag/v0.27.0) and migrates the library to its reworked C ABI.

The CBOR request/response protocol has been removed, so the `HEGEL_PROTOCOL_DEBUG` environment variable and the Debug-verbosity `REQUEST:`/`RESPONSE:` dump have been removed. `Verbosity::Debug` now enables the engine's own shrinker tracing.

## 0.5.0 - 2026-07-02

This release adds the `report_multiple_failures` setting and reworks how
`hegel::test` reports failures.

With `report_multiple_failures` set to `true`, the engine keeps generating
after the first failure to surface additional distinct bugs. The setting 
defaults to `false`.

A test with a single failing example now re-raises the test's own exception
instead of `std::runtime_error`.

Foreign (non-C++) exceptions escaping a test body are now reported as a
`std::runtime_error` instead of undefined behavior.

## 0.4.0 - 2026-06-30

This release replaces the Python hegel-core engine with the `libhegel` Rust engine,
called in-process through its C ABI. There is no longer a subprocess, socket,
or wire protocol, and `uv` is no longer required.

The public generator and `hegel::test` API is unchanged.

This release also adds a C++17 build path. Configure with `-DHEGEL_REFLECTION=OFF`
to drop the reflect-cpp dependency and build at C++17. `default_generator` and
automatic struct derivation become unavailable, but every other generator and
combinator still works.

## 0.3.9 - 2026-05-20

This patch bumps our pinned hegel-core from [0.6.0](https://github.com/hegeldev/hegel-core/releases/tag/v0.6.0) to [0.9.1](https://github.com/hegeldev/hegel-core/releases/tag/v0.9.1).

## 0.3.8 - 2026-04-30

Internal refactor.

## 0.3.7 - 2026-04-29

Internal refactor of `one_of`.

## 0.3.6 - 2026-04-28

Bump our pinned [`hegel-core`](https://github.com/hegeldev/hegel-core) version from `0.4.0` to [`0.4.14`](https://github.com/hegeldev/hegel-core/releases/tag/v0.4.14).

## 0.3.5 - 2026-04-21

Fix our CMake integration with `FetchContent` or `add_subdirectory`, which previously errored.

## 0.3.4 - 2026-04-21

General documentation improvements

## 0.3.3 - 2026-04-20

Update and build our documentation to https://hegel.dev/.

## 0.3.2 - 2026-04-20

Update how we install uv and hegel-core to match https://hegel.dev/reference/installation.

## 0.3.1 - 2026-04-20

Support displaying user defined exceptions, health check failures, and flaky failures.

## 0.3.0 - 2026-04-20

Multiple improvements to make hegel-cpp more idiomatic to use.

* Rename `hegel::hegel` to `hegel::test`.
* Rename `HegelSettings` to `Settings`.
* Move settings objects into the main `hegel` namespace, from the `hegel::settings` namespace. Removed the `hegel::settings` namespace.
* Rename `from_function` to `compose`. `compose` no longer requires an explicit declaration of the return type, unless you want to override the inferred type.
* Rename `dictionaries` to `maps`.
* Remove the `nulls()` generator, which misleadingly returned `std::monostate`.
* The `just` and `sampled_from` generators now accept any object, not just serializable objects.

## 0.2.0 - 2026-04-16

Multiple refactors and renames, as we prepare for a more standard release.

## 0.1.0 - 2026-04-15

Initial release!
