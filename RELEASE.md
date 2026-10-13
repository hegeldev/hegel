RELEASE_TYPE: patch

This patch fixes a test whose every input is rejected by `assume()` or `reject()` passing when `HealthCheck::FilterTooMuch` is suppressed. Such a run now fails with `Unsatisfiable`, as a test that rejects its input without drawing anything already did, since no valid input was ever generated; the same applies to a test whose every input overruns the choice limit with `HealthCheck::TestCasesTooLarge` suppressed.

It also fixes a test that accepts only a rare input — say one of seven equally likely values — sometimes stopping after about twenty rejections, passing vacuously under suppression or failing `FilterTooMuch` with a message claiming the whole search space had been filtered. Generation now continues until the test-case budget or the invalid budget is spent.
