RELEASE_TYPE: minor

This release brings Hegel for C++ into line with the other Hegel frontends: `hegel::Settings` now combine with the engine's settings profiles instead of overriding them, run errors and failure reports match hegel-rust's, every reproduce blob is replayed, output goes through libhegel's printer, and it adds explicit test cases, several generators and the rest of the engine's settings.

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

Explicit test cases run a test with values you choose, before Hegel generates any, when the `Explicit` phase is enabled. Add one to a `HEGEL_TEST` with `HEGEL_EXPLICIT_TEST_CASE(name, values...)`, or pass a list of `hegel::ExplicitTestCase` to `hegel::test()` as its new `explicit_cases` argument, after `failure_blobs`. Positional values go to the test's draws in order, and `hegel::named("x", value)` gives a value to the draw named `x`:

```cpp
HEGEL_EXPLICIT_TEST_CASE(division, 7, hegel::named("d", 0))
HEGEL_TEST(division)(hegel::TestCase& tc) {
    auto n = tc.draw(gs::integers<int>());
    auto d = tc.draw("d", gs::integers<int>());
    // ...
}
```

A failing explicit test case prints what it drew and noted, and its exception propagates. A misuse of an explicit test case is a usage error, which makes `hegel::test()` throw `std::invalid_argument` without reporting a failure: a rejection (its message begins `Explicit test cases must not be rejected`), a draw with no value or a value of another type, a value that no draw takes, and anything that needs generated data other than a draw, such as `TestCase::clone()` or stateful testing. `TestCase::repeat()` runs its body once, so a rejection inside it is reported like any other.

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

### Generators

This release adds:

- `TestCase::event()` and `TestCase::event_value()`, which record events and numeric observations for the end-of-run statistics report that `Settings::show_statistics` turns on.
- `hegel::generators::none()`, which generates `std::monostate{}`.
- `hegel::generators::permutations()`, which generates reorderings of a list of values.
- A `vectors()` overload that takes a key function, `vectors(elements, params, unique_by)`: no two elements of a generated vector have equal keys.
- `hegel::generators::recursive()`, which generates recursively defined values, such as trees, through libhegel's recursion support: the engine decides where a value branches and how large it grows, within `RecursiveParams::max_depth` and `RecursiveParams::max_leaves`.
- `hegel::generators::from_lark()`, which generates strings of a grammar written in the syntax of the [Lark](https://lark-parser.readthedocs.io/en/stable/grammar.html) parsing library. `LarkParams` selects the start rule, gives explicit terminals whose values come from your own generators, restricts the characters of the grammar's own terminals with an `alphabet`, limits the size and nesting depth of the strings, and sets the directory that `%import`ed grammar files are read from.
- Support for integer types wider than 64 bits in `integers()`, such as `__int128` and `unsigned __int128`. Before, their bounds were truncated to 64 bits.

Generators now identify themselves to the engine with the same span labels as the other Hegel frontends: each label comes from a `hegel.<kind>` name (`hegel.lists`, `hegel.one_of`, `hegel.integers`, ...), and a generator built from others combines its own label with theirs. Before, every `vectors()` span had the same label whatever its elements were, so the shrinker treated a vector of integers and a vector of strings as interchangeable. Database entries saved by earlier versions may no longer replay. `compose()` now groups the draws its function makes into one span, as `map()`, `flat_map()` and `filter()` do, so the engine shrinks them as a unit.

`text()` and `binary()` with no `max_size` now take libhegel's default maximum, the same as every other Hegel frontend: 100, or `min_size + 100` when `min_size` is above 100. Before, a `min_size` above 100 with no `max_size` made every value exactly `min_size` long. A `max_size` too large for any allocation (such as `SIZE_MAX`) now means no upper bound. `text({.alphabet = ...})` now generates only the characters of the alphabet, and `characters({.categories = {}})` allows no category. Before, an empty `categories` list was sent to libhegel as no list, so these generated characters from all of Unicode.

`floats<long double>()` now draws a `double`. Before, it failed with an engine error where `long double` is wider than 64 bits. There, a bound that is not a `double` is moved inward to the nearest `double`, so every value it generates is within the bounds you gave.

A `filter()` on `hegel::stateful::values_consumed()` now removes only the value it returns from the pool. Before, every value its predicate rejected was also removed, so later draws could not use it, and the same happened to every value it had tried when the predicate threw.

### Removals and corrections

The undocumented `hegel::verbosity_to_string()` and `hegel::health_check_to_string()` are removed.

The 0.13.2 changelog entry says the Nix flake was removed. That is not correct: `nix/flake.nix` is still available and still exports `lib.mkHegelCppProject`. It now builds the in-tree libhegel instead of downloading one.
