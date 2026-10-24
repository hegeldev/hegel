RELEASE_TYPE: patch

This patch makes the `unbounded_choices` setting reachable the way the other settings are: profiles in `hegel.toml` accept an `unbounded_choices` key (a boolean), and the `HEGEL_UNBOUNDED_CHOICES` environment variable (`true`, `1` or `yes`, or `false`, `0` or `no`) sets it over the resolved profile. Previously it could only be set through `hegel_settings_set_unbounded_choices` or carried by a registered profile.
