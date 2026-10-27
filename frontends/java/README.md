> [!IMPORTANT]
> We're excited you're checking out Hegel! Hegel is in beta, and we'd love for you to try it and [report any feedback](https://github.com/hegeldev/hegel/issues/new).
>
> As part of our beta, we may make breaking changes if it makes Hegel a better property-based testing library. If that instability bothers you, please check back in a few months for a stable release!
>
> See https://hegel.dev/compatibility for more details.

# Hegel for Java

* [Documentation](https://javadoc.io/doc/dev.hegel/hegel)
* [Website](https://hegel.dev)

Hegel is a property-based testing library for Java. Hegel is based on [Hypothesis](https://github.com/hypothesisworks/hypothesis), using the [Hegel protocol](https://hegel.dev/).

Instead of writing tests with hand-picked example inputs, you describe a *property* that should hold for all inputs and let Hegel generate inputs to try to falsify it. When it finds a failing input it automatically **shrinks** it to a minimal counterexample.

## Installation

Hegel for Java ships as two interchangeable artifacts with the same API — pick the one that
matches your JVM:

- **`dev.hegel:hegel`** — requires **Java 22+**; binds the engine over the
  [Foreign Function & Memory API](https://docs.oracle.com/en/java/javase/22/core/foreign-function-and-memory-api.html)
  with no extra dependencies.
- **`dev.hegel:hegel-jna`** — requires **Java 17+**; binds the engine over
  [JNA](https://github.com/java-native-access/jna).

Add the dependency with Maven:

```xml
<dependency>
  <groupId>dev.hegel</groupId>
  <artifactId>hegel</artifactId> <!-- or "hegel-jna" for Java 17-21 -->
  <version>0.10.3</version>
  <scope>test</scope>
</dependency>
```

or with Gradle:

```kotlin
testImplementation("dev.hegel:hegel:0.10.3") // or "dev.hegel:hegel-jna:0.10.3"
```

Depend on exactly one of the two — they contain the same classes and differ only in how they call
the native engine. The engine is bundled in both jars for Linux (x86-64 and arm64), macOS (Apple
Silicon), and Windows (x86-64 and arm64). Both pull in a third, small artifact,
`dev.hegel:hegel-lowlevel`, which holds the binding contract; you never need to depend on it
directly unless you are [binding Hegel yourself](#binding-hegel-yourself).

Because Hegel calls native code, pass `--enable-native-access=ALL-UNNAMED` to silence the JVM's
native-access warning — printed by JDK 22+ for `hegel` (FFM) and by JDK 24+ for `hegel-jna` (JNA,
under [JEP 472](https://openjdk.org/jeps/472)). The flag is accepted on every supported JDK
(17+), so it is safe to set unconditionally. With Maven Surefire:

```xml
<argLine>--enable-native-access=ALL-UNNAMED</argLine>
```

## Quickstart

Here's a quick example of how to write a Hegel test:

```java
import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static org.junit.jupiter.api.Assertions.assertEquals;

import dev.hegel.HegelTest;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;

class SortTest {
  static List<Integer> mySort(List<Integer> xs) {
    return new ArrayList<>(new TreeSet<>(xs)); // oops: a TreeSet removes duplicates
  }

  @HegelTest
  void sortingPreservesLength(TestCase tc) {
    List<Integer> xs = tc.draw(lists(integers()), "xs");
    assertEquals(xs.size(), mySort(xs).size());
  }
}
```

This test will fail when run with `mvn test`! Hegel will produce a minimal failing test case for us:

```
var xs = [0, 0];

org.opentest4j.AssertionFailedError: expected: <2> but was: <1>
```

Hegel reports the minimal example showing that our sort is incorrectly dropping duplicates: `[0, 0]`, two equal elements, which `mySort` collapses into one. If we replace the `TreeSet`-based body of `mySort()` with a sort that keeps duplicates, this test will then pass.

The optional `"xs"` label passed to `draw` names the value in the falsifying-example output. The generator prints the value as it draws it, so `draw` takes a `PrintableGenerator`, and drawing a generator that cannot print is a compile-time error. Every built-in leaf generator is printable, and a structural one (`lists`, `optional`, `tuples`, `oneOf`, `filter`, ...) is printable exactly when its parts are; make a hand-written generator printable by implementing `PrintableGenerator` or with `printWith`, `printAsRepr` or `map(...).printAsCall(...)`, or draw it with `drawSilent`. Declare a printable generator you keep in a variable as a `PrintableGenerator<T>` (or the builder type its factory returns), not a `Generator<T>`, so that it can still be drawn. See the [API documentation](https://javadoc.io/doc/dev.hegel/hegel) for a full tour of generators, combinators, control functions, and settings.

## Generators

`Generators` has a generator for each common Java type, plus combinators (`map`, `filter`,
`flatMap`, `oneOf`, `lists(...).unique()`, `records(...)`, ...). A few that go beyond the basics:

```java
BigInteger n = tc.draw(bigIntegers().min(BigInteger.ONE.shiftLeft(200)));   // past the range of long
List<String> order = tc.draw(permutations(List.of("a", "b", "c")));         // shrinks toward the original order
String expr = tc.draw(fromLark("""
        ?start: sum
        ?sum: NUMBER | sum "+" NUMBER
        %import common.NUMBER
        """));                                                                 // strings matching a Lark grammar
Object tree = tc.draw(recursive(
        integers().map(x -> (Object) x),
        children -> lists(children).map(xs -> (Object) xs)));                  // recursive data, such as trees
```

## Explicit examples

Give values a test must always be run with, before any are generated. A draw takes the value named
for it, or else the example's next positional value:

```java
static List<Example> edgeCases() {
  return List.of(Example.of(List.of()), Example.of().with("xs", List.of(0, 0)));
}

@HegelTest(examples = "edgeCases")
void sortingPreservesLength(TestCase tc) { ... }
```

or `new Settings().examples(...)` with `Hegel.test`. An example that does not fit the test, or that
the test rejects with `assume` or `reject`, is a usage error: explicit test cases must not be rejected.

## Settings and profiles

Every setting has an `@HegelTest` attribute and a `Settings` method, and anything a test leaves
unset comes from the engine's settings profile: a `hegel.toml` file, the `HEGEL_*` environment
variables, or the shipped `development`, `ci` and `workload` profiles, picked automatically. Reach
any engine setting by its `hegel.toml` key with `Settings.set`, and name, register and select
profiles from Java:

```java
Hegel.test(body, new Settings().set("suppress_health_check", "too_slow"));

Settings.registerProfile("nightly", Settings.fromProfile("ci").testCases(10_000));
Settings.setDefaultProfile("nightly");

@HegelTest(profile = "ci")
void property(TestCase tc) { ... }
```

## Reproducing failures

A failure report ends with a reproducer, such as `reproduceFailure = "..."` for an `@HegelTest`.
Add it to the annotation (or call `Settings.reproduceFailure(...)` for `Hegel.test`) to replay
exactly that failure instead of generating; give several blobs to replay several failures at once.

## Stateful testing

Model-based tests drive a state machine whose `@Rule` methods are the actions and whose `@Invariant`
methods are the properties, with the engine choosing and shrinking the action sequence:

```java
Stateful.run(new IntegerStack(), tc);
```

Run the same machine with a concurrency bound to look for races: the engine draws how many worker
threads apply the rules at once, `@Rule(group = "...")` says which rules may overlap, and a
`ConcurrentPool` passes generated values between concurrent rules.

```java
Stateful.run(new Counter(tc), tc, Stateful.options().maxConcurrency(4));
```

See the `Stateful` Javadoc for complete sequential and concurrent examples. The concurrent one, a
counter that loses updates under contention, is checked in as a runnable demonstration that is
skipped unless asked for; run it from a checkout with

```bash
mvn test -pl hegel -am -Dtest=ConcurrentCounterExample -Dhegel.examples=true -Dsurefire.failIfNoSpecifiedTests=false
```

(or `-pl hegel-jna -am` for the JNA frontend) and it fails with a report of the rounds and the
worker-stamped rule and draw lines that led to the lost update.

## Using Hegel as a library

Frontends for other JVM languages (or custom runners) use `Hegel.run` instead of `Hegel.test`. It
returns a `RunReport` rather than throwing, and a `Reporter` lets you own every line of output:

```java
RunReport report = Hegel.run(tc -> { ... }, new Settings().testCases(200), Reporter.silent());
report.status();                        // PASSED, FAILED, or ERROR
report.statistics();                    // valid / invalid / overrun / interesting case counts
for (Failure f : report.failures()) {   // one per distinct counterexample
  f.draws();                            // labelled draws of the minimal example, as Java values
  f.output();                           // the printed report of the minimal example, line by line
  f.exception();                        // the body's own throwable
  f.reproduceBlob();                    // Optional: replay it later with Settings.reproduceFailure
  f.caveat();                           // Optional: how reliably a nondeterministic failure reproduced
}
```

`TestCase.isFinal()` identifies the executions the engine stamped for the failure report (the
final replay of a counterexample among them), `TestCase.span` and `Label` let custom composite
generators tell the engine about their structure, and `Settings.infrastructurePackages` keeps a
frontend's own stack frames out of failure origins.

## Binding Hegel yourself

`dev.hegel:hegel-lowlevel` (Java 17+, no dependencies) is the binding contract on its own, for two
audiences that do not want the Java frontend:

- **Writing a binding** — implement `dev.hegel.lowlevel.Libhegel` (one method per `hegel_*`
  function in `hegel.h` that the Java frontend uses, with raw handles and return codes) over your FFI mechanism, and register
  it as a `dev.hegel.lowlevel.LibhegelBackend` service provider. The two bundled bindings are
  registered the same way.
- **Building a frontend from scratch** — depend on `hegel-lowlevel` plus one binding, call
  `Libhegel.load()` to get the engine, and drive the run loop and per-case primitives yourself.
  `Abi` holds the constants; `LibraryLoader` resolves the shared library.

The package is experimental: new engine functions become new `Libhegel` methods.
