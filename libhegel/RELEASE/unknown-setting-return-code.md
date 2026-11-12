RELEASE_TYPE: minor

This release changes `hegel_settings_set_from_string` to return the new result code `HEGEL_E_UNKNOWN_SETTING` when its key names no setting ([#77](https://github.com/hegeldev/hegel/issues/77)). It used to return `HEGEL_E_INVALID_ARG` for both an unknown key and a value its key does not accept, so a binding could only tell the two apart by matching the error message. A value outside its key's vocabulary is still `HEGEL_E_INVALID_ARG`, as is a key or value that is not valid UTF-8, and the error messages are unchanged.

A binding that treats `HEGEL_E_INVALID_ARG` from this function as a user error should now treat `HEGEL_E_UNKNOWN_SETTING` the same way:

```c
hegel_result_t rc = hegel_settings_set_from_string(ctx, s, key, value);
if (rc == HEGEL_E_UNKNOWN_SETTING) {
    /* key is not a setting */
} else if (rc == HEGEL_E_INVALID_ARG) {
    /* value is not valid for key */
}
```
