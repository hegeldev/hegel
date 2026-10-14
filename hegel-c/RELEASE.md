RELEASE_TYPE: patch

This patch adds `hegel_run_result_error_kind`, which reports why a run ended in `HEGEL_RUN_STATUS_ERROR` as a `hegel_run_error_kind_t` (a failed health check, a flaky or nondeterministic test under `error` strictness, an unsatisfiable test, a usage error, or an internal error) and, for a failed health check, which check as its `hegel_health_check_t` value. Bindings previously had to classify run errors by the wording of `hegel_run_result_error`, which a rewording could silently break; the message is unchanged and remains the diagnostic to attach. Either out parameter may be NULL when only the other is wanted.

```c
hegel_run_error_kind_t kind;
uint32_t check;
hegel_run_result_error_kind(ctx, result, &kind, &check);
if (kind == HEGEL_RUN_ERROR_HEALTH_CHECK && check == HEGEL_HC_FILTER_TOO_MUCH) { ... }
```
