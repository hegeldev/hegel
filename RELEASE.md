RELEASE_TYPE: patch

This patch makes `tc.target()` steer generation from early in a large run instead of only from halfway through its budget, so a test with `test_cases` of 1,000 or more whose failure targeting leads to is found within a few hundred cases rather than after thousands.
