package dev.hegel;

import static dev.hegel.Generators.integers;
import static dev.hegel.Generators.lists;
import static dev.hegel.Generators.maps;
import static dev.hegel.Generators.sets;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
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
        final Set<Long> livePrinters = new HashSet<>();
        final Set<Long> liveRecursions = new HashSet<>();
        final Set<Long> liveGrammarBuilders = new HashSet<>();
        final Set<Long> liveGrammars = new HashSet<>();
        final Set<Long> liveGrammarDraws = new HashSet<>();
        int grammarsCreated;
        int grammarDrawsCreated;
        int standalonePrintersCreated;
        int deferredPrintersCreated;
        int maxLiveTestCases;
        int recursionsCreated;
        int blocksCreated;
        int printersCreated;
        int collectionsCreated;
        int poolsCreated;
        final Map<Thread, Boolean> threadReleased = new HashMap<>();
        final Libhegel lib;

        Tracker(Libhegel real) {
            lib = (Libhegel) Proxy.newProxyInstance(
                    Libhegel.class.getClassLoader(), new Class<?>[] {Libhegel.class}, (proxy, method, args) -> {
                        boolean frees = method.getName().endsWith("Free");
                        if (frees) {
                            record(method.getName(), args, null);
                        }
                        Object result;
                        try {
                            result = method.invoke(real, args);
                        } catch (InvocationTargetException e) {
                            throw e.getCause();
                        }
                        if (!frees) {
                            record(method.getName(), args, result);
                        }
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
                case "newRecursion" -> {
                    if (ok) {
                        recursionsCreated++;
                        liveRecursions.add(((long[]) args[3])[0]);
                    }
                }
                case "recursionFree" -> assertTrue(liveRecursions.remove((Long) args[0]));
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
                case "testCaseBlock" -> {
                    if (ok) {
                        blocksCreated++;
                        liveTestCases.add(((long[]) args[2])[0]);
                    }
                }
                case "testCaseFree" -> assertTrue(liveTestCases.remove((Long) args[0]));
                case "grammarBuilderNew" -> {
                    if (ok) {
                        liveGrammarBuilders.add(((long[]) args[1])[0]);
                    }
                }
                case "grammarBuilderFree" -> assertTrue(liveGrammarBuilders.remove((Long) args[0]));
                case "grammarNew" -> {
                    if (ok) {
                        grammarsCreated++;
                        liveGrammars.add(((long[]) args[6])[0]);
                    }
                }
                case "grammarFree" -> assertTrue(liveGrammars.remove((Long) args[0]));
                case "grammarDrawNew" -> {
                    grammarDrawsCreated++;
                    liveGrammarDraws.add((Long) result);
                }
                case "grammarDrawFree" -> assertTrue(liveGrammarDraws.remove((Long) args[0]));
                case "printerNew" -> {
                    standalonePrintersCreated++;
                    livePrinters.add((Long) result);
                }
                case "testCasePrinter" -> {
                    if (ok) {
                        printersCreated++;
                        livePrinters.add(((long[]) args[1])[0]);
                    }
                }
                case "printerDeferred" -> {
                    if (ok) {
                        deferredPrintersCreated++;
                        livePrinters.add(((long[]) args[1])[0]);
                    }
                }
                case "printerFree" -> assertTrue(livePrinters.remove((Long) args[0]));
                default -> {}
            }
            maxLiveTestCases = Math.max(maxLiveTestCases, liveTestCases.size());
        }

        RunReport run(Consumer<TestCase> body) {
            return run(new Settings(), body);
        }

        RunReport run(Settings settings, Consumer<TestCase> body) {
            return Runner.run(lib, settings.database(Database.disabled()), body, Reporter.silent());
        }

        void assertNothingLeaked() {
            assertEquals(Set.of(), liveCollections);
            assertEquals(Set.of(), livePools);
            assertEquals(Set.of(), liveTestCases);
            assertEquals(Set.of(), livePrinters);
            assertEquals(Set.of(), liveRecursions);
            assertEquals(Set.of(), liveGrammarBuilders);
            assertEquals(Set.of(), liveGrammarDraws);
        }

        synchronized int liveGrammarsCount() {
            return liveGrammars.size();
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
    void setsAndMapsFreeTheirCollections() {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(tc -> {
                    tc.draw(sets(integers().min(0).max(5)));
                    tc.draw(maps(integers().min(0).max(5), integers()));
                })
                .passed());
        assertTrue(tracker.collectionsCreated > 0);
        tracker.assertNothingLeaked();
    }

    @Test
    void forksAndRepetitionBlocksAreFreed() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report =
                tracker.run(new Settings().verbosity(Verbosity.VERBOSE).testCases(5), tc -> {
                    tc.fork(1).draw(integers());
                    int[] repetitions = {0};
                    tc.repeat(() -> {
                        tc.draw(integers(), "x");
                        if (++repetitions[0] > 3) {
                            throw new IllegalStateException();
                        }
                    });
                });
        assertFalse(report.passed());
        assertTrue(tracker.blocksCreated > 0);
        tracker.assertNothingLeaked();
    }

    @Test
    void deferredPrinterRegionsAreFreed() {
        Tracker tracker = new Tracker(Engine.get());
        PrintableGenerator<Printer> slots = (tc, p) -> {
            p.text("slot(");
            Printer slot = p.deferred();
            p.text(")");
            return slot;
        };
        RunReport report =
                tracker.run(new Settings().verbosity(Verbosity.VERBOSE).testCases(5), tc -> {
                    tc.draw(slots).text("filled");
                    tc.fork(1).draw(slots);
                    tc.repeat(() -> {
                        tc.draw(slots);
                        throw new IllegalStateException();
                    });
                });
        assertFalse(report.passed());
        List<String> output = report.failures().get(0).output();
        assertEquals("var draw_1 = slot(filled);", output.get(0));
        assertEquals("  var draw_1 = slot();", output.get(output.size() - 1));
        assertTrue(tracker.deferredPrintersCreated > 0);
        tracker.assertNothingLeaked();
    }

    @Test
    void recursionsAreFreed() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report = tracker.run(tc -> {
            Object tree = tc.draw(Generators.recursive(
                    integers().map(x -> (Object) x), subtrees -> lists(subtrees).map(xs -> (Object) xs)));
            assertTrue(tree instanceof Integer);
        });
        assertFalse(report.passed());
        assertTrue(tracker.recursionsCreated > 0);
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

    @Test
    void printersAndRuleBlocksAreFreed() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report =
                tracker.run(new Settings().verbosity(Verbosity.VERBOSE).testCases(20), tc -> {
                    tc.draw(integers(), "x");
                    Stateful.run(new StatefulTest.PoolMachine(), tc);
                });
        assertTrue(report.passed());
        assertTrue(tracker.blocksCreated > 0);
        assertTrue(tracker.printersCreated > 0);
        tracker.assertNothingLeaked();

        Tracker concurrent = new Tracker(Engine.get());
        report = concurrent.run(
                new Settings().verbosity(Verbosity.VERBOSE).testCases(20),
                tc -> Stateful.run(
                        new ConcurrentStatefulTest.PoolMachine(tc),
                        tc,
                        Stateful.options().minConcurrency(2).maxConcurrency(3)));
        assertTrue(report.passed());
        assertTrue(concurrent.blocksCreated > 0);
        concurrent.assertNothingLeaked();
    }

    @Test
    void repetitionsHoldSteadyHandleCounts() {
        Tracker tracker = new Tracker(Engine.get());
        RunReport report = tracker.run(
                new Settings().verbosity(Verbosity.VERBOSE).testCases(3).seed(1), tc -> {
                    int[] repetitions = {0};
                    tc.repeat(() -> {
                        tc.draw(integers(), "x");
                        if (++repetitions[0] == 30) {
                            throw new IllegalStateException("long enough");
                        }
                    });
                });
        assertInstanceOf(IllegalStateException.class, report.failures().get(0).exception());
        assertEquals(2, tracker.maxLiveTestCases);
        assertTrue(tracker.blocksCreated >= 30);
        tracker.assertNothingLeaked();
    }

    @Test
    void ruleBlocksHoldSteadyHandleCounts() {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(
                        new Settings().verbosity(Verbosity.VERBOSE).testCases(5),
                        tc -> Stateful.run(
                                new StatefulTest.StackMachine(),
                                tc,
                                Stateful.options().stepCount(60)))
                .passed());
        assertTrue(tracker.blocksCreated > 60);
        assertEquals(2, tracker.maxLiveTestCases);
        tracker.assertNothingLeaked();

        Tracker concurrent = new Tracker(Engine.get());
        assertTrue(concurrent
                .run(
                        new Settings().verbosity(Verbosity.VERBOSE).testCases(5),
                        tc -> Stateful.run(
                                new ConcurrentStatefulTest.PoolMachine(tc),
                                tc,
                                Stateful.options()
                                        .stepCount(60)
                                        .minConcurrency(2)
                                        .maxConcurrency(3)))
                .passed());
        assertTrue(concurrent.blocksCreated > 60);
        assertTrue(concurrent.maxLiveTestCases <= 7, "max live test cases: " + concurrent.maxLiveTestCases);
        concurrent.assertNothingLeaked();
    }

    @Test
    void grammarsFreeTheirBuildersAndDrawsAndAreFreedWhenUnreachable() throws InterruptedException {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(tc -> tc.draw(
                        Generators.fromLark("start: A+ B\nA: \"a\"\n%declare B").explicit("B", Generators.just("b"))))
                .passed());
        assertTrue(tracker.grammarsCreated > 0);
        assertTrue(tracker.grammarDrawsCreated > 0);
        tracker.assertNothingLeaked();
        long deadline = System.nanoTime() + 60_000_000_000L;
        while (tracker.liveGrammarsCount() > 0 && System.nanoTime() < deadline) {
            System.gc();
            Thread.sleep(10);
        }
        assertEquals(0, tracker.liveGrammarsCount());
    }

    @Test
    void explicitExamplesFreeTheirPrinters() {
        Tracker tracker = new Tracker(Engine.get());
        assertTrue(tracker.run(
                        new Settings().testCases(1).examples(Example.of(1), Example.of(2)),
                        tc -> tc.draw(integers(), "x"))
                .passed());
        assertEquals(2, tracker.standalonePrintersCreated);
        tracker.assertNothingLeaked();
    }
}
