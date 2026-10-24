package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.junit.jupiter.api.Test;

/** A filtered consuming pool draw consumes only the value it yields. */
class PoolFilterTest {
    private static final Settings SETTINGS = new Settings().database(Database.disabled());

    @Test
    void filteredConsumptionPutsRejectedValuesBack() {
        Hegel.test(
                tc -> {
                    Pool<Integer> pool = new Pool<>(tc);
                    for (int i = 0; i < 10; i++) {
                        pool.add(i);
                    }
                    int even = tc.draw(pool.consuming().filter(x -> x % 2 == 0));
                    assertEquals(0, even % 2);
                    assertEquals(9, pool.size());
                    Set<Integer> rest = new HashSet<>();
                    for (int i = 0; i < 9; i++) {
                        rest.add(tc.draw(pool.consuming()));
                    }
                    assertEquals(9, rest.size());
                    assertFalse(rest.contains(even));
                    assertTrue(pool.isEmpty());
                },
                SETTINGS);
    }

    @Test
    void concurrentPoolFilteredConsumptionPutsRejectedValuesBack() {
        Hegel.test(
                tc -> {
                    ConcurrentPool<Integer> pool = new ConcurrentPool<>(tc);
                    for (int i = 0; i < 10; i++) {
                        pool.add(tc, i);
                    }
                    int big = tc.draw(pool.consuming().filter(x -> x >= 5));
                    assertTrue(big >= 5);
                    assertEquals(9, pool.size());
                    Set<Integer> rest = new HashSet<>();
                    for (int i = 0; i < 9; i++) {
                        rest.add(tc.draw(pool.consuming()));
                    }
                    assertEquals(9, rest.size());
                    assertFalse(rest.contains(big));
                },
                SETTINGS);
    }

    private static TestCase fakeCase(FakeLibhegel fake) {
        return new TestCase(new LiveDataSource(fake, FakeLibhegel.TC), false);
    }

    @Test
    void attemptsStopOnceEveryValueIsRejected() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = fakeCase(fake);
        Pool<Integer> pool = new Pool<>(tc);
        pool.add(1);
        assertThrows(AssumeRejected.class, () -> tc.draw(pool.consuming().filter(x -> x == 0)));
        assertEquals(List.of(0L), fake.restoredVariables);
        assertEquals(List.of(Label.FILTER_ATTEMPT, Label.POOL_VALUES_CONSUMED), fake.startedSpans);
        assertEquals(1, pool.size());
    }

    @Test
    void anEmptyPoolRejectsAFilteredDraw() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = fakeCase(fake);
        ConcurrentPool<Integer> pool = new ConcurrentPool<>(tc);
        assertThrows(AssumeRejected.class, () -> tc.draw(pool.consuming().filter(x -> true)));
        assertEquals(List.of(), fake.startedSpans);
    }

    @Test
    void anAbortedCaseRestoresNothing() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = fakeCase(fake);
        Pool<Integer> pool = new Pool<>(tc);
        pool.add(1);
        pool.add(2);
        fake.poolGenerateRcs.add(Abi.OK);
        fake.poolGenerateRcs.add(Abi.E_STOP_TEST);
        assertThrows(StopTest.class, () -> tc.draw(pool.consuming().filter(x -> x == 2)));
        assertEquals(List.of(), fake.restoredVariables);
    }

    @Test
    void aRestoreFailureIsReported() {
        FakeLibhegel fake = new FakeLibhegel();
        TestCase tc = fakeCase(fake);
        Pool<Integer> pool = new Pool<>(tc);
        pool.add(1);
        fake.poolRestoreRc = Abi.E_INVALID_ARG;
        assertThrows(
                IllegalArgumentException.class, () -> tc.draw(pool.consuming().filter(x -> x == 0)));
    }
}
