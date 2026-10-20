# Configuring tests

Hegel's behaviour is controlled by *settings*: how many test cases to run,
how much to print, where to keep the example database, and more. This page
covers the settings you're most likely to change and the ways to change them.
{doc}`../reference/settings` describes every setting and exactly how the
different ways of setting them combine.

## Settings for one test

Pass settings as keyword arguments to `@hegel.test`. The most common one is
`test_cases`, the number of test cases to run, which defaults to 100:

```python
@hegel.test(test_cases=500)
def test_integers_many(tc):
    n = tc.draw(gs.integers())
    assert n == n
```

Settings can also be collected in a {class}`hegel.Settings` object and passed
as `settings=`, which lets several tests share them:

```python
thorough = hegel.Settings(test_cases=1000, report_multiple_failures=True)


@hegel.test(settings=thorough)
def test_one(tc): ...


@hegel.test(settings=thorough, verbosity="verbose")
def test_two(tc): ...
```

Keyword arguments given alongside `settings=` override its fields. A setting
whose value is an enum, like `verbosity`, takes either the enum member
({attr}`hegel.Verbosity.verbose`) or its name (`"verbose"`).

Some settings you may want:

`test_cases`
: How many test cases to run.

`verbosity`
: `"quiet"`, `"normal"`, `"verbose"` (print every test case as it runs) or
  `"debug"`.

`seed`
: A fixed seed, to make a run repeatable. `derandomize=True` instead derives
  the seed from the test's name, so every run of a test is the same.

`database`
: The directory for the example database, or `""` to turn it off.

`suppress_health_check`
: Health checks to turn off; see {doc}`../how-to/health-checks`.

`show_statistics`
: Print statistics about what the test generated; see
  {doc}`../how-to/statistics`.

## Changing settings without editing tests

Settings written in the test always apply. Everything else comes from a
*settings profile*, a named collection of settings that applies to every test
in a run. Hegel ships with three, and picks one for you:

`development`
: Hegel's default settings, used when running locally.

`ci`
: Used automatically on a CI server. It derandomizes every test, so CI runs
  are repeatable, turns off the example database, and suppresses the
  `too_slow` health check.

`workload`
: Used automatically inside [Antithesis](https://antithesis.com).

You can modify these or add your own in a `hegel.toml` file at the root of
your project:

```toml
[profiles.development]
test_cases = 200

[profiles.nightly]
test_cases = 10000
```

Select a profile for a run with pytest's `--hegel-profile` option, or the
`HEGEL_DEFAULT_PROFILE` environment variable:

```bash
pytest --hegel-profile=nightly
HEGEL_DEFAULT_PROFILE=nightly pytest
```

A few settings also have environment variables of their own, which are the
quickest way to adjust one run. For example:

```bash
HEGEL_TEST_CASES=10000 pytest
```

runs 10,000 test cases of every test that doesn't set `test_cases` itself.
{doc}`../reference/settings` lists all of them, and
{doc}`../reference/pytest` lists the pytest options.

You can also define profiles in Python, with
{meth}`Settings.register_profile <hegel.Settings.register_profile>`, and
choose one with
{meth}`Settings.set_default_profile <hegel.Settings.set_default_profile>`,
for example in a `conftest.py`:

```python
import hegel

hegel.Settings.register_profile("thorough", test_cases=10_000)
hegel.Settings.set_default_profile("thorough")
```
