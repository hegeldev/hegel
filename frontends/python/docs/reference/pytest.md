# pytest integration

Installing Hegel registers a [pytest](https://pytest.org) plugin, so Hegel
tests run under pytest without any configuration. A function decorated with
{func}`@hegel.test <hegel.test>` is an ordinary test function to pytest: its
parameters after the test case are pytest fixtures, and it can be
parametrised and marked like any other test.

## Markers

The plugin marks every Hegel test with the `hegel` marker, so you can select
or deselect them:

```bash
pytest -m hegel        # only Hegel tests
pytest -m "not hegel"  # everything else
```

## Command-line options

These options change the settings of every Hegel test in the run:

`--hegel-profile NAME`
: Start from the named settings profile, instead of the default one.

`--hegel-setting KEY=VALUE`
: Set any setting, named by its `hegel.toml` key, with the value spelled as
  in `hegel.toml` (see {doc}`settings`): `--hegel-setting test_cases=1000`,
  `--hegel-setting suppress_health_check=too_slow,filter_too_much`,
  `--hegel-setting unbounded_choices=true`. Booleans take `true`, `1`, `yes`,
  `false`, `0` or `no`, and lists their names separated by commas. Repeat the
  option to set several. Hegel's engine parses the values, so they read the
  same as in `hegel.toml` and the `HEGEL_*` environment variables, and an
  unknown key or value is a usage error that lists what is accepted.

`--hegel-seed SEED`
: Fix the seed: an integer from 0 to 2<sup>64</sup> - 1, or `none` for a
  fresh random seed.

`--hegel-verbosity {quiet,normal,verbose,debug}`
: Set the verbosity.

`--hegel-derandomize`
: Derive each test's seed from the test itself, so that every run of a test
  generates the same test cases.

`--hegel-database PATH`
: Keep the example database in `PATH`. `--hegel-database=` (empty) or
  `--hegel-database=disabled` turns the database off, and
  `--hegel-database=default` selects the default directory, as in
  `hegel.toml`; write `./default` for a directory of that name.

The options act on the default profile: Hegel builds a profile from the one
named by `--hegel-profile` (or the default profile) with the other options
applied, the `--hegel-setting` options first and then the others, and makes it
the default for the run. So settings written in a test, or given to a
{class}`~hegel.Settings` object, still take precedence over the options, and so
do the `HEGEL_*` environment variables in {doc}`settings`, which apply on top of
whichever profile is the default. Everything a `Settings` object leaves unset
follows the options, even for one created at import time in a `conftest.py`.

## Detecting Hegel tests

A function decorated with `@hegel.test` has an `is_hegel_test` attribute set
to `True`, and the undecorated function as `hegel_inner_test`.
