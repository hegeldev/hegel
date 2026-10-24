RELEASE_TYPE: minor

This release changes how `hegel::Settings` combine with the engine's settings profiles, gives run errors their own exception types, and replays every reproduce blob it is given.

A `Settings` field that you leave unset now keeps the value of the settings profile (`hegel.toml`, `HEGEL_DEFAULT_PROFILE`, the `HEGEL_*` variables, or the `ci`, `workload` and `development` profiles the engine picks itself). Before, `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness`, `phases` and the seed were always set to Hegel for C++'s own defaults, which overrode the profile, and `print_blob` was never sent to the engine at all. Hegel for C++ no longer detects CI itself; the engine's `ci` profile does it. To support this:

- `verbosity`, `derandomize`, `report_multiple_failures`, `stop_on_first_failure`, `nondeterminism_strictness` and `print_blob` are now `std::optional`. Code that reads these fields from a `Settings` must handle the unset case; code that sets them does not change.
- `suppress_health_check` and `phases` are now `hegel::ListSetting`, which is unset by default. A braced list sets them as before, and an empty list (`.suppress_health_check = {}`) now suppresses no health checks instead of keeping the profile's set.
- `Database::unset()` (the default) now keeps the profile's database, and the new `Database::default_location()` selects `.hegel` even where the profile disables the database.

This release also adds:

- `Settings::show_statistics` and `Settings::unbounded_choices`.
- `Settings::profile`, `hegel::register_profile()` and `hegel::set_default_profile()`, to select and define settings profiles from C++.
- `hegel::engine_version()`, the version of the bundled libhegel.

A run that stops without a verdict now throws `hegel::FailedHealthCheck` (with the check that failed), `hegel::Flaky`, `hegel::Unsatisfiable`, or else `hegel::RunError`, their base class, instead of a plain `std::runtime_error` with a `Hegel run error:` prefix. A run that reports several failures throws `hegel::MultipleFailures`, which holds each failure's exception. All of these still derive from `std::runtime_error`.

Every blob given to `HEGEL_REPRODUCE_FAILURE` or to `hegel::test()` is now replayed, not only the first, and a nondeterministic failure's blob is replayed until it fails. A blob replay prints the same failure report as a generated failure. If no blob makes the test fail, `hegel::test()` throws `hegel::DidNotReproduce`, and an invalid blob throws `std::invalid_argument`.

`hegel::test()` now derives its database key (`<file>::<name>`) also when the database is disabled, as `HEGEL_TEST` already did, because a derandomized run derives its seed from the key. Without it, every derandomized test under the `ci` profile saw the same inputs.

The undocumented `hegel::verbosity_to_string()` and `hegel::health_check_to_string()` are removed.

Generators now identify themselves to the engine with the same span labels as the other Hegel frontends: each label comes from a `hegel.<kind>` name (`hegel.lists`, `hegel.one_of`, `hegel.integers`, ...), and a generator built from others combines its own label with theirs through libhegel's `hegel_label_combine`. Before, every `vectors()` span had the same label whatever its elements were, so the shrinker treated a vector of integers and a vector of strings as interchangeable. Database entries saved by earlier versions may no longer replay.

Notes and drawn values now print through libhegel's printer instead of Hegel for C++'s own output code, which changes how they look:

- A drawn value too wide for one line is broken across lines, one element per line, instead of printing as one long line.
- A clone's output appears where the clone was made (`tc.clone()` or `tc.spawn()`), not interleaved with the parent's in the order the lines were written.
- In a failure report of `hegel::stateful::run_concurrent()`, the `[worker N +T ms]` prefix comes after the report's indentation instead of before it, and each worker's lines appear under the header of the round they ran in.
