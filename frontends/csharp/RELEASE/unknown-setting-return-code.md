RELEASE_TYPE: patch

This patch updates the .NET frontend for libhegel's new `HEGEL_E_UNKNOWN_SETTING` result code ([#77](https://github.com/hegeldev/hegel/issues/77)), which libhegel returns when a setting key given as text names no setting. `Settings.Set` with an unknown key still throws `ArgumentException`.
