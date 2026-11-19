RELEASE_TYPE: patch

This patch names the single run of a `@HegelTest` method after the test, its `@DisplayName` or the method name, instead of `hegel`. JUnit XML reports, and the CI test views built on them, used to list every property test as `hegel`; they now show which property each entry is ([#105](https://github.com/hegeldev/hegel/issues/105)).
