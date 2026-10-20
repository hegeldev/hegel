RELEASE_TYPE: minor

This release makes `text()`, `characters()`, `from_regex()` and `from_lark()` follow this Python's Unicode version (`unicodedata.unidata_version`) rather than the engine's newer tables. A `characters()` or `text()` generator with category constraints no longer draws a character this Python does not know yet or puts in another category, and a `from_regex()` draw, or a grammar terminal's, always matches the pattern as this Python's `re` reads it: `\w`, `\d` and `\s`, word boundaries and `re.IGNORECASE` all follow the host's tables.

`from_regex()` also accepts the `\z` end-of-string anchor that `re` gained in Python 3.14.

The API's docstrings are much expanded, for the new documentation site at <https://hegel.readthedocs.io>: every generator documents its arguments, its defaults, how its values shrink and what is an invalid argument. Three descriptions were also wrong and are corrected: `uuids()` without `version=` generates arbitrary 128-bit values, most of which have no RFC 4122 version; invariants run on the initial and final state and only sometimes in between, unless `always_run=True`; and `Settings()` resolves the default profile from `set_default_profile` first, then `HEGEL_DEFAULT_PROFILE`, then `hegel.toml`.

`Settings` gains `stop_on_first_failure` (default `True`), the engine setting the Rust library already exposes: with it off a run uses its whole `test_cases` budget before reporting. A `hegel.toml` profile setting it was previously ignored by Python runs.

`label_from_name` and `combine_labels` are no longer exported from `hegel.generators`. They are implementation details, not part of the public API.
