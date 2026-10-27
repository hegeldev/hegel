/**
 * Property-based testing for Java, powered by the Hegel engine.
 *
 * <p>Instead of writing tests with hand-picked example inputs, you describe a <em>property</em> that
 * should hold for all inputs and let Hegel generate inputs to try to falsify it. When it finds a
 * failing input it automatically <strong>shrinks</strong> it to a minimal counterexample.
 *
 * <p>Hegel requires <strong>Java 22+</strong> (it uses the Foreign Function &amp; Memory API) and
 * {@code --enable-native-access=ALL-UNNAMED} on the test JVM. The native engine, {@code libhegel},
 * is bundled inside the jar for every supported platform and loaded automatically — nothing else to
 * install.
 *
 * <h2>Your first test</h2>
 *
 * <p>Use JUnit 5 as the runner. Annotate a method with {@link dev.hegel.HegelTest @HegelTest} and
 * give it a {@link dev.hegel.TestCase} parameter:
 *
 * <pre>{@code
 * import static dev.hegel.Generators.integers;
 * import static org.junit.jupiter.api.Assertions.assertEquals;
 *
 * import dev.hegel.HegelTest;
 * import dev.hegel.TestCase;
 *
 * class FirstTest {
 *   @HegelTest
 *   void integerSelfEquality(TestCase tc) {
 *     int n = tc.draw(integers());
 *     assertEquals(n, n); // an integer always equals itself
 *   }
 * }
 * }</pre>
 *
 * <p>{@code @HegelTest} runs the method many times (100 by default). Each run receives a
 * {@link dev.hegel.TestCase}, whose {@link dev.hegel.TestCase#draw(dev.hegel.Generator) draw} method
 * produces a value from a generator.
 *
 * <p>When you need a setting that can't be a compile-time constant, or want to run a property
 * outside a JUnit method, drive it programmatically with
 * {@link dev.hegel.Hegel#test(java.util.function.Consumer)} — the body comes first, with optional
 * {@link dev.hegel.Settings}:
 *
 * <pre>{@code
 * import static dev.hegel.Generators.integers;
 * import static org.junit.jupiter.api.Assertions.assertEquals;
 *
 * import dev.hegel.Hegel;
 * import org.junit.jupiter.api.Test;
 *
 * class CommutativityTest {
 *   @Test
 *   void additionCommutes() {
 *     Hegel.test(tc -> {
 *       int x = tc.draw(integers());
 *       int y = tc.draw(integers());
 *       assertEquals(x + y, y + x);
 *     });
 *   }
 * }
 * }</pre>
 *
 * <h2>Understanding test output</h2>
 *
 * <p>When a property fails, Hegel replays the minimal counterexample and prints each top-level
 * {@code draw} as a declaration:
 *
 * <pre>{@code
 * var draw_1 = 50;
 * }</pre>
 *
 * <p>Pass a label — {@code tc.draw(integers(), "n")} — to name the variable instead; each use of
 * a label is numbered in turn:
 *
 * <pre>{@code
 * var n_1 = 50;
 * }</pre>
 *
 * <p>Long values are laid out over several lines, one element per line, and notes from {@link
 * dev.hegel.TestCase#note(String)} appear in order among the draws.
 *
 * <h2>Printing</h2>
 *
 * <p>A value often cannot print itself usefully: a drawn lambda has no readable {@code
 * toString()}, and the useful description of a stateful object is what it does after it is drawn.
 * The generator that made a value can always describe it, so printing is a capability of the
 * generator, not the value: a {@link dev.hegel.PrintableGenerator} draws and prints in one step,
 * writing to the engine's {@link dev.hegel.Printer} as it draws. {@link
 * dev.hegel.TestCase#draw(dev.hegel.Generator) draw} needs a printable generator, so that the
 * report can say what was drawn; {@link dev.hegel.TestCase#drawSilent(dev.hegel.Generator)
 * drawSilent} draws from any generator and prints nothing.
 *
 * <p>Every built-in generator is printable:
 *
 * <ul>
 *   <li>The leaf generators print the value they draw: integers, floats, booleans, strings,
 *       bytes, dates and times, UUIDs, IP addresses, emails, URLs, durations, regex and grammar
 *       strings, pool draws.
 *   <li>The structural generators print as they draw, element by element and component by
 *       component: {@code lists}, {@code sets}, {@code maps}, {@code arrays}, {@code tuples},
 *       {@code optional}, {@code records} and {@code forType}, {@code oneOf}, {@code filter},
 *       {@code flatMap}, {@code recursive} and {@code deferred}. An element's own printing (a
 *       {@link dev.hegel.Generator#printWith printWith} callback, a deferred description) therefore
 *       appears inside its container.
 *   <li>The value-producing generators print the value they produce with {@link
 *       dev.hegel.Printer#repr}, in Java syntax: {@code map}, {@code just}, {@code sampledFrom}
 *       and {@code composite}. A composite's own draws are not printed, only what it returns.
 * </ul>
 *
 * <p>A hand-written {@link dev.hegel.Generator} draws silently. To draw it with {@code draw}:
 *
 * <ul>
 *   <li>{@link dev.hegel.Generator#printWith printWith} prints each value with a callback taking
 *       the value and the printer: {@code .printWith((key, p) -> p.text("Key.of(" + key.id() +
 *       ")"))}. It is also the way to mask a secret, or to print a type whose {@code toString()} is
 *       unhelpful.
 *   <li>{@link dev.hegel.Generator#printAsRepr() printAsRepr} prints each value with {@link
 *       dev.hegel.Printer#repr}.
 *   <li>{@link dev.hegel.generators.MappedGenerator#printAsCall printAsCall}, on a {@code map}
 *       whose source is printable, prints the call that made the value instead of the value:
 *       {@code integers().map(Key::of).printAsCall("Key.of")} reports {@code Key.of(3)}.
 *   <li>Implement {@link dev.hegel.PrintableGenerator}, drawing each part with {@link
 *       dev.hegel.TestCase#drawAndPrint} so that it prints itself in place, or build one with
 *       {@link dev.hegel.Generators#composite(java.util.function.BiFunction)}.
 * </ul>
 *
 * <p>Drawing a generator that cannot print with {@code draw}, or a structural generator with a
 * part that cannot, fails the run with an {@link IllegalArgumentException} naming the generator.
 * Printability belongs to the generator object, so declaring it as a {@code Generator<T>} does not
 * lose it.
 *
 * <p>The {@link dev.hegel.Printer} is the engine's layout engine: text, break points that become
 * newlines only when their group does not fit, and groups. A generator that may reject an attempt
 * prints it inside a {@linkplain dev.hegel.Printer#speculate() speculation} and commits only the
 * accepted one, and {@link dev.hegel.Printer#deferred()} opens a hole that is filled in after the
 * draw, for a value whose description is only known once it is used:
 *
 * <pre>{@code
 * PrintableGenerator<IntSupplier> counters = (tc, p) -> {
 *     int start = tc.drawSilent(integers().min(0).max(9));
 *     p.beginGroup("Counter[calls=[");
 *     Printer calls = p.deferred();
 *     p.endGroup("]]");
 *     int[] next = {start};
 *     return () -> {
 *         if (next[0] > start) {
 *             calls.separator();
 *         }
 *         calls.repr(next[0]);
 *         return next[0]++;
 *     };
 * };
 *
 * IntSupplier c = tc.draw(counters, "c");   // var c_1 = Counter[calls=[3, 4, 5]];
 * }</pre>
 *
 * <p>{@link dev.hegel.Document} lays out a standalone document, for trying printing code outside a
 * test.
 *
 * <p>This report is produced by the run's {@link dev.hegel.Reporter}; the default prints to
 * {@code System.err}. Pass your own to {@link dev.hegel.Hegel#test(java.util.function.Consumer,
 * dev.hegel.Settings, dev.hegel.Reporter)} to route it elsewhere, or {@link
 * dev.hegel.Reporter#silent()} to suppress it.
 *
 * <h2>Generators</h2>
 *
 * <p>{@link dev.hegel.Generators} provides a rich set of generators. Primitives include
 * {@code integers}, {@code longs}, {@code bigIntegers}, {@code floats} (32-bit) and {@code doubles}
 * (64-bit), {@code booleans}, {@code text} (with an {@code alphabet} or a {@code codec}), and
 * {@code binary}; collections include {@code lists} (optionally {@code unique}), {@code sets},
 * {@code maps}, fixed-length {@code arrays} and {@code permutations}; and there are {@code tuples},
 * {@code oneOf}, {@code optional}, {@code nulls}, {@code sampledFrom}, {@code just}, {@code durations} ({@code java.time.Duration}), and the temporal
 * generators {@code dates}, {@code times}, and {@code datetimes} (which produce
 * {@code java.time.LocalDate}/{@code LocalTime}/{@code LocalDateTime}), plus format generators
 * ({@code emails}, {@code urls}, {@code ipAddresses}, {@code uuids}, {@code fromRegex}, …) and
 * {@code fromLark}, which generates strings of a grammar written in Lark's syntax.
 *
 * <p>For zone-aware datetimes, attach a timezone to a {@code datetimes()} generator:
 *
 * <ul>
 *   <li>{@link dev.hegel.generators.DateTimeGenerator#timezones timezones}:
 *       {@code datetimes().timezones(zoneIds())} produces DST-aware {@code java.time.ZonedDateTime}
 *       values over the full range of zones the JVM supports (see
 *       {@link dev.hegel.Generators#zoneIds()}); pin one with
 *       {@code datetimes().timezones(just(ZoneId.of("Europe/London")))}.
 *   <li>{@link dev.hegel.generators.DateTimeGenerator#offsets offsets}:
 *       {@code datetimes().offsets(zoneOffsets())} produces fixed-offset
 *       {@code java.time.OffsetDateTime} values (see {@link dev.hegel.Generators#zoneOffsets()}).
 * </ul>
 *
 * <p>The bound- and size-bearing generators are fluent builders that <em>are</em> the generator:
 *
 * <pre>{@code
 * tc.draw(integers().min(0).max(100));       // bounded ints
 * tc.draw(text().minSize(1).maxSize(10));    // short strings
 * tc.draw(doubles().min(0).max(1));          // a probability (64-bit)
 * tc.draw(floats().min(0).max(1));           // a 32-bit float in [0, 1]
 * tc.draw(lists(integers()).minSize(1).maxSize(5));          // 1–5 element lists
 * }</pre>
 *
 * <h2>Combinators</h2>
 *
 * <p>Build new generators from existing ones (see {@link dev.hegel.Generator}):
 *
 * <ul>
 *   <li>{@link dev.hegel.Generator#map map} transforms each value (and keeps the efficient
 *       single-draw path when possible):
 *       <pre>{@code
 * Generator<Integer> evens = integers().min(0).max(50).map(x -> x * 2);
 * }</pre>
 *   <li>{@link dev.hegel.Generator#filter filter} keeps values matching a predicate (prefer
 *       constraining over filtering when you can):
 *       <pre>{@code
 * Generator<Integer> big = integers().filter(x -> x > 1000);
 * }</pre>
 *   <li>{@link dev.hegel.Generator#flatMap flatMap} makes one draw depend on another:
 *       <pre>{@code
 * Generator<List<Boolean>> sized = integers().min(0).max(10).flatMap(n -> lists(booleans()).minSize(n).maxSize(n));
 * }</pre>
 *   <li>{@link dev.hegel.Generators#composite(java.util.function.Function) composite} builds a
 *       value imperatively from several draws, and prints the value it returns:
 *       <pre>{@code
 * Generator<int[]> pair = Generators.composite(tc -> new int[] {
 *     tc.draw(integers()), tc.draw(integers())
 * });
 * }</pre>
 * </ul>
 *
 * <h2>Recursive generators</h2>
 *
 * <p>{@link dev.hegel.Generators#recursive(dev.hegel.Generator, java.util.function.Function)
 * recursive} builds recursive data such as trees from a generator of leaves and a function that
 * builds a branch from a generator of sub-values. The engine decides where the value branches,
 * within {@code maxDepth} and {@code maxLeaves}, and shrinks a tree toward its subtrees:
 *
 * <pre>{@code
 * Generator<Object> trees = Generators.recursive(
 *     integers().map(n -> (Object) n),
 *     children -> lists(children).maxSize(3).map(xs -> (Object) xs));
 * }</pre>
 *
 * <p>For mutually recursive types, {@link dev.hegel.Generators#deferred() deferred} creates a
 * forward reference so a generator can refer to itself or to one defined later:
 *
 * <pre>{@code
 * record Tree(Integer leaf, Tree left, Tree right) {} // leaf != null XOR children != null
 *
 * Deferred<Tree> tree = Generators.deferred();
 * Generator<Tree> leaf = integers().map(n -> new Tree(n, null, null));
 * Generator<Tree> branch =
 *     tuples(tree, tree).map(t -> new Tree(null, t.value1(), t.value2()));
 * tree.set(oneOf(leaf, branch)); // wire up the self-reference
 * Tree t = tc.draw(tree);
 * }</pre>
 *
 * <p>With {@code deferred} the engine's size control keeps generated structures finite. Drawing
 * before
 * {@link dev.hegel.generators.Deferred#set set} is called fails.
 *
 * <h2>Control functions</h2>
 *
 * <p>Inside a test body you can steer the engine via {@link dev.hegel.TestCase}:
 *
 * <ul>
 *   <li>{@link dev.hegel.TestCase#assume(boolean) assume} discards the current input if a
 *       precondition does not hold, and {@link dev.hegel.TestCase#reject() reject} discards it
 *       unconditionally.
 *   <li>{@link dev.hegel.TestCase#note(String) note} records a message shown only on the final
 *       replay of a failing case.
 *   <li>{@link dev.hegel.TestCase#target(double, String) target} reports a score so the search can
 *       hill-climb toward interesting inputs.
 *   <li>{@link dev.hegel.TestCase#event(String) event} and {@link
 *       dev.hegel.TestCase#eventValue(String, double) eventValue} record what a run saw, for the
 *       statistics report {@code showStatistics} prints.
 *   <li>{@link dev.hegel.TestCase#drawSilent(dev.hegel.Generator) drawSilent} draws a value without
 *       printing it in the counterexample.
 *   <li>{@link dev.hegel.TestCase#repeat(Runnable) repeat} runs a block until the engine decides the
 *       test case is long enough, for tests that are logically infinite loops.
 *   <li>{@link dev.hegel.TestCase#clone() clone} gives another thread its own handle on the test
 *       case to draw from, and {@link dev.hegel.TestCase#setWorker(int) setWorker} says which
 *       thread printed what.
 *   <li>{@link dev.hegel.TestCase#isFinal() isFinal} tells whether this run of the body is the
 *       final replay of a minimal counterexample, where expensive diagnostics are worth doing.
 * </ul>
 *
 * <pre>{@code
 * @HegelTest
 * void divisionRoundTrips(TestCase tc) {
 *   int x = tc.draw(integers().min(1).max(1000));
 *   int y = tc.draw(integers().min(1).max(1000));
 *   tc.assume(y != 0);
 *   tc.note("testing " + x + " * " + y + " / " + y);
 *   assertEquals(x, (x * y) / y);
 * }
 * }</pre>
 *
 * <h2>Settings</h2>
 *
 * <p>Configure a run by passing a {@link dev.hegel.Settings} value (built with
 * {@code new Settings()} and the fluent setters) to
 * {@link dev.hegel.Hegel#test(java.util.function.Consumer, dev.hegel.Settings)}, or with attributes
 * on {@link dev.hegel.HegelTest @HegelTest}:
 *
 * <pre>{@code
 * Hegel.test(
 *     tc -> {
 *       // your property here
 *     },
 *     new Settings()
 *         .testCases(500)    // run more inputs
 *         .seed(42));        // reproducible run
 *
 * @HegelTest(testCases = 1000, seed = 42)
 * void thorough(TestCase tc) {
 *   // your property here
 * }
 * }</pre>
 *
 * <p>Other settings include {@code derandomize},
 * {@link dev.hegel.Settings#database(dev.hegel.Database) database}, {@code suppressHealthCheck},
 * {@code verbosity}, {@code reportMultipleFailures}, {@code stopOnFirstFailure}, {@code
 * showStatistics}, {@code unboundedChoices} and {@link dev.hegel.Settings#phases(dev.hegel.Phase...)
 * phases}, {@link dev.hegel.Settings#examples(dev.hegel.Example...) examples} (explicit {@link
 * dev.hegel.Example}s that run before any generated input; {@code @HegelTest(examples = ...)} names
 * methods that supply them), and a test can start from a named settings profile ({@link
 * dev.hegel.Settings#fromProfile(String)}, or {@code @HegelTest(profile = "...")}). Whatever a test
 * leaves unset is resolved by the engine from its settings profile — a {@code hegel.toml} in the
 * working directory or an ancestor, a profile registered with {@link
 * dev.hegel.Settings#registerProfile(String, dev.hegel.Settings)}, or the shipped {@code ci} profile,
 * which makes runs deterministic and disables the example database when a CI server is detected —
 * and from the {@code HEGEL_*} settings environment variables ({@code HEGEL_TEST_CASES}, {@code
 * HEGEL_SEED}, {@code HEGEL_DATABASE} and so on), which win over the profile.
 * If a health check fires — for example, your generators reject almost every input — Hegel aborts
 * the run and throws {@link dev.hegel.HealthCheckFailure} (distinct from a property's own failure);
 * pass the relevant {@link dev.hegel.HealthCheck} to {@code suppressHealthCheck} if the behaviour is
 * intentional.
 *
 * <h2>Using Hegel as a library</h2>
 *
 * <p>Frontends for other JVM languages, or custom test runners, drive Hegel through
 * {@link dev.hegel.Hegel#run(java.util.function.Consumer, dev.hegel.Settings, dev.hegel.Reporter)}
 * rather than {@code Hegel.test}. It never throws for a property outcome; it returns a
 * {@link dev.hegel.RunReport} with the verdict ({@link dev.hegel.RunStatus}), the
 * {@link dev.hegel.RunStatistics case counts}, the engine's message for an errored run, and one
 * {@link dev.hegel.Failure} per distinct counterexample carrying the exception the body threw, the
 * labelled top-level draws of the minimal example as Java values, its printed report line by line,
 * and the reproduce blob.
 * A {@link dev.hegel.Reporter} receives the same information as callbacks while the run proceeds,
 * which is how a frontend owns its output instead of sharing {@code System.err}:
 *
 * <pre>{@code
 * RunReport report = Hegel.run(body, new Settings().testCases(200), Reporter.silent());
 * if (report.status() == RunStatus.FAILED) {
 *   Failure f = report.failures().get(0);
 *   f.draws();       // {"xs": [0, 0]}
 *   f.exception();   // the body's own throwable
 *   f.reproduceBlob();
 * }
 * report.throwIfFailed(); // Hegel.test's behaviour, when wanted
 * }</pre>
 *
 * <p>The test body is any {@code Consumer<TestCase>}: returning normally passes the case, throwing
 * anything fails it, and nothing is JUnit-specific. Hegel distinguishes bugs by the exception's
 * type and the first stack frame outside Hegel, the JDK, and JUnit; a frontend lists its own
 * packages in {@link dev.hegel.Settings#infrastructurePackages(String...)} so they are skipped
 * too. Custom composite generators implement {@link dev.hegel.Generator} and enclose their draws
 * in {@link dev.hegel.TestCase#span(long, java.util.function.Supplier) tc.span} with a
 * {@link dev.hegel.Label} so the engine shrinks the structure as a unit.
 *
 * <h2>Deriving generators from types</h2>
 *
 * <p>Hegel can build a generator for a record, enum, or supported scalar/collection type by
 * reflection via {@link dev.hegel.Generators#forType(Class) forType} and
 * {@link dev.hegel.Generators#records(Class) records}:
 *
 * <pre>{@code
 * record Point(int x, int y) {}
 * enum Color { RED, GREEN, BLUE }
 *
 * @HegelTest
 * void derived(TestCase tc) {
 *   Point p = tc.draw(Generators.forType(Point.class));
 *   Color c = tc.draw(Generators.forType(Color.class));
 *   // Override a single component:
 *   Point bounded = tc.draw(Generators.records(Point.class).with("x", integers().min(0).max(9)));
 * }
 * }</pre>
 */
package dev.hegel;
