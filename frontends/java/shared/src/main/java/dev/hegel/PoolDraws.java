package dev.hegel;

import dev.hegel.generators.FilteredGenerator;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.Predicate;

/** The draws {@link Pool} and {@link ConcurrentPool} share, over a pool handle and its values. */
final class PoolDraws {
    private PoolDraws() {}

    /** Draw a value from the pool, removing it when {@code consume}; an empty pool rejects the case. */
    static <T> T draw(TestCase tc, long poolId, Map<Long, T> values, boolean consume) {
        tc.assume(!values.isEmpty());
        long variableId = tc.poolGenerate(poolId, consume);
        return consume ? values.remove(variableId) : values.get(variableId);
    }

    /**
     * A consuming draw of a value {@code predicate} accepts. Each attempt consumes an engine-chosen
     * value inside a {@link Label#FILTER} span, closed as discarded on rejection, so no attempt
     * draws a value an earlier one rejected; the attempts stop early once every value in the pool
     * has been rejected. Once they are over every rejected value is put back, so the draw consumes
     * only the value it yields. The case is rejected when no attempt is accepted.
     */
    static <T> T drawFiltered(TestCase tc, long poolId, Map<Long, T> values, Predicate<? super T> predicate) {
        List<Long> rejected = new ArrayList<>();
        try {
            for (int attempt = 0; attempt < FilteredGenerator.ATTEMPTS && rejected.size() < values.size(); attempt++) {
                tc.startSpan(Label.FILTER);
                boolean discard = true;
                try {
                    long variableId = tc.poolGenerate(poolId, true);
                    if (predicate.test(values.get(variableId))) {
                        discard = false;
                        return values.remove(variableId);
                    }
                    rejected.add(variableId);
                } finally {
                    tc.stopSpan(discard);
                }
            }
        } finally {
            for (long variableId : rejected) {
                tc.poolRestore(poolId, variableId);
            }
        }
        throw new AssumeRejected();
    }
}
