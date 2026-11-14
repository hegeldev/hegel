RELEASE_TYPE: minor

This release changes `Libhegel.settingsFromString` in `dev.hegel:hegel-lowlevel` to return the new `Abi.E_UNKNOWN_SETTING` when its key names no setting, following libhegel ([#77](https://github.com/hegeldev/hegel/issues/77)). It used to return `Abi.E_INVALID_ARG` for that case as well as for a value outside the key's vocabulary, which it still does. Code that checks for `Abi.E_INVALID_ARG` to catch an unknown key should check for `Abi.E_UNKNOWN_SETTING` instead. `Hegel.main`'s command line is unchanged.
