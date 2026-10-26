# Settings

Every Hegel run is configured by a {class}`hegel.Settings` value. This page
covers what the settings are, the places each one can be set, how those places
combine, and how the *profile* a run starts from is chosen.

## The settings

| Setting | Values | Base value | Effect |
|---|---|---|---|
| `test_cases` | integer ≥ 0 | `100` | How many test cases to run. |
| `verbosity` | `quiet`, `normal`, `verbose`, `debug` | `normal` | How much Hegel prints ({class}`~hegel.Verbosity`). |
| `seed` | integer, or `None` | `None` | A fixed seed, for reproducibility. `None` means a fresh random seed for each run. |
| `derandomize` | boolean | `False` | Use a fixed seed derived from the test, so every run of a test is the same. A fixed `seed` takes precedence. |
| `database` | a directory path, `None`, or `""` | `None` | Where failing examples are kept for replay. `None` is the default directory, `.hegel/examples` under the working directory, and `""` (or `False`) turns the database off. |
| `suppress_health_check` | collection of {class}`~hegel.HealthCheck` | none | Health checks that should not fail the run. See {doc}`../how-to/health-checks`. |
| `phases` | collection of {class}`~hegel.Phase` | all five | Which parts of the run happen; leaving out `shrink`, say, reports the first failure found as it is. See {doc}`../explanation/how-hegel-works`. |
| `report_multiple_failures` | boolean | `False` | Report every distinct failure a run finds, as a {class}`~hegel.MultipleFailures`, rather than only the smallest. |
| `stop_on_first_failure` | boolean | `True` | Stop generating once a failure is found. When `False`, the run uses its whole `test_cases` budget before reporting; every failure found is saved to the database, and with `report_multiple_failures` off only the smallest is shrunk and reported. |
| `show_statistics` | boolean | `False` | Print the end-of-run statistics report for events recorded with {meth}`~hegel.TestCase.event` and {meth}`~hegel.TestCase.event_value`. |
| `unbounded_choices` | boolean | `False` | Let a test case make any number of choices. Otherwise a test case making more than 2<sup>20</sup> choices is stopped and discarded. A long-running test case, such as a concurrent state machine driven for hours, needs the limit removed; the cost is the memory to record every choice. Suppressing the `test_cases_too_large` health check removes the limit too. |
| `print_blob` | boolean | `True` | On failure, print a `@hegel.reproduce_failure("...")` line to replay it. |
| `backend` | `default`, `urandom` | `default` | The source of randomness ({class}`~hegel.Backend`): a seeded random number generator, or fresh entropy for every choice, for running under Antithesis. |
| `nondeterminism_strictness` | `quiet`, `warn`, `error` | `quiet` | How a run reacts when the test behaves nondeterministically ({class}`~hegel.NondeterminismStrictness`). See {doc}`../explanation/nondeterminism`. |

In Python, a setting whose value is an enum takes either the member
(`Verbosity.verbose`) or its name (`"verbose"`), and one taking a collection of
enum members also accepts a single one. The names are also the vocabulary of
`hegel.toml`.

## Where settings come from

A run's settings are built in layers. Each layer overrides what the layers
below it set and leaves the rest alone:

1. **The base settings**: the values in the table above. They cannot be
   changed, and are available by name as the reserved `base` profile.
2. **The profile**: a named set of overrides, from the profiles shipped with
   Hegel, your `hegel.toml`, and profiles registered in Python. `Settings()`
   resolves the `default` profile, the one in effect for the current
   environment; {meth}`Settings.from_profile <hegel.Settings.from_profile>`
   resolves any profile by name. Most of the rest of this page is about this
   layer.
3. **Environment variables**, applied over the resolved profile:

   | Variable | Effect |
   |---|---|
   | `HEGEL_TEST_CASES` | Sets `test_cases`. Must be a positive integer. |
   | `HEGEL_DATABASE` | Sets `database`: `disabled` turns it off, any other value is the path. |
   | `HEGEL_STATISTICS` | Anything but `0` or the empty string turns `show_statistics` on. |
   | `HEGEL_SEED` | Sets `seed`: an integer is the seed, `none` clears one the profile set. |
   | `HEGEL_DERANDOMIZE` | Sets `derandomize`: `true`, `1` or `yes`, or `false`, `0` or `no`. |
   | `HEGEL_PRINT_BLOB` | Sets `print_blob`, with the same vocabulary. |
   | `HEGEL_STOP_ON_FIRST_FAILURE` | Sets `stop_on_first_failure`, with the same vocabulary. |
   | `HEGEL_UNBOUNDED_CHOICES` | Sets `unbounded_choices`, with the same vocabulary. |
   | `HEGEL_NONDETERMINISM_STRICTNESS` | Sets `nondeterminism_strictness`: `quiet`, `warn` or `error`. |

   An empty variable is ignored; any other malformed value is an error,
   raised when a `Settings` value is created or used. A fixed `seed`, wherever it
   came from, still takes precedence over `derandomize`, so
   `HEGEL_DERANDOMIZE=true` has no effect on a profile with a seed unless
   `HEGEL_SEED=none` clears it.
4. **Settings in the test**: the keyword arguments of
   {func}`@hegel.test <hegel.test>`, applied over the `settings=` argument if
   one is given, or over `Settings()` otherwise:

   ```python
   @hegel.test(test_cases=500, derandomize=True)
   def test_compiled_in(tc): ...
   ```

   These are the settings the author of the test chose, so they take
   precedence over the environment variables: a variable adjusts a test suite,
   not a test that sets the same setting itself. A `Settings` object passed as
   `settings=` belongs to this layer too: it holds only the settings it was
   given, and everything else still comes from the layers below.

Hegel resolves the settings each time a test runs, so changes to the
environment or the default profile made before a test runs apply to it, even
for a `Settings` object made earlier, at import time in a `conftest.py` say.
Reading a field of a `Settings` object resolves it the same way, at the time of
reading.

`HEGEL_DEFAULT_PROFILE` and `HEGEL_CONFIG` also come from the environment, but
act on the second layer, choosing the default profile and the configuration
file. The [pytest options](pytest.md) act on it too.

For example, with this `hegel.toml`:

```toml
[profiles.ci]
test_cases = 1000
```

a test declared `@hegel.test` and run on a CI server with
`HEGEL_TEST_CASES=5000` resolves the `ci` profile (1000 test cases,
derandomized, database off) and the environment variable overrides
`test_cases` to 5000. A test declared `@hegel.test(test_cases=200)` in the same
run keeps its 200. Locally, without the variable, the first test runs 100 test
cases and the second 200, with the `development` profile's settings for
everything else.

## Profiles

A profile is a named set of changes to another profile: the settings it
overrides, plus the profile it *extends*. Resolving a profile follows that
chain down to the base settings and applies the changes from the bottom up.

### The reserved names

- **`base`** is the base settings from the table above. It ends every chain,
  so a profile that extends `base`, or a test that selects it, gets exactly the
  base settings plus its own overrides, whichever profile the environment
  would otherwise have chosen. (The `HEGEL_*` settings variables still apply
  over it, as over every profile.)
- **`default`** is the default profile: the one a run gets when nothing names
  a profile, and the one custom profiles extend when they don't say otherwise.
  It is an alias, resolved as described under
  [Choosing the default profile](#choosing-the-default-profile).

Neither can be modified or registered.

### The shipped profiles

Three profiles ship with Hegel. All three extend `base` directly.

| Profile | Overrides | When it is the environment's profile |
|---|---|---|
| `development` | nothing | Locally: whenever neither of the others applies. |
| `ci` | `derandomize = true`, `database = "disabled"`, `suppress_health_check = ["too_slow"]` | On a CI server, detected from `CI`, `GITHUB_ACTIONS`, `GITLAB_CI`, `BUILDKITE`, `CIRCLECI`, and the variables other common services set. |
| `workload` | `backend = "urandom"`, `database = "disabled"`, `suppress_health_check = ["all"]` | Inside [Antithesis](https://antithesis.com/), detected from `ANTITHESIS_OUTPUT_DIR`. See {doc}`../how-to/antithesis`. |

### Custom profiles and inheritance

Custom profiles are defined in `hegel.toml` or registered in Python. A
`hegel.toml` profile chooses its parent with `extends`. Without one, a custom
profile extends `default`, and a shipped profile extends `base`. So with

```toml
[profiles.nightly]
test_cases = 10000

[profiles.pinned]
extends = "base"
test_cases = 10000

[profiles.deep-ci]
extends = "ci"
test_cases = 100000
```

- `nightly` resolves as `nightly` → `ci` → `base` on a CI server and
  `nightly` → `development` → `base` locally: it gets the environment's
  behaviour plus more test cases, wherever it is selected.
- `pinned` resolves as `pinned` → `base` everywhere: 10000 test cases, a fresh
  seed and the database on, even on CI (unless a `HEGEL_*` variable says
  otherwise).
- `deep-ci` resolves as `deep-ci` → `ci` → `base` everywhere.

Because the shipped profiles are siblings, a change meant for every
environment does not go in `development`; it goes in a profile of its own,
which extends `base` and which the others name with `extends`:

```toml
[profiles.common]
extends = "base"
test_cases = 200

[profiles.development]
extends = "common"

[profiles.ci]
extends = "common"
```

A `hegel.toml` section for a shipped profile merges over the shipped
settings (`[profiles.ci]` with `print_blob = false` keeps derandomization and
the disabled database), and may set `extends` to change its parent, as above.

`common` has to extend `base` explicitly. Without that it would extend
`default`, which is `development` locally, and `development` extends `common`:
a cycle. An `extends` naming an unknown profile, or a cycle, is an error.
Every profile in a `hegel.toml` is checked whenever any profile is resolved,
so a broken profile fails loudly even before something selects it.

### Choosing the default profile

The `default` alias resolves to the first of these that is set:

1. {meth}`Settings.set_default_profile <hegel.Settings.set_default_profile>`,
   which the `--hegel-*` pytest options use.
2. The `HEGEL_DEFAULT_PROFILE` environment variable, when non-empty.
3. The top-level `default = "<profile>"` entry in `hegel.toml`.
4. The environment's profile: `workload` inside Antithesis, else `ci` on a CI
   server, else `development`.

Naming a profile that does not exist is an error, reported with the list of
known profiles.

```bash
HEGEL_DEFAULT_PROFILE=nightly pytest
```

### Selecting a profile for one test

{meth}`Settings.from_profile <hegel.Settings.from_profile>` resolves a profile
by name without changing what `default` is:

```python
@hegel.test(settings=hegel.Settings.from_profile("nightly"))
def test_thorough(tc): ...
```

The selected profile still sits on its usual parent, so `nightly` selected this
way on CI still inherits from `ci`.

## `hegel.toml`

### Discovery

Hegel looks for `hegel.toml` in the working directory of the test process and
then in each of its parent directories; the first file found is used, and
files are never merged. The file is read once per process.

To load a file somewhere else, set `HEGEL_CONFIG` to its path. A non-empty
`HEGEL_CONFIG` that cannot be read is an error.

### Format

The file has an optional top-level `default = "<profile>"` entry and
`[profiles.<name>]` tables whose entries are settings. Hegel is strict about
the contents: an unknown key, a value of the wrong type, or any other
top-level key is an error naming the file and line, as is malformed TOML.
Profile names use ASCII letters, digits, `-` and `_`, and `base` and `default`
cannot be sections.

```toml
default = "nightly"      # optional: the default profile for this project

[profiles.development]
test_cases = 200

[profiles.ci]            # merges onto the shipped ci profile
test_cases = 1000
print_blob = false

[profiles.nightly]
extends = "ci"
test_cases = 10000
seed = "none"
suppress_health_check = ["too_slow", "filter_too_much"]
phases = ["explicit", "reuse", "generate", "target", "shrink"]
database = "default"
backend = "default"
```

Every setting in the table at the top of this page is accepted, plus
`extends`. Two values exist to undo a parent's setting: `seed = "none"` clears
an inherited seed, and `database = "default"` restores the default database
after a parent turned it off or set a path. In `hegel.toml`,
`database = "disabled"` turns the database off, and
`suppress_health_check = ["all"]` suppresses every health check.

## Profiles in Python

{meth}`Settings.register_profile <hegel.Settings.register_profile>` resolves a
`Settings` value and stores the result, a complete snapshot of every setting,
under a name, for the rest of the process:

```python
hegel.Settings.register_profile(
    "nightly", hegel.Settings.from_profile("ci"), test_cases=10_000
)
```

A registered profile ends its chain, like `base`: it has no parent, because
the snapshot already holds every setting. Registering a shipped profile's name
replaces that profile completely. A `hegel.toml` section of the same name
still merges over the snapshot. Registering again under the same name replaces
the earlier snapshot.

{meth}`Settings.set_default_profile <hegel.Settings.set_default_profile>` makes
a profile the default for the process. The profile must already exist.

Make these calls before the tests they should affect run: in a `conftest.py`,
for example. A `Settings` value created before them, without
{meth}`~hegel.Settings.from_profile`, follows the new default profile, as it
resolves its settings each time it is used.

## Environment variables

| Variable | Effect |
|---|---|
| `HEGEL_DEFAULT_PROFILE` | The default profile, unless `Settings.set_default_profile` (or a pytest option) set one. |
| `HEGEL_CONFIG` | The path of the `hegel.toml` to load instead of searching for one. |
| `HEGEL_TEST_CASES` | Sets `test_cases` over the resolved profile. |
| `HEGEL_DATABASE` | Sets `database` over the resolved profile. |
| `HEGEL_STATISTICS` | Turns `show_statistics` on over the resolved profile. |
| `HEGEL_SEED` | Sets `seed` over the resolved profile. |
| `HEGEL_DERANDOMIZE` | Sets `derandomize` over the resolved profile. |
| `HEGEL_PRINT_BLOB` | Sets `print_blob` over the resolved profile. |
| `HEGEL_STOP_ON_FIRST_FAILURE` | Sets `stop_on_first_failure` over the resolved profile. |
| `HEGEL_UNBOUNDED_CHOICES` | Sets `unbounded_choices` over the resolved profile. |
| `HEGEL_NONDETERMINISM_STRICTNESS` | Sets `nondeterminism_strictness` over the resolved profile. |
| `ANTITHESIS_OUTPUT_DIR` | Selects the `workload` profile, and each test's verdict is reported to Antithesis. Must name an existing directory. |
| `CI`, `GITHUB_ACTIONS`, ... | Selects the `ci` profile. |
