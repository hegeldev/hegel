RELEASE_TYPE: patch

This patch adds `Generators.optionalInts`, `Generators.optionalLongs` and `Generators.optionalDoubles`,
which generate the primitive optionals `OptionalInt`, `OptionalLong` and `OptionalDouble` the same way
`Generators.optional` generates `Optional<T>` ([#27](https://github.com/hegeldev/hegel-java/issues/27)).
Each one generates either an empty optional or a value from the generator you pass in, and shrinks
toward empty.

```java
Generator<OptionalLong> max = Generators.optionalLongs(Generators.longs().min(0).max(100));
```

`Generators.forType` and `Generators.records` now also derive generators for these types, so a record
like `record Stats(long count, OptionalLong max, OptionalDouble mean)` no longer fails with
`No default generator for java.util.OptionalLong`.
