RELEASE_TYPE: patch

This patch sets a lower bound of 1% for the probability of checking a sampled invariant. Previously, the probability was always
1 / step_count, so using a very large step count, such as in Antithesis, meant sampled invariants almost never ran.
