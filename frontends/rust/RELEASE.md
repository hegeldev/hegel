RELEASE_TYPE: patch

This patch updates the `hegeltest-c` dependency to the new libhegel release, which fixes a shrinking regression: with `report_multiple_failures` off, a failure found while shrinking another could be reported without being shrunk itself. `from_regex()` also now accepts the `\z` end-of-string anchor.

`generators::label_from_name` and `generators::combine_labels` are now hidden from the documentation. They are implementation details of the derive macros, not part of the public API.
