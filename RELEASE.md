RELEASE_TYPE: patch

This patch picks up two generation fixes from the engine: a hundred test cases over a small finite space (a `one_of` over a few `sampled_from` lists, say) now reach every value instead of repeating the simplest one, and `gs::integers::<u128>()` over a range too wide for `i128` spreads its values over the range's orders of magnitude as narrower ranges do rather than clustering near the top. Seeded runs that draw such integers, or that spend cases on span mutation, generate different values from this version on.
