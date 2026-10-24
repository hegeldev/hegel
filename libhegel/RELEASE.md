RELEASE_TYPE: patch

This patch adds `HEGEL_MAX_SIZE_DEFAULT`, which a frontend passes as the `max_size` of `hegel_string_generator_text` or `hegel_generate_bytes` when the user gave no maximum. libhegel then applies its default of 100, or `min_size + 100` when `min_size` is above 100, so every frontend's text and binary generators share one default instead of each hard-coding its own. `UINT64_MAX` still means no upper bound.

It also adds `HEGEL_FILTER_ATTEMPTS`, the number of times a filtered generator draws a value before it rejects the test case (3), so that every frontend's `filter` uses the engine's number.

It also adds `hegel_settings_set_from_string`, which sets one setting from its `hegel.toml` key and a value given as text, parsed by the same parser `hegel.toml` uses: integers in decimal, booleans as the `HEGEL_*` environment variables take them, and lists such as `phases` and `suppress_health_check` as comma-separated names. A frontend's command-line interface can pass each flag through it, so every setting is settable from the command line without the frontend knowing each setting's vocabulary. `unbounded_choices` is settable this way too.
