package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import org.junit.jupiter.api.Test;
import org.junit.platform.testkit.engine.EngineTestKit;
import org.junit.platform.testkit.engine.Events;

class HegelTestAnnotationTest {

    @HegelTest
    void additionCommutes(TestCase tc) {
        int x = tc.draw(integers());
        int y = tc.draw(integers());
        assertEquals(x + y, y + x);
    }

    @HegelTest(seed = 99)
    void reverseTwiceIsIdentity(TestCase tc) {
        List<Integer> xs = tc.draw(lists(integers().min(0).max(100)));
        List<Integer> twice = reverse(reverse(xs));
        assertEquals(xs, twice);
    }

    private static List<Integer> reverse(List<Integer> xs) {
        List<Integer> out = new ArrayList<>(xs);
        java.util.Collections.reverse(out);
        return out;
    }

    // A nested class with a deliberately failing property, executed via the JUnit test kit so the
    // failure is observed rather than failing this suite.
    static class FailingProperties {
        @HegelTest(seed = 1)
        void alwaysSmall(TestCase tc) {
            int x = tc.draw(integers().min(0).max(1000));
            assertTrue(x < 5, "too big");
        }
    }

    @Test
    void failingHegelTestIsReportedAsFailure() {
        Events tests = EngineTestKit.engine("junit-jupiter")
                .selectors(org.junit.platform.engine.discovery.DiscoverySelectors.selectClass(FailingProperties.class))
                .execute()
                .testEvents();
        tests.assertStatistics(stats -> stats.started(1).failed(1));
    }

    static class ExampleProperties {
        static Example oneExample() {
            return Example.of(12345);
        }

        List<Example> moreExamples() {
            return List.of(Example.of(7), Example.of(8));
        }

        String notAnExample() {
            return "no";
        }

        @HegelTest(examples = {"oneExample", "moreExamples"})
        void failsOnlyOnTheExample(TestCase tc) {
            int x = tc.draw(integers().min(0).max(10));
            assertTrue(x != 12345, "the example");
        }
    }

    @Test
    void anExampleMethodSuppliesExplicitExamples() {
        Events tests = EngineTestKit.engine("junit-jupiter")
                .selectors(org.junit.platform.engine.discovery.DiscoverySelectors.selectMethod(
                        ExampleProperties.class, "failsOnlyOnTheExample", TestCase.class.getName()))
                .execute()
                .testEvents();
        tests.assertStatistics(stats -> stats.started(1).failed(1));
    }

    @Test
    void exampleMethodsAreInvokedOnTheTestInstance() throws Exception {
        Method test = ExampleProperties.class.getDeclaredMethod("failsOnlyOnTheExample", TestCase.class);
        Example[] examples = HegelTestExtension.examplesFrom(
                test.getAnnotation(HegelTest.class), ExampleProperties.class, new ExampleProperties());
        assertEquals("[Example(12345), Example(7), Example(8)]", java.util.Arrays.toString(examples));
    }

    static class BadExampleMethods {
        @HegelTest(examples = "missing")
        void namesAMissingMethod(TestCase tc) {}

        @HegelTest(examples = "notAnExample")
        void namesAWrongMethod(TestCase tc) {}
    }

    @Test
    void badExampleMethodsAreUsageErrors() throws Exception {
        Method missing = BadExampleMethods.class.getDeclaredMethod("namesAMissingMethod", TestCase.class);
        assertThrows(
                IllegalArgumentException.class,
                () -> HegelTestExtension.examplesFrom(
                        missing.getAnnotation(HegelTest.class), ExampleProperties.class, new ExampleProperties()));
        Method wrong = BadExampleMethods.class.getDeclaredMethod("namesAWrongMethod", TestCase.class);
        assertThrows(
                IllegalArgumentException.class,
                () -> HegelTestExtension.examplesFrom(
                        wrong.getAnnotation(HegelTest.class), ExampleProperties.class, new ExampleProperties()));
    }
}
