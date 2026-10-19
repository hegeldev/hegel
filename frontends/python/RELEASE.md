RELEASE_TYPE: patch

This patch makes `text()`, `characters()` and `from_regex()` follow this Python's Unicode version (`unicodedata.unidata_version`) rather than the engine's newer tables. A `characters()` or `text()` generator with category constraints no longer draws a character this Python does not know yet or puts in another category, and a `from_regex()` draw always matches the pattern as this Python's `re` reads it: `\w`, `\d` and `\s`, word boundaries and `re.IGNORECASE` all follow the host's tables.

`from_regex()` also accepts the `\z` end-of-string anchor that `re` gained in Python 3.14.
