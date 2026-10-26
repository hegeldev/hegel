RELEASE_TYPE: patch

This patch adds `HEGEL_MAX_SIZE_DEFAULT`, which a frontend passes as the `max_size` of `hegel_string_generator_text` or `hegel_generate_bytes` when the user gave no maximum, or of `hegel_new_collection` for a collection that builds text or bytes, such as text whose characters come from an arbitrary generator. libhegel then applies its default of 100, or `min_size + 100` when `min_size` is above 100, so every frontend's text and binary generators share one default instead of each hard-coding its own. `UINT64_MAX` still means no upper bound. The sentinel is `UINT64_MAX - 1`, so that value is reserved: a frontend passing a user's explicit maximum should pass `UINT64_MAX` for anything at or above `HEGEL_MAX_SIZE_DEFAULT`, which no allocation could reach anyway.

It also adds `HEGEL_FILTER_ATTEMPTS`, the number of times a filtered generator draws a value before it rejects the test case (3), so that every frontend's `filter` uses the engine's number.

It also adds `hegel_settings_set_from_string`, which sets one setting from its `hegel.toml` key and a value given as text, in the vocabulary `hegel.toml` uses for that key, with these spellings for values that are not strings there: integers as decimal digits, optionally after a `+` (no `_` separators or `0x` prefix), booleans as the `HEGEL_*` environment variables take them, and lists such as `phases` and `suppress_health_check` as comma-separated names. A frontend's command-line interface can pass each flag through it, so every setting is settable from the command line without the frontend knowing each setting's vocabulary. `unbounded_choices` and the database key (`database_key`) are settable this way too; only the test location, which has four parts, still needs `hegel_settings_set_test_location`.

It also moves the splicing of explicit grammar terminals into libhegel. A frontend drawing a grammar string whose explicit terminals take values from the user's own generators now starts a draw with `hegel_grammar_draw_new`, calls `hegel_grammar_draw_next` to learn which terminal needs a value next, draws that value and hands it over with `hegel_grammar_draw_provide`, and reads the finished string with `hegel_grammar_draw_result` once `hegel_grammar_draw_next` returns `HEGEL_E_NOT_FOUND`:

```c
hegel_grammar_draw_t *draw;
hegel_grammar_draw_new(ctx, grammar, 0, &draw);
size_t terminal;
hegel_result_t rc;
while ((rc = hegel_grammar_draw_next(ctx, tc, draw, &terminal)) == HEGEL_OK) {
    /* draw a value for explicit_names[terminal] */
    hegel_grammar_draw_provide(ctx, draw, value);
}
if (rc == HEGEL_E_NOT_FOUND)
    hegel_grammar_draw_result(ctx, draw, &string);
hegel_grammar_draw_free(ctx, draw);
```

libhegel replaces each placeholder marker with its value, keeps the whole string, values included, under the size cap by drawing it again with less room for the grammar's own text when the values overflow it, and rejects the test case after `HEGEL_FILTER_ATTEMPTS` attempts that do not fit, so a frontend no longer reimplements any of this. The draw's `max_size` is read as `hegel_generate_grammar` reads its own: 0 means the grammar's cap, and a cap below the grammar's smallest string or above its cap is read as that. A draw belongs to the choice stream of the test-case handle it is first advanced with: `hegel_grammar_draw_next` accepts that handle and its block handles from `hegel_test_case_block`, returns `HEGEL_E_INVALID_ARG` for a handle on another stream (a clone, or a later test case's handle), and a draw handle used from two threads at once returns `HEGEL_E_CONCURRENT_USE`. `hegel_generate_grammar` and `hegel_grammar_placeholder` still work as before.

The header's list of constructors and their matching frees now includes `hegel_settings_new_for_profile`, `hegel_new_recursion` and the new grammar draw functions, and the documentation of `hegel_stop_span` no longer claims that libhegel retries a discarded span itself: the caller draws again.

It also makes the `unbounded_choices` setting reachable the way the other settings are: profiles in `hegel.toml` accept an `unbounded_choices` key (a boolean), and the `HEGEL_UNBOUNDED_CHOICES` environment variable (`true`, `1` or `yes`, or `false`, `0` or `no`) sets it over the resolved profile. Previously it could only be set through `hegel_settings_set_unbounded_choices` or carried by a registered profile.
