# Run Hegel in CI

Hegel detects common CI services (from environment variables such as `CI`,
`GITHUB_ACTIONS`, `GITLAB_CI`, `BUILDKITE` and `CIRCLECI`) and uses the `ci`
settings profile there automatically. Compared with running locally, the
`ci` profile:

- derandomizes every test, so the same code always gets the same test cases
  and a CI failure can be reproduced by running the same commit again;
- turns off the example database, which a fresh CI machine would not keep;
- suppresses the `too_slow` health check, since CI machines are often slower
  and more variable than development ones.

Nothing needs configuring for this to work. The rest of this page covers
changing it.

## Reproducing a CI failure locally

A failure report ends with a `@hegel.reproduce_failure(...)` line. Copy that
decorator onto the test, below `@hegel.test`, to rerun exactly the failing
test case on your own machine. See {doc}`../tutorial/failures`.

## Running more test cases on CI

To test harder on CI than locally, change the `ci` profile in `hegel.toml`.
Your settings are merged into the shipped ones, so CI stays derandomized:

```toml
[profiles.ci]
test_cases = 1000
```

## A separate, longer run

For a nightly or weekly job that runs many more test cases, define a profile
that extends `ci`:

```toml
[profiles.nightly]
extends = "ci"
test_cases = 100000
derandomize = false
```

and select it in that job only:

```bash
pytest --hegel-profile=nightly
```

Turning `derandomize` off makes each nightly run explore different test
cases, rather than repeating the same ones; failure reports still say how to
reproduce what they found.

For a one-off change, the environment variables in
{doc}`../reference/settings` set individual settings without a profile:

```bash
HEGEL_TEST_CASES=100000 pytest
```

## Using a different profile on CI

If your CI service isn't detected, or you want a profile other than `ci`
there, set `HEGEL_DEFAULT_PROFILE` in the CI environment:

```bash
HEGEL_DEFAULT_PROFILE=ci pytest
```
