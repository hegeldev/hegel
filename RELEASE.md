RELEASE_TYPE: minor

This release removes the `hegel.hypothesis` namespace, the reimplementation of
Hypothesis's API (`@given`, `strategies`, `settings`, `RuleBasedStateMachine`,
...) on top of Hegel, along with the pytest plugin's `--hypothesis-profile`,
`--hypothesis-seed` and `--hypothesis-verbosity` options and its `hypothesis`
marker. Write tests against the `hegel` API instead; `docs/comparison-with-hypothesis.md`
maps Hypothesis's API onto it. Hypothesis's own test suite now runs against the
`hegel` API directly.
