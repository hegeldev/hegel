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

`--hegel-seed SEED`
: Fix the seed.

`--hegel-verbosity {quiet,normal,verbose,debug}`
: Set the verbosity.

`--hegel-derandomize`
: Derive each test's seed from the test itself, so that every run of a test
  generates the same test cases.

`--hegel-database PATH`
: Keep the example database in `PATH`. `--hegel-database=` (empty) turns the
  database off.

The options act on the default profile: Hegel builds a profile from the one
named by `--hegel-profile` (or the default profile) with the other options
applied, and makes it the default for the run. So settings written in a test
still take precedence over the options, and so do the `HEGEL_*` environment
variables in {doc}`settings`, which apply on top of whichever profile is the
default.

## Detecting Hegel tests

A function decorated with `@hegel.test` has an `is_hegel_test` attribute set
to `True`, and the undecorated function as `hegel_inner_test`.
