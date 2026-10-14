RELEASE_TYPE: patch

This patch makes `hegel_string_generator_regex` fail with an invalid-argument error, naming the pattern and the alphabet, when the given alphabet can supply no string matching the pattern (`abc` over `def`, `[^a]` over `a`, `\d` over letters, ...), and likewise for a pattern no string matches at all, such as `(?!)`. Before, the generator was built and every draw from it was rejected, so the run failed the filter-too-much health check or reported the test as unsatisfiable instead of pointing at the mistake.
