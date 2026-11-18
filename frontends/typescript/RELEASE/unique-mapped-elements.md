RELEASE_TYPE: patch

This patch fixes `gs.arrays(..., { unique: true })` returning duplicate elements, and `gs.sets` and `gs.maps` returning fewer than `minSize` elements, when the elements or keys come from a `.map()` that sends different values to the same result, such as `gs.integers().map((n) => n % 3)` ([#96](https://github.com/hegeldev/hegel/issues/96)). Sets and maps now compare elements and keys structurally, as `unique` arrays do, whatever generator they come from.
