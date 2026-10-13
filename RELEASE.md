RELEASE_TYPE: patch

This patch improves the shrinking of bounded integers. A `gs::integers()` draw whose failing values are a sparse set of multiples, such as the multiples of six, could stop short of the minimum at a value none of the shrinker's moves could leave: a search over `0..=100` for a positive multiple of six reported 78 rather than 6 from about one seed in twenty-five. The shrinker now also tries dividing the distance from the shrink target by each of its prime factors, so such a search reports 6.
