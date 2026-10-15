RELEASE_TYPE: patch

This patch adds the `stop_on_first_failure` setting (`hegel_settings_set_stop_on_first_failure` / `hegel_settings_get_stop_on_first_failure`, the `stop_on_first_failure` profile key, the `HEGEL_STOP_ON_FIRST_FAILURE` environment variable; on by default). Turned off, a run uses its whole `test_cases` budget before reporting instead of stopping at the first failure: a failure replayed from the database no longer skips generation, every stored failure is replayed, and failing test cases count against the budget like valid ones, so a test that fails on every input still ends.

Every distinct failure found is persisted to the database either way. With `report_multiple_failures` off, a run that found several failures now shrinks and reports only the smallest of them and persists the rest exactly as found, where it used to shrink every one before reporting one.
