package dev.hegel;

import java.util.HashMap;
import java.util.Map;
import java.util.function.Predicate;

/**
 * A pool of previously generated values the engine can draw from and shrink over. Mostly used in
 * stateful tests ({@link Stateful}), where a rule needs to act on some value an earlier rule
 * produced.
 *
 * <p>Create one per test case, populate it with {@link #add}, and draw from it through the
 * generators it hands out rather than reading it directly: {@link #reusable()} yields a value
 * without removing it, {@link #consuming()} removes the value it yields. Both are drawn through
 * {@link TestCase#draw}, so the chosen value is recorded in the failing-test replay and the choice
 * shrinks like any other draw. Drawing from an empty pool rejects the current test case (as if by
 * {@code assume(false)}).
 *
 * <p>A pool is bound to the test case it was created on and is not thread-safe: in a machine run
 * with {@link Stateful.Options#maxConcurrency maxConcurrency} above 1, use a {@link
 * ConcurrentPool} instead.
 *
 * @param <T> the type of pooled values
 */
public final class Pool<T> {
    private final TestCase tc;
    private final long poolId;
    private final Map<Long, T> values = new HashMap<>();

    /**
     * Create a pool tracked by the current test case.
     *
     * @param tc the current test case
     */
    public Pool(TestCase tc) {
        this.tc = tc;
        this.poolId = tc.newPool();
        tc.pin();
    }

    /**
     * @return whether no values are in the pool
     */
    public boolean isEmpty() {
        return values.isEmpty();
    }

    /**
     * @return the number of values currently in the pool
     */
    public int size() {
        return values.size();
    }

    /**
     * Add a value to the pool.
     *
     * @param value the value to add
     */
    public void add(T value) {
        values.put(tc.poolAdd(poolId), value);
    }

    /**
     * A generator over the values in the pool that yields a value without removing it.
     *
     * @return the reusing generator
     */
    public PrintableGenerator<T> reusable() {
        return new ReusableGenerator();
    }

    /**
     * A generator that consumes values from the pool: it removes the value it yields, so once
     * consumed a value is never drawn again.
     *
     * <p>A {@link Generator#filter filter} on this generator consumes only the value it yields:
     * each attempt consumes a value, so no attempt draws the same value twice, and once the
     * attempts are over every rejected value is back in the pool. Nothing else puts a value back:
     * one that reaches a rule which then fails an {@link TestCase#assume assumption} stays
     * consumed, and so does one drawn through another combinator, such as a {@link Generator#map
     * map}, that a filter around the combinator rejects.
     *
     * @return the consuming generator
     */
    public PrintableGenerator<T> consuming() {
        return new ConsumingGenerator();
    }

    private final class ReusableGenerator extends PoolGenerator<T> {
        @Override
        public long label() {
            return Label.POOL_VALUES_REUSABLE;
        }

        @Override
        public T doDraw(TestCase tc) {
            return PoolDraws.draw(tc, poolId, values, false);
        }
    }

    private final class ConsumingGenerator extends PoolGenerator<T> {
        @Override
        public long label() {
            return Label.POOL_VALUES_CONSUMED;
        }

        @Override
        public T doDraw(TestCase tc) {
            return PoolDraws.draw(tc, poolId, values, true);
        }

        @Override
        public PrintableGenerator<T> filter(Predicate<? super T> predicate) {
            return new PoolGenerator<T>() {
                @Override
                public long label() {
                    return Label.combine(Label.FILTER, Label.POOL_VALUES_CONSUMED);
                }

                @Override
                public T doDraw(TestCase tc) {
                    return PoolDraws.drawFiltered(tc, poolId, values, predicate);
                }
            };
        }
    }
}
