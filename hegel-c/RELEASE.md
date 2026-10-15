RELEASE_TYPE: patch

`hegel_pool_restore(ctx, tc, pool, variable_id)` puts back a variable that a consuming `hegel_pool_generate` removed from the pool, so a frontend can leave the pool as it was when the value drawn with it is thrown away unused, as a filter does with a rejected attempt. A variable that is not a consumed member of the pool is an argument error, and libhegel never puts a variable back on its own.
