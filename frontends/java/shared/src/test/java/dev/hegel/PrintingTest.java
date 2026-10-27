package dev.hegel;

import static dev.hegel.Generators.arrays;
import static dev.hegel.Generators.booleans;
import static dev.hegel.Generators.composite;
import static dev.hegel.Generators.deferred;
import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.just;
import static dev.hegel.Generators.lists;
import static dev.hegel.Generators.maps;
import static dev.hegel.Generators.oneOf;
import static dev.hegel.Generators.optional;
import static dev.hegel.Generators.optionalDoubles;
import static dev.hegel.Generators.optionalInts;
import static dev.hegel.Generators.optionalLongs;
import static dev.hegel.Generators.records;
import static dev.hegel.Generators.recursive;
import static dev.hegel.Generators.sets;
import static dev.hegel.Generators.text;
import static dev.hegel.Generators.tuples;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.generators.Deferred;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.Set;
import java.util.function.Consumer;
import java.util.function.IntSupplier;
import org.junit.jupiter.api.Test;

/**
 * Generator-driven printing end to end: each top-level draw of a failing case prints as {@code
 * var name = ...;}, described by the generator as it draws.
 */
class PrintingTest {
    private static final Settings SETTINGS =
            new Settings().database(Database.disabled()).derandomize(true).testCases(50);

    /** The output of the reported failure of a property that always fails after {@code body}. */
    private static List<String> failingOutput(Consumer<TestCase> body) {
        RunReport report = Hegel.run(
                tc -> {
                    body.accept(tc);
                    throw new AssertionError("boom");
                },
                SETTINGS,
                Reporter.silent());
        assertEquals(RunStatus.FAILED, report.status(), String.valueOf(report.error()));
        return report.failures().get(0).output();
    }

    private static final PrintableGenerator<Integer> POINTS =
            integers().min(0).max(0).printWith((v, p) -> p.text("pt(" + v + ")"));

    record Point(int x, int y) {}

    record Key(int id) {
        static Key of(int id) {
            return new Key(id);
        }
    }

    /**
     * A counter whose report is the values it hands out, written to a deferred region as it is
     * used, after the draw.
     */
    private static final PrintableGenerator<IntSupplier> COUNTERS = (tc, p) -> {
        int start = tc.drawSilent(integers().min(0).max(0));
        p.beginGroup("Counter[calls=[");
        Printer slot = p.deferred();
        p.endGroup("]]");
        int[] next = {start};
        boolean[] first = {true};
        return () -> {
            if (!first[0]) {
                slot.separator();
            }
            first[0] = false;
            slot.repr(next[0]);
            return next[0]++;
        };
    };

    @Test
    void anElementsOwnPrintingAppearsInsideItsContainer() {
        assertEquals(
                List.of("var xs = [pt(0), pt(0)];"),
                failingOutput(tc -> tc.draw(lists(POINTS).minSize(2).maxSize(2), "xs")));
        assertEquals(
                List.of("var p = Point[x=pt(0), y=0];"),
                failingOutput(tc -> tc.draw(records(Point.class).with("x", POINTS), "p")));
    }

    @Test
    void aDeferredValueIsDescribedAfterItIsDrawn() {
        assertEquals(List.of("var c = Counter[calls=[0, 1, 2]];", "between"), failingOutput(tc -> {
            IntSupplier c = tc.draw(COUNTERS, "c");
            c.getAsInt();
            tc.note("between");
            c.getAsInt();
            c.getAsInt();
        }));
    }

    @Test
    void deferredValuesStayInsideTheirContainer() {
        assertEquals(
                List.of("var cs = [Counter[calls=[0]], Counter[calls=[0, 1]], Counter[calls=[]]];"),
                failingOutput(tc -> {
                    List<IntSupplier> cs = tc.draw(lists(COUNTERS).minSize(3).maxSize(3), "cs");
                    cs.get(0).getAsInt();
                    cs.get(1).getAsInt();
                    cs.get(1).getAsInt();
                }));
    }

    @Test
    void mapPrintsTheFinishedValue() {
        assertEquals(
                List.of("var s = \"xxx\";"),
                failingOutput(tc -> tc.draw(integers().min(3).max(3).map("x"::repeat), "s")));
        assertEquals(
                List.of("var n = 2;"),
                failingOutput(tc -> tc.draw(
                        integers()
                                .min(1)
                                .max(1)
                                .printWith((v, p) -> p.text("ONE"))
                                .map(n -> n + 1),
                        "n")));
    }

    @Test
    void aMapOverSilentPartsPrintsTheFinishedValue() {
        Generator<Integer> silent = tc -> 4;
        assertEquals(
                List.of("var n = 2;"),
                failingOutput(tc -> tc.draw(lists(silent).minSize(2).maxSize(2).map(List::size), "n")));
        assertEquals(
                List.of("var xs = [4];"),
                failingOutput(tc -> tc.draw(lists(silent).minSize(1).maxSize(1).printAsRepr(), "xs")));
    }

    @Test
    void wideValuesWrapAcrossLines() {
        String element = "a".repeat(20);
        assertEquals(
                List.of(
                        "var draw_1 = [\"aaaaaaaaaaaaaaaaaaaa\",",
                        " \"aaaaaaaaaaaaaaaaaaaa\",",
                        " \"aaaaaaaaaaaaaaaaaaaa\"];"),
                failingOutput(tc -> tc.draw(lists(just(element)).minSize(3).maxSize(3))));
    }

    @Test
    void printAsCallWrapsWideInputsInsideTheCall() {
        String element = "a".repeat(20);
        assertEquals(
                List.of(
                        "var draw_1 = concat_all([\"aaaaaaaaaaaaaaaaaaaa\",",
                        "            \"aaaaaaaaaaaaaaaaaaaa\",",
                        "            \"aaaaaaaaaaaaaaaaaaaa\"]);"),
                failingOutput(tc -> tc.draw(lists(just(element))
                        .minSize(3)
                        .maxSize(3)
                        .map(parts -> String.join("", parts))
                        .printAsCall("concat_all"))));
    }

    @Test
    void printAsCallPrintsTheInputDraw() {
        assertEquals(
                List.of("var k = Key.of(3);"),
                failingOutput(
                        tc -> tc.draw(integers().min(3).max(3).map(Key::of).printAsCall("Key.of"), "k")));
        assertEquals(
                List.of("var ks = [Key.of(0), Key.of(0)];"),
                failingOutput(tc -> tc.draw(
                        lists(integers().min(0).max(0).map(Key::of).printAsCall("Key.of"))
                                .minSize(2)
                                .maxSize(2),
                        "ks")));
        assertEquals(
                3L,
                Hegel.run(
                                tc -> assertEquals(
                                        Key.of(3),
                                        tc.drawSilent(integers()
                                                .min(3)
                                                .max(3)
                                                .map(Key::of)
                                                .printAsCall("Key.of"))),
                                SETTINGS.testCases(3),
                                Reporter.silent())
                        .statistics()
                        .valid());
    }

    @Test
    void printAsCallNeedsANamedPrintableSource() {
        Generator<Integer> silent = tc -> 1;
        assertThrows(IllegalArgumentException.class, () -> silent.map(Key::of).printAsCall("Key.of"));
        assertThrows(
                IllegalArgumentException.class, () -> integers().map(Key::of).printAsCall(""));
    }

    @Test
    void printAsReprMakesAHandWrittenGeneratorPrintable() {
        Generator<Integer> seven = tc -> 7;
        PrintableGenerator<Integer> printable = seven.printAsRepr();
        assertEquals(seven.label(), printable.label());
        assertEquals(List.of("var x = 7;"), failingOutput(tc -> tc.draw(printable, "x")));
    }

    @Test
    void aCompositePrintsOnlyTheValueItReturns() {
        assertEquals(
                List.of("var pair = [1, 2];"),
                failingOutput(tc -> tc.draw(
                        composite(inner -> List.of(
                                inner.draw(integers().min(1).max(1), "a"),
                                inner.draw(integers().min(2).max(2), "b"))),
                        "pair")));
    }

    @Test
    void aPrintingCompositePrintsWhatItsBodyPrints() {
        PrintableGenerator<Point> points = composite((tc, p) -> {
            p.beginGroup("Point.of(");
            int x = tc.drawAndPrint(integers().min(1).max(1), p);
            p.separator();
            int ignored = tc.draw(integers().min(5).max(5), "ignored");
            int y = tc.drawAndPrint(integers().min(2).max(2), p);
            p.endGroup(")");
            return new Point(x, y + ignored - 5);
        });
        RunReport report = Hegel.run(
                tc -> {
                    tc.draw(points, "pt");
                    throw new AssertionError("boom");
                },
                SETTINGS,
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertEquals(List.of("var pt = Point.of(1, 2);"), failure.output());
        assertEquals(Map.of("pt", new Point(1, 2)), failure.draws());
        assertEquals(Label.COMPOSITE, points.label());
    }

    @Test
    void rejectedFilterAttemptsPrintNothing() {
        assertEquals(
                List.of("var x = pt(5);"),
                failingOutput(tc -> tc.draw(
                        integers()
                                .min(0)
                                .max(5)
                                .printWith((v, p) -> p.text("pt(" + v + ")"))
                                .filter(x -> x == 5),
                        "x")));
    }

    @Test
    void rejectedDuplicatesPrintNothing() {
        PrintableGenerator<Integer> small = integers().min(0).max(1).printWith((v, p) -> p.text("e" + v));
        List<String> unique =
                failingOutput(tc -> tc.draw(lists(small).unique().minSize(2).maxSize(2), "xs"));
        assertTrue(
                unique.equals(List.of("var xs = [e0, e1];")) || unique.equals(List.of("var xs = [e1, e0];")),
                unique.toString());
        List<String> set = failingOutput(tc -> tc.draw(sets(small).minSize(2).maxSize(2), "s"));
        assertTrue(
                set.equals(List.of("var s = [e0, e1];")) || set.equals(List.of("var s = [e1, e0];")), set.toString());
        List<String> map =
                failingOutput(tc -> tc.draw(maps(small, booleans()).minSize(2).maxSize(2), "m"));
        assertTrue(
                map.equals(List.of("var m = {e0: false, e1: false};"))
                        || map.equals(List.of("var m = {e1: false, e0: false};")),
                map.toString());
    }

    sealed interface Tree permits Leaf, Node {}

    record Leaf(int value) implements Tree {}

    record Node(List<Tree> children) implements Tree {}

    @Test
    @SuppressWarnings("unchecked")
    void recursiveBranchesPrintTheirSubtreesInPlaceAndRetriesLeaveNothing() {
        Generator<Object> zero = tc -> tc.drawSilent(integers().min(0).max(0));
        PrintableGenerator<Object> trees = recursive(zero.printWith((v, p) -> p.text("L")), sub ->
                        (Generator<Object>) (Generator<?>) lists(sub).minSize(2).maxSize(2))
                .maxLeaves(2);
        RunReport report = Hegel.run(
                tc -> {
                    if (tc.draw(trees, "t") instanceof List) {
                        throw new AssertionError("a branch");
                    }
                },
                SETTINGS.testCases(200),
                Reporter.silent());
        assertEquals(List.of("var t = [L, L];"), report.failures().get(0).output());
    }

    @Test
    void forwardReferencesPrintWhatTheyDelegateTo() {
        Deferred<Tree> tree = deferred();
        tree.set(oneOf(
                integers().min(1).max(1).map(Leaf::new),
                lists(tree).minSize(1).maxSize(1).map(Node::new)));
        List<String> out = failingOutput(tc -> tc.draw(tree, "t"));
        assertEquals(List.of("var t = Leaf[value=1];"), out);
    }

    @Test
    void structuralGeneratorsPrintAsTheyDraw() {
        assertEquals(
                List.of("var o = Optional[pt(0)];"),
                failingOutput(tc -> tc.draw(optional(POINTS).filter(Optional::isPresent), "o")));
        assertEquals(List.of("var o = Optional.empty;"), failingOutput(tc -> tc.draw(optional(POINTS), "o")));
        assertEquals(
                List.of("var i = OptionalInt[0];", "var l = OptionalLong.empty;", "var d = OptionalDouble.empty;"),
                failingOutput(tc -> {
                    tc.draw(optionalInts(integers().min(0).max(0)).filter(OptionalInt::isPresent), "i");
                    tc.draw(optionalLongs(Generators.longs()), "l");
                    tc.draw(optionalDoubles(Generators.doubles()), "d");
                }));
        assertEquals(
                List.of("var t = Tuple2[value1=pt(0), value2=\"\"];", "var u = [pt(0), false];"), failingOutput(tc -> {
                    tc.draw(tuples(POINTS, text()), "t");
                    tc.draw(tuples(new Generator<?>[] {POINTS, booleans()}), "u");
                }));
        assertEquals(
                List.of("var a = [pt(0), pt(0)];"),
                failingOutput(tc -> tc.draw(arrays(POINTS, 2, Integer[]::new), "a")));
        assertEquals(List.of("var x = pt(0);"), failingOutput(tc -> tc.draw(oneOf(POINTS, POINTS), "x")));
        assertEquals(
                List.of("var x = pt(0);"),
                failingOutput(tc -> tc.draw(booleans().flatMap(b -> POINTS), "x")));
        Deferred<Integer> forward = deferred();
        forward.set(POINTS);
        assertEquals(List.of("var x = pt(0);"), failingOutput(tc -> tc.draw(forward, "x")));
    }

    @Test
    void upcastingKeepsAGeneratorsOwnPrinting() {
        Generator<Integer> g = POINTS;
        assertEquals(List.of("var x = pt(0);"), failingOutput(tc -> tc.draw(g, "x")));
    }

    @Test
    void poolDrawsPrintTheirValues() {
        assertEquals(List.of("var r = \"a\";", "var c = \"a\";", "var f = \"b\";"), failingOutput(tc -> {
            Pool<String> pool = new Pool<>(tc);
            pool.add("a");
            tc.draw(pool.reusable(), "r");
            tc.draw(pool.consuming(), "c");
            pool.add("b");
            tc.draw(pool.consuming().filter(s -> s.equals("b")), "f");
        }));
        assertEquals(List.of("var r = 1;", "var c = 1;", "var f = 2;"), failingOutput(tc -> {
            ConcurrentPool<Integer> pool = new ConcurrentPool<>(tc);
            pool.add(tc, 1);
            tc.draw(pool.reusable(), "r");
            tc.draw(pool.consuming(), "c");
            pool.add(tc, 2);
            tc.draw(pool.consuming().filter(v -> v == 2), "f");
        }));
    }

    @Test
    void anUnprintableGeneratorCannotBeDrawnButCanBeDrawnSilently() {
        Generator<Integer> silent = tc -> 1;
        List<RunReport> reports = new ArrayList<>();
        int[] cases = {0};
        InvalidArgument e = assertThrows(
                InvalidArgument.class,
                () -> Hegel.run(
                        tc -> {
                            cases[0]++;
                            tc.draw(silent);
                        },
                        SETTINGS,
                        Reporter.silent()));
        assertEquals(1, cases[0]);
        assertTrue(e.getMessage().contains("cannot print the values it draws"), e.getMessage());
        assertTrue(e.getMessage().contains(".printAsRepr()"), e.getMessage());
        InvalidArgument nested = assertThrows(
                InvalidArgument.class, () -> Hegel.run(tc -> tc.draw(lists(silent)), SETTINGS, Reporter.silent()));
        assertTrue(nested.getMessage().startsWith(silent.getClass().getName()), nested.getMessage());
        PrintableGenerator<Integer> inside = (tc, p) -> tc.drawAndPrint(silent, p);
        assertThrows(InvalidArgument.class, () -> Hegel.run(tc -> tc.draw(inside), SETTINGS, Reporter.silent()));
        reports.add(Hegel.run(
                tc -> {
                    assertEquals(1, tc.drawSilent(silent));
                    assertEquals(
                            List.of(1), tc.drawSilent(lists(silent).minSize(1).maxSize(1)));
                    assertEquals(Optional.of(1), tc.drawSilent(optional(silent).filter(Optional::isPresent)));
                    assertEquals(new Tuple2<>(1, 1), tc.drawSilent(tuples(silent, silent)));
                    assertEquals(
                            List.of(1, 1),
                            tc.drawSilent(tuples(silent, silent).map(t -> List.of(t.value1(), t.value2()))));
                    assertEquals(1, (int) tc.drawSilent(oneOf(silent)));
                    assertEquals(1, (int) tc.drawSilent(booleans().flatMap(b -> silent)));
                    assertEquals(
                            Set.of(1), tc.drawSilent(sets(silent).minSize(1).maxSize(1)));
                    assertEquals(
                            Map.of(1, 1),
                            tc.drawSilent(maps(silent, silent).minSize(1).maxSize(1)));
                    assertEquals(1, tc.drawSilent(arrays(silent, 1, Integer[]::new))[0]);
                    assertEquals(
                            new Point(1, 1),
                            tc.drawSilent(records(Point.class).with("x", silent).with("y", silent)));
                    assertEquals(
                            1,
                            tc.drawSilent(recursive(
                                    silent,
                                    sub -> lists(sub).minSize(1).maxSize(1).map(l -> l.get(0)))));
                },
                SETTINGS.testCases(50),
                Reporter.silent()));
        assertSame(RunStatus.PASSED, reports.get(0).status());
    }

    @Test
    void anUnprintableGeneratorCannotBeDrawnInAnExplicitExample() {
        Generator<Integer> silent = tc -> 1;
        assertThrows(
                InvalidArgument.class,
                () -> Hegel.run(tc -> tc.draw(silent, "x"), SETTINGS.examples(Example.of(1)), Reporter.silent()));
    }

    @Test
    void aDrawInsideAPrintableGeneratorPrintsNothing() {
        PrintableGenerator<Integer> sneaky = (tc, p) -> {
            int inner = tc.draw(integers().min(4).max(4), "inner");
            p.text("sum");
            return inner;
        };
        RunReport report = Hegel.run(
                tc -> {
                    tc.draw(sneaky, "outer");
                    throw new AssertionError("boom");
                },
                SETTINGS,
                Reporter.silent());
        Failure failure = report.failures().get(0);
        assertEquals(List.of("var outer = sum;"), failure.output());
        assertEquals(Map.of("outer", 4), failure.draws());
    }
}
