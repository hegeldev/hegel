# Ported Hypothesis test suite

These tests are ported from Hypothesis's own test suite
(`hypothesis-python/tests` at Hypothesis 6.168.5) and run against
`hegel.hypothesis`, the Hypothesis-compatible API implemented on Hegel.

Conventions:

- Imports of `hypothesis` become `hegel.hypothesis`; `tests.common` becomes
  `tests.hypothesis_port.common`.
- Tests of Hypothesis internals (`hypothesis.internal.*`, conjecture, the
  shrinker, observability, the database backend, ghostwriter, extras) are not
  ported.
- Tests of behaviour Hegel does not reproduce are kept and marked with
  `pytest.mark.xfail(reason=...)` (strict) when the feature is a plausible
  future addition, or `pytest.mark.skip(reason=...)` when it is out of scope.
- `GAPS.md` lists the differences found while porting.
