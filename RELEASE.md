RELEASE_TYPE: patch

This patch makes `gs::from_regex` panic with an invalid-argument error, naming the pattern and the alphabet, when its `alphabet` can supply no string matching the pattern (`abc` over `def`, `[^a]` over `a`, `\d` over letters, ...), and likewise for a pattern no string matches at all, such as `(?!)`. Before, every draw was rejected and the run failed `HealthCheck::FilterTooMuch` or reported the test as unsatisfiable instead of pointing at the mistake.
