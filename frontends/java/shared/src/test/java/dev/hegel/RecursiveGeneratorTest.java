package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static dev.hegel.Generators.recursive;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.util.List;
import org.junit.jupiter.api.Test;

class RecursiveGeneratorTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    /** A tree: an {@code Integer} leaf or a {@code List} of trees. */
    private static Generator<Object> trees(int maxDepth, int maxLeaves) {
        return recursive(
                        integers().min(0).max(10).map(x -> (Object) x),
                        subtrees -> lists(subtrees).maxSize(3).map(xs -> (Object) xs))
                .maxDepth(maxDepth)
                .maxLeaves(maxLeaves);
    }

    private static int depth(Object tree) {
        if (tree instanceof List<?> children) {
            int deepest = 0;
            for (Object child : children) {
                deepest = Math.max(deepest, depth(child));
            }
            return deepest + 1;
        }
        return 0;
    }

    private static int leaves(Object tree) {
        if (tree instanceof List<?> children) {
            int total = 0;
            for (Object child : children) {
                total += leaves(child);
            }
            return total;
        }
        return 1;
    }

    @Test
    void valuesRespectTheDepthAndLeafLimits() {
        boolean[] sawBranch = {false};
        Hegel.test(
                tc -> {
                    Object tree = tc.draw(trees(3, 10));
                    assertTrue(depth(tree) <= 3, tree::toString);
                    assertTrue(leaves(tree) <= 10, tree::toString);
                    sawBranch[0] |= tree instanceof List<?>;
                },
                SETTINGS);
        assertTrue(sawBranch[0]);
    }

    @Test
    void depthZeroGeneratesOnlyLeaves() {
        Hegel.test(tc -> assertInstanceOf(Integer.class, tc.draw(trees(0, 10))), SETTINGS.testCases(50));
    }

    @Test
    void deepTreesAreFoundAndShrunk() {
        RunReport report =
                Hegel.run(tc -> assertTrue(depth(tc.draw(trees(10, 50), "t")) < 3), SETTINGS, Reporter.silent());
        Object tree = report.failures().get(0).draws().get("t_1");
        assertEquals(3, depth(tree));
    }

    @Test
    void oversizedAttemptsAreRetried() {
        Generator<Object> bushy = recursive(
                        integers().min(0).max(10).map(x -> (Object) x),
                        subtrees -> lists(subtrees).minSize(3).map(xs -> (Object) xs))
                .maxLeaves(2);
        Hegel.test(tc -> assertInstanceOf(Integer.class, tc.draw(bushy)), SETTINGS.testCases(50));
    }

    @Test
    void invalidLimitsAreRejected() {
        assertThrows(IllegalArgumentException.class, () -> trees(-1, 10));
        assertThrows(IllegalArgumentException.class, () -> trees(3, 0));
    }

    private static TestCase fakeCase(FakeLibhegel fake) {
        return new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
    }

    @Test
    void anOverBudgetAttemptLeavesItsSpansToTheEngine() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.branchSequence.add(true);
        fake.moreSequence = new boolean[] {true, false};
        fake.recursionLeafRcs.add(Abi.E_RETRY);
        TestCase tc = fakeCase(fake);
        Object value = tc.draw(trees(3, 10));
        assertEquals(0, value);
        // The retried attempt's map, list and subtree spans were left open for the engine; only the
        // second attempt's leaf span, its map span and the generator's own span were closed.
        assertEquals(3, fake.stoppedSpans.get());
        assertEquals(List.of(fake.recursionId), fake.freedRecursions);
    }

    @Test
    void aMispricedAttemptIsDrawnAgain() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.recursionFinishRcs.add(Abi.E_RETRY);
        fake.integerValue = 4L;
        TestCase tc = fakeCase(fake);
        assertEquals(4, tc.draw(trees(3, 10)));
        assertTrue(fake.recursionFinishRcs.isEmpty());
    }

    @Test
    void exhaustedAttemptsRejectTheCase() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.recursionLeafRcs.add(Abi.E_RETRY);
        fake.recursionRetryRcs.add(Abi.E_ASSUME);
        TestCase tc = fakeCase(fake);
        assertThrows(AssumeRejected.class, () -> tc.draw(trees(3, 10)));
        assertEquals(List.of(fake.recursionId), fake.freedRecursions);
    }

    @Test
    void engineErrorsAreTranslated() {
        FakeLibhegel fake = new FakeLibhegel();
        fake.recursionFinishRcs.add(Abi.E_STOP_TEST);
        assertThrows(StopTest.class, () -> fakeCase(fake).draw(trees(3, 10)));

        FakeLibhegel leafError = new FakeLibhegel();
        leafError.recursionLeafRcs.add(Abi.E_STOP_TEST);
        assertThrows(StopTest.class, () -> fakeCase(leafError).draw(trees(3, 10)));

        FakeLibhegel creation = new FakeLibhegel();
        creation.newRecursionRc = Abi.E_STOP_TEST;
        assertThrows(StopTest.class, () -> fakeCase(creation).draw(trees(3, 10)));
    }
}
