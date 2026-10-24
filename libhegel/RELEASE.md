RELEASE_TYPE: patch

This patch adds `HEGEL_MAX_SIZE_DEFAULT`, which a frontend passes as the `max_size` of `hegel_string_generator_text` or `hegel_generate_bytes` when the user gave no maximum. libhegel then applies its default of 100, or `min_size + 100` when `min_size` is above 100, so every frontend's text and binary generators share one default instead of each hard-coding its own. `UINT64_MAX` still means no upper bound.

It also adds `HEGEL_FILTER_ATTEMPTS`, the number of times a filtered generator draws a value before it rejects the test case (3), so that every frontend's `filter` uses the engine's number.
