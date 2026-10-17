RELEASE_TYPE: patch

`hegel_pool_restore(ctx, tc, pool, variable_id)` puts back a variable that a consuming `hegel_pool_generate` removed from the pool, so a frontend can leave the pool as it was when the value drawn with it is thrown away unused, as a filter does with a rejected attempt. A variable that is not a consumed member of the pool is an argument error, and libhegel never puts a variable back on its own.

`hegel_tzdb_version(ctx, out_version)` reports the IANA time zone database release bundled in libhegel, such as `"2026e"`, as a static string, so a frontend can tell which zones and transitions the engine's forthcoming time zone generators will draw from. No generator uses the bundled database yet.
