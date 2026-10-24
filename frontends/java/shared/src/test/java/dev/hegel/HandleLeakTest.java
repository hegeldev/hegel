package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;
import org.junit.jupiter.api.Test;

/**
 * Every handle the engine hands out is freed, and every thread that drives the engine on Hegel's
 * behalf releases its error context before it exits. The real engine runs behind a counting proxy
 * that records each handle as it is created and as it is freed.
 */
class HandleLeakTest {
    /** Wraps the real binding, tracking live collections, pools and test cases and the threads using it. */
    private static final class Tracker {
        final Set<Long> liveCollections = new HashSet<>();
        final Set<Long> livePools = new HashSet<>();
        final Set<Long> liveTestCases = new HashSet<>();
        int collectionsCreated;
        int poolsCreated;
        final Map<Thread, Boolean> threadReleased = new HashMap<>();
        final Libhegel lib;

        Tracker(Libhegel real) {
            lib = (Libhegel) Proxy.newProxyInstance(
                    Libhegel.class.getClassLoader(), new Class<?>[] {Libhegel.class}, (proxy, method, args) -> {
                        Object result;
                        try {
                            result = method.invoke(real, args);
                        } catch (InvocationTargetException e) {
                            throw e.getCause();
                        }
                        record(method.getName(), args, result);
                        return result;
                    });
        }

        private synchronized void record(String name, Object[] args, Object result) {
            threadReleased.put(Thread.currentThread(), name.equals("releaseThreadContext"));
            boolean ok = result instanceof Integer rc && rc == Abi.OK;
            switch (name) {
                case "newCollection" -> {
                    if (ok) {
                        collectionsCreated++;
                        liveCollections.add(((long[]) args[3])[0]);
                    }
                }
                case "collectionFree" -> assertTrue(liveCollections.remove((Long) args[0]));
                case "newPool" -> {
                    if (ok) {
                        poolsCreated++;
                        livePools.add(((long[]) args[1])[0]);
                    }
                }
                case "poolFree" -> assertTrue(livePools.remove((Long) args[0]));
                case "nextTestCase" -> {
                    if ((Long) result != 0) {
                        liveTestCases.add((Long) result);
                    }
                }
                case "testCaseClone" -> {
                    if (ok) {
                        liveTestCases.add(((long[]) args[1])[0]);
                    }
                }
                case "testCaseFree" -> assertTrue(liveTestCases.remove((Long) args[0]));
                default -> {}
            }
        }

        RunReport run(Consumer<TestCase> body) {
            return Runner.run(lib, new Settings().database(Database.disabled()), body, Reporter.silent());
        }

        void assertNothingLeaked() {
            assertEquals(Set.of(), liveCollections);
            assertEquals(Set.of(), livePools);
            assertEquals(Set.of(), liveTestCases);
        }

        List<Thread> workerThreads() {
            List<Thread> workers = new ArrayList<>();
            for (Thread thread : threadReleased.keySet()) {
                if (thread.getName().startsWith("hegel-worker-")) {
                    workers.add(thread);
                }
            }
            return workers;
        }
    }

    @Test
    void listsFreeTheirCollections() {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(tc -> tc.draw(lists(lists(integers())))).passed());
        assertTrue(tracker.collectionsCreated > 0);
        tracker.assertNothingLeaked();
    }

    @Test
    void collectionsAreFreedWhenTheCaseFails() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report = tracker.run(tc -> {
            List<Integer> xs = tc.draw(lists(integers()));
            assertTrue(xs.size() < 3);
        });
        assertFalse(report.passed());
        tracker.assertNothingLeaked();
    }

    @Test
    void sequentialPoolsAreFreed() {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(tc -> Stateful.run(new StatefulTest.PoolMachine(), tc))
                .passed());
        assertTrue(tracker.poolsCreated > 0);
        tracker.assertNothingLeaked();
    }

    @Test
    void concurrentRunsFreePoolsClonesAndWorkerContexts() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report = tracker.run(tc -> Stateful.run(
                new ConcurrentStatefulTest.PoolMachine(tc),
                tc,
                Stateful.options().minConcurrency(2).maxConcurrency(3)));
        assertTrue(report.passed());
        assertTrue(tracker.poolsCreated > 0);
        tracker.assertNothingLeaked();
        List<Thread> workers = tracker.workerThreads();
        assertFalse(workers.isEmpty());
        for (Thread worker : workers) {
            assertTrue(tracker.threadReleased.get(worker), worker + " exited without releasing its context");
        }
    }
}
