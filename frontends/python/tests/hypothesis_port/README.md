# Ported Hypothesis test suite

These tests are ported from Hypothesis's own test suite
(`hypothesis-python/tests` at Hypothesis 6.168.5) and run against the `hegel`
API: `@given(st.integers())` on `def test(x)` becomes `@hegel.test` on
`def test(tc)` with `x = tc.draw(gs.integers())`, and so on.

Conventions:

- Tests of Hypothesis internals (`hypothesis.internal.*`, conjecture, the
  shrinker, observability, the database backend, ghostwriter, extras) and of
  features Hegel does not have (`deadline`, `find`, `.example()`, `@seed`,
  `shared`, `randoms`, `Bundle`-specific stateful features, ...) are not
  ported; each file notes what was dropped from the original and why.
- Tests of behaviour Hegel has but does differently are kept and marked
  `pytest.mark.xfail(reason=...)` (strict, so they flag when the engine or the
  generators change). Engine bugs found this way are recorded in
  `docs/engine-issues.md`.
- `common/` holds the helpers the tests share: `debug.minimal` and friends
  search a generator by running a Hegel test, and `utils.capture_out` collects
  Hegel's output.

Run them with `just check-hypothesis-port`.
