RELEASE_TYPE: patch

This patch updates the internal libhegel bindings for libhegel's new `HEGEL_E_UNKNOWN_SETTING` result code ([#77](https://github.com/hegeldev/hegel/issues/77)), which it returns when a setting key given as text names no setting. The public API is unchanged.
