RELEASE_TYPE: patch

This patch fixes integer shrinking stopping short of the minimum on some values. A bounded integer whose failing values are a sparse set of multiples, such as the multiples of six, could stall at a value none of the shrinker's moves could leave: a search over `0..=100` for a positive multiple of six reported 78 rather than 6 from about one seed in twenty-five. The shrinker now also tries dividing the distance from `shrink_towards` by each of its prime factors, which steps straight from 78 to 6.
